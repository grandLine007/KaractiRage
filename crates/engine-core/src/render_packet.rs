//! Contrat binaire `RenderPacket` v0 — le *seam* entre le noyau de simulation
//! (Rust, autorité) et la couche de rendu (three.js dans la WebView, plus tard un
//! backend natif Bevy/wgpu).
//!
//! Format little-endian, versionné, **sans dépendance**. L'encodeur est déterministe
//! (même scène → mêmes octets) pour rester testable et rejouable.
//!
//! Layout (octets, little-endian) :
//! ```text
//! header    magic u32 (=0x4B50524B "KRPK"), version u16 (=1), flags u16 (=0),
//!           frame_id u32, batch_count u32, light_count u32                 (20 o)
//! camera    13 × f32 : fov, aspect, near, far, pos[3], target[3], up[3]    (52 o)
//! ambient   3 × f32                                                        (12 o)
//! lights    light_count × 52 o
//!   light   kind u8 + pad u8 + pad u16 + color[3] f32 + intensity f32
//!           + pos[3] f32 + dir[3] f32 + range f32 + spot_cos_cutoff f32
//! batches   batch_count × (8 + instance_count × 136 o)
//!   batch   mesh_id u16, material_id u16, instance_count u32               (8 o)
//!   inst    matrix 16 × f32, prev_matrix 16 × f32, tint u32, flags u32   (136 o)
//! ```
//!
//! Les matrices sont **colonne-major**, dans l'ordre `Matrix4.elements` de three.js :
//! le côté JS copie 16 floats d'un bloc dans `matrix.elements` sans réordonner.

use crate::camera::{Camera3D, CameraProjection};
use crate::entity::{Entity, MaterialRef, MeshRef, Renderable3D, Transform3D};
use crate::mesh_assets::{MeshAssetDef, MeshAssetKind, MeshManifest};

/// Signature « KRPK » en little-endian.
pub const MAGIC: u32 = 0x4B50_524B;
/// Version du contrat binaire.
pub const VERSION: u16 = 1;
/// Taille d'un `LightPacket` sérialisé.
pub const LIGHT_BYTES: usize = 52;
/// Taille d'un `InstancePacket` sérialisé (matrice + prev + tint + flags + entity_id).
pub const INSTANCE_BYTES: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPacket {
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    pub pos: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    Directional = 0,
    Point = 1,
    Spot = 2,
    Ambient = 3,
}

impl LightKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Directional),
            1 => Some(Self::Point),
            2 => Some(Self::Spot),
            3 => Some(Self::Ambient),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightPacket {
    pub kind: LightKind,
    pub color: [f32; 3],
    pub intensity: f32,
    pub pos: [f32; 3],
    pub dir: [f32; 3],
    pub range: f32,
    pub spot_cos_cutoff: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstancePacket {
    /// Matrice modèle, colonne-major (ordre `Matrix4.elements` de three.js).
    pub matrix: [f32; 16],
    /// Matrice de la frame précédente pour interpolation de rendu (120 Hz → 60 Hz).
    pub prev_matrix: [f32; 16],
    /// Teinte prémultipliée `0xAARRGGBB`.
    pub tint: u32,
    /// Réservé (0 pour l'instant).
    pub flags: u32,
    /// Identifiant de l'entité source (picking / édition / télémétrie).
    pub entity_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchPacket {
    /// 0–7 : primitives ; ≥ 8 : identifiant d'asset (manifest).
    pub mesh_id: u16,
    pub material_id: u16,
    pub instances: Vec<InstancePacket>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderPacket {
    pub frame_id: u32,
    pub camera: CameraPacket,
    pub ambient: [f32; 3],
    pub lights: Vec<LightPacket>,
    pub batches: Vec<BatchPacket>,
}

// --- Lecture ---

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        if self.buf.len().saturating_sub(self.pos) < n {
            return Err("packet too short");
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        let b: [u8; 2] = self.take(2)?.try_into().map_err(|_| "bad u16")?;
        Ok(u16::from_le_bytes(b))
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        let b: [u8; 4] = self.take(4)?.try_into().map_err(|_| "bad u32")?;
        Ok(u32::from_le_bytes(b))
    }

    fn f32(&mut self) -> Result<f32, &'static str> {
        let b: [u8; 4] = self.take(4)?.try_into().map_err(|_| "bad f32")?;
        Ok(f32::from_le_bytes(b))
    }
}

fn read_f32s<const N: usize>(r: &mut Reader<'_>) -> Result<[f32; N], &'static str> {
    let mut out = [0.0f32; N];
    for v in out.iter_mut() {
        *v = r.f32()?;
    }
    Ok(out)
}

// --- Écriture ---

fn write_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}

fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_f32s(out: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        write_f32(out, *x);
    }
}

fn write_light(out: &mut Vec<u8>, l: &LightPacket) {
    write_u8(out, l.kind as u8);
    write_u8(out, 0);
    write_u16(out, 0);
    write_f32s(out, &l.color);
    write_f32(out, l.intensity);
    write_f32s(out, &l.pos);
    write_f32s(out, &l.dir);
    write_f32(out, l.range);
    write_f32(out, l.spot_cos_cutoff);
}

// --- Encodage / décodage ---

impl RenderPacket {
    /// Taille exacte en octets du paquet encodé (sans allouer).
    pub fn encoded_size(&self) -> usize {
        let header = 20;
        let camera = 13 * 4;
        let ambient = 3 * 4;
        let lights = self.lights.len() * LIGHT_BYTES;
        let batches: usize = self
            .batches
            .iter()
            .map(|b| 8 + b.instances.len() * INSTANCE_BYTES)
            .sum();
        header + camera + ambient + lights + batches
    }

    /// Sérialise le paquet en little-endian (déterministe).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.encoded_size());
        write_u32(&mut out, MAGIC);
        write_u16(&mut out, VERSION);
        write_u16(&mut out, 0); // flags réservés
        write_u32(&mut out, self.frame_id);
        write_u32(&mut out, self.batches.len() as u32);
        write_u32(&mut out, self.lights.len() as u32);

        write_f32(&mut out, self.camera.fov);
        write_f32(&mut out, self.camera.aspect);
        write_f32(&mut out, self.camera.near);
        write_f32(&mut out, self.camera.far);
        write_f32s(&mut out, &self.camera.pos);
        write_f32s(&mut out, &self.camera.target);
        write_f32s(&mut out, &self.camera.up);

        write_f32s(&mut out, &self.ambient);

        for l in &self.lights {
            write_light(&mut out, l);
        }

        for b in &self.batches {
            write_u16(&mut out, b.mesh_id);
            write_u16(&mut out, b.material_id);
            write_u32(&mut out, b.instances.len() as u32);
            for i in &b.instances {
                write_f32s(&mut out, &i.matrix);
                write_f32s(&mut out, &i.prev_matrix);
                write_u32(&mut out, i.tint);
                write_u32(&mut out, i.flags);
                write_u32(&mut out, i.entity_id);
            }
        }

        out
    }

    /// Désérialise un paquet (valide magic + version, rejette les buffers tronqués).
    pub fn decode(buf: &[u8]) -> Result<Self, &'static str> {
        let mut r = Reader::new(buf);

        if r.u32()? != MAGIC {
            return Err("bad magic");
        }
        if r.u16()? != VERSION {
            return Err("unsupported version");
        }
        let _flags = r.u16()?;
        let frame_id = r.u32()?;
        let batch_count = r.u32()? as usize;
        let light_count = r.u32()? as usize;

        let camera = CameraPacket {
            fov: r.f32()?,
            aspect: r.f32()?,
            near: r.f32()?,
            far: r.f32()?,
            pos: read_f32s::<3>(&mut r)?,
            target: read_f32s::<3>(&mut r)?,
            up: read_f32s::<3>(&mut r)?,
        };
        let ambient = read_f32s::<3>(&mut r)?;

        let mut lights = Vec::with_capacity(light_count);
        for _ in 0..light_count {
            let kind = LightKind::from_u8(r.u8()?).ok_or("bad light kind")?;
            let _pad = r.u8()?;
            let _pad2 = r.u16()?;
            let color = read_f32s::<3>(&mut r)?;
            let intensity = r.f32()?;
            let pos = read_f32s::<3>(&mut r)?;
            let dir = read_f32s::<3>(&mut r)?;
            let range = r.f32()?;
            let spot_cos_cutoff = r.f32()?;
            lights.push(LightPacket {
                kind,
                color,
                intensity,
                pos,
                dir,
                range,
                spot_cos_cutoff,
            });
        }

        let mut batches = Vec::with_capacity(batch_count);
        for _ in 0..batch_count {
            let mesh_id = r.u16()?;
            let material_id = r.u16()?;
            let instance_count = r.u32()? as usize;
            let mut instances = Vec::with_capacity(instance_count);
            for _ in 0..instance_count {
                let matrix = read_f32s::<16>(&mut r)?;
                let prev_matrix = read_f32s::<16>(&mut r)?;
                let tint = r.u32()?;
                let flags = r.u32()?;
                let entity_id = r.u32()?;
                instances.push(InstancePacket {
                    matrix,
                    prev_matrix,
                    tint,
                    flags,
                    entity_id,
                });
            }
            batches.push(BatchPacket {
                mesh_id,
                material_id,
                instances,
            });
        }

        Ok(Self {
            frame_id,
            camera,
            ambient,
            lights,
            batches,
        })
    }
}

// --- Producteur Rust (Lot 4) : entités 3D → RenderPacket ---

fn solid_tint(m: &MaterialRef) -> u32 {
    match m {
        MaterialRef::Solid { color, .. } => *color,
        MaterialRef::Asset { .. } => 0xFFFF_FFFF,
    }
}

fn instance_at(mat: glam::Mat4, prev: glam::Mat4, tint: u32, entity_id: u32) -> InstancePacket {
    InstancePacket {
        matrix: mat.to_cols_array(),
        prev_matrix: prev.to_cols_array(),
        tint,
        flags: 0,
        entity_id,
    }
}

/// Étend une entité 3D en éléments de dessin `(mesh_id, material_id, instance)`.
/// Un `MeshRef::Asset` procédural se décompose en autant d'instances que de parties ;
/// un asset inconnu est remplacé par un cube rouge (frontière de confiance).
fn entity_draw_items(
    manifest: &MeshManifest,
    t3d: &Transform3D,
    r3d: &Renderable3D,
    entity_id: u32,
) -> Vec<(u16, u16, InstancePacket)> {
    let entity_mat = t3d.to_mat4();
    let prev_entity_mat = match t3d.prev_position {
        Some(p) => {
            let mut prev = t3d.clone();
            prev.position = p;
            prev.to_mat4()
        }
        None => entity_mat,
    };
    let material_id = material_id_of(&r3d.material);
    let tint = solid_tint(&r3d.material);
    let mut items = Vec::new();

    match &r3d.mesh {
        MeshRef::Cube => items.push((0, material_id, instance_at(entity_mat, prev_entity_mat, tint, entity_id))),
        MeshRef::Sphere => items.push((1, material_id, instance_at(entity_mat, prev_entity_mat, tint, entity_id))),
        MeshRef::Plane => items.push((2, material_id, instance_at(entity_mat, prev_entity_mat, tint, entity_id))),
        MeshRef::Capsule => items.push((3, material_id, instance_at(entity_mat, prev_entity_mat, tint, entity_id))),
        MeshRef::Asset { id, .. } => match manifest.resolve(id) {
            Some(MeshAssetDef {
                kind: MeshAssetKind::Procedural { parts },
                ..
            }) => {
                for part in parts {
                    let world = entity_mat * part.to_mat4();
                    let prev = prev_entity_mat * part.to_mat4();
                    items.push((part.primitive.id(), material_id, instance_at(world, prev, part.tint, entity_id)));
                }
            }
            Some(MeshAssetDef {
                kind: MeshAssetKind::Gltf { .. },
                ..
            }) => {
                items.push((asset_id(id), material_id, instance_at(entity_mat, prev_entity_mat, tint, entity_id)));
            }
            None => {
                items.push((0, material_id, instance_at(entity_mat, prev_entity_mat, 0xFFF43F5E, entity_id)));
            }
        },
    }

    items
}

fn material_id_of(m: &MaterialRef) -> u16 {
    match m {
        MaterialRef::Solid { .. } => 0,
        MaterialRef::Asset { id } => asset_id(id),
    }
}

/// Identifiant stable ≥ 8 pour les assets (hachage FNV-1a 32 bits tronqué sur 15 bits).
fn asset_id(id: &str) -> u16 {
    let mut h: u32 = 0x811c9dc5;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    (8 + (h & 0x7fff)) as u16
}

/// Construit un `RenderPacket` depuis les entités 3D visibles, une caméra et le
/// manifest d'assets. Groupe les instances par (mesh_id, material_id).
pub fn packet_from_entities(
    manifest: &MeshManifest,
    camera: &Camera3D,
    entities: &[&Entity],
    frame_id: u32,
) -> RenderPacket {
    let (fov, near, far) = match camera.projection {
        CameraProjection::Perspective { fov_deg, near, far } => (fov_deg, near, far),
        // Orthographique : le RenderPacket v0 transporte une caméra perspective
        // (fov factice) ; l'ortho est un ajout v1.
        CameraProjection::Orthographic { near, far, .. } => (45.0, near, far),
    };

    let camera_pkt = CameraPacket {
        fov,
        aspect: camera.aspect,
        near,
        far,
        pos: camera.position.to_array(),
        target: camera.target.to_array(),
        up: camera.up.to_array(),
    };

    let mut batches: Vec<BatchPacket> = Vec::new();
    for e in entities {
        let (Some(t3d), Some(r3d)) = (&e.transform3d, &e.renderable3d) else {
            continue;
        };
        for (mesh_id, material_id, instance) in entity_draw_items(manifest, t3d, r3d, e.id as u32) {
            if let Some(b) = batches
                .iter_mut()
                .find(|b| b.mesh_id == mesh_id && b.material_id == material_id)
            {
                b.instances.push(instance);
            } else {
                batches.push(BatchPacket {
                    mesh_id,
                    material_id,
                    instances: vec![instance],
                });
            }
        }
    }

    RenderPacket {
        frame_id,
        camera: camera_pkt,
        ambient: [0.35, 0.35, 0.35],
        lights: vec![],
        batches,
    }
}

/// Conversion HSV → couleur opaque prémultipliée `0xAARRGGBB`.
pub fn hsv_to_argb(h: f32, s: f32, v: f32) -> u32 {
    let c = v * s;
    let hp = (h * 6.0).rem_euclid(6.0);
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to8 = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    (0xFF << 24) | (to8(r) << 16) | (to8(g) << 8) | to8(b)
}

/// Scène 3D de démonstration (tranche verticale Lot 4) : grille 10×10 de cubes
/// animés + sol + lumière directionnelle. Déterministe par (time, frame_id).
pub fn demo_3d_packet(time: f32, frame_id: u32) -> RenderPacket {
    use glam::{Quat, Vec3};

    let camera = Camera3D::perspective(Vec3::new(0.0, 7.0, 16.0), Vec3::ZERO, 60.0, 0.1, 200.0, 1.5);
    let camera_pkt = CameraPacket {
        fov: 60.0,
        aspect: 1.5,
        near: 0.1,
        far: 200.0,
        pos: camera.position.to_array(),
        target: camera.target.to_array(),
        up: camera.up.to_array(),
    };

    let n = 10usize;
    let mut cubes = Vec::with_capacity(n * n);
    for i in 0..n {
        for j in 0..n {
            let x = (i as f32 - (n as f32 - 1.0) * 0.5) * 1.6;
            let z = (j as f32 - (n as f32 - 1.0) * 0.5) * 1.6;
            let y = 0.6 + (time * 1.5 + i as f32 * 0.6 + j as f32 * 0.3).sin() * 0.25;
            let rot = time * 0.6 + (i + j) as f32 * 0.13;
            let t = Transform3D::from_euler_deg(
                Vec3::new(x, y, z),
                Vec3::new(0.0, rot, 0.0),
                Vec3::splat(0.9),
            );
            let matrix = t.to_mat4().to_cols_array();
            let tint = hsv_to_argb(0.5 + (i as f32 / n as f32) * 0.9, 0.7, 0.55);
            cubes.push(InstancePacket {
                matrix,
                prev_matrix: matrix,
                tint,
                flags: 0,
                entity_id: 0,
            });
        }
    }

    // Assets composites : 4 pilliers aux coins (résolus depuis le manifest intégré).
    let manifest = MeshManifest::builtin();
    if let Some(MeshAssetDef {
        kind: MeshAssetKind::Procedural { parts },
        ..
    }) = manifest.resolve("pillar")
    {
        for (px, pz) in [(-9.0, -9.0), (9.0, -9.0), (-9.0, 9.0), (9.0, 9.0)] {
            let base = glam::Mat4::from_scale_rotation_translation(
                Vec3::ONE,
                Quat::IDENTITY,
                Vec3::new(px, 0.0, pz),
            );
            for part in parts {
                let world = base * part.to_mat4();
                let m = world.to_cols_array();
                cubes.push(InstancePacket {
                    matrix: m,
                    prev_matrix: m,
                    tint: part.tint,
                    flags: 0,
                    entity_id: 0,
                });
            }
        }
    }

    // Sol : plane (mesh_id=2) mise à plat, 40×40.
    let ground = Transform3D::new(
        Vec3::ZERO,
        Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        Vec3::new(40.0, 40.0, 1.0),
    );
    let ground_matrix = ground.to_mat4().to_cols_array();

    RenderPacket {
        frame_id,
        camera: camera_pkt,
        ambient: [0.35, 0.35, 0.35],
        lights: vec![LightPacket {
            kind: LightKind::Directional,
            color: [1.0, 1.0, 1.0],
            intensity: 2.2,
            pos: [8.0, 14.0, 6.0],
            dir: [-0.5, -0.8, -0.4],
            range: 0.0,
            spot_cos_cutoff: 0.0,
        }],
        batches: vec![
            BatchPacket {
                mesh_id: 0,
                material_id: 0,
                instances: cubes,
            },
            BatchPacket {
                mesh_id: 2,
                material_id: 0,
                instances: vec![InstancePacket {
                    matrix: ground_matrix,
                    prev_matrix: ground_matrix,
                    tint: 0xFF1B2130,
                    flags: 0,
                    entity_id: 0,
                }],
            },
        ],
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, //
        ]
    }

    fn sample_packet() -> RenderPacket {
        let mut translated = identity();
        translated[12] = 3.0;
        translated[13] = 4.0;

        RenderPacket {
            frame_id: 42,
            camera: CameraPacket {
                fov: 60.0,
                aspect: 1.5,
                near: 0.1,
                far: 200.0,
                pos: [0.0, 6.0, 14.0],
                target: [0.0, 0.0, 0.0],
                up: [0.0, 1.0, 0.0],
            },
            ambient: [0.35, 0.35, 0.35],
            lights: vec![LightPacket {
                kind: LightKind::Directional,
                color: [1.0, 1.0, 1.0],
                intensity: 2.2,
                pos: [8.0, 14.0, 6.0],
                dir: [-0.5, -0.8, -0.4],
                range: 0.0,
                spot_cos_cutoff: 0.0,
            }],
            batches: vec![
                BatchPacket {
                    mesh_id: 0,
                    material_id: 0,
                    instances: vec![InstancePacket {
                        matrix: identity(),
                        prev_matrix: identity(),
                        tint: 0xFF_38BDF8,
                        flags: 0,
                        entity_id: 1,
                    }],
                },
                BatchPacket {
                    mesh_id: 12,
                    material_id: 3,
                    instances: vec![InstancePacket {
                        matrix: translated,
                        prev_matrix: identity(),
                        tint: 0xFF_F59E0B,
                        flags: 0,
                        entity_id: 2,
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_roundtrip_preserves_packet() {
        let p = sample_packet();
        let decoded = RenderPacket::decode(&p.encode()).expect("décodage");
        assert_eq!(decoded, p);
    }

    #[test]
    fn test_encode_is_deterministic() {
        let p = sample_packet();
        assert_eq!(p.encode(), p.encode());
    }

    #[test]
    fn test_size_matches_encoded_bytes() {
        let p = sample_packet();
        assert_eq!(p.encode().len(), p.encoded_size());
    }

    #[test]
    fn test_size_budget_thousand_instances() {
        let instances = vec![
            InstancePacket {
                matrix: identity(),
                prev_matrix: identity(),
                tint: 0xFF_FFFFFF,
                flags: 0,
                entity_id: 0,
            };
            1000
        ];
        let p = RenderPacket {
            frame_id: 0,
            camera: CameraPacket {
                fov: 60.0,
                aspect: 1.0,
                near: 0.1,
                far: 100.0,
                pos: [0.0; 3],
                target: [0.0; 3],
                up: [0.0, 1.0, 0.0],
            },
            ambient: [0.0; 3],
            lights: vec![],
            batches: vec![BatchPacket {
                mesh_id: 0,
                material_id: 0,
                instances,
            }],
        };
        let bytes = p.encode();
        // 20 (header) + 52 (caméra) + 12 (ambient) + 8 (batch) + 1000×136.
        assert_eq!(bytes.len(), 20 + 52 + 12 + 8 + 1000 * INSTANCE_BYTES);
        // 2 000 instances ≈ 272 Ko/frame, très en dessous des 3,84 Mo/frame du
        // pixel-streaming 2D actuel (1200×800×4).
        assert!(bytes.len() * 2 < 3_840_000);
    }

    #[test]
    fn test_decode_rejects_bad_magic() {
        let mut bytes = sample_packet().encode();
        bytes[0] = 0x00;
        assert_eq!(RenderPacket::decode(&bytes), Err("bad magic"));
    }

    #[test]
    fn test_decode_rejects_bad_version() {
        let mut bytes = sample_packet().encode();
        // La version u16 est aux octets 4..6 (juste après le magic u32).
        bytes[4] = 0x00;
        bytes[5] = 0x00;
        assert_eq!(RenderPacket::decode(&bytes), Err("unsupported version"));
    }

    #[test]
    fn test_decode_rejects_truncated() {
        let bytes = sample_packet().encode();
        let cut = &bytes[..bytes.len() - 10];
        assert_eq!(RenderPacket::decode(cut), Err("packet too short"));
    }
}

#[cfg(test)]
mod producer_tests {
    use super::*;
    use crate::camera::Camera3D;
    use crate::entity::{Entity, MaterialRef, MeshRef, Renderable3D, Transform2D, Transform3D};
    use crate::mesh_assets::MeshManifest;
    use glam::{Quat, Vec3};

    fn cube_entity(id: usize, x: f32) -> Entity {
        let mut e = Entity::new(id, "cube", "cube", Transform2D::new(0.0, 0.0, 1.0, 1.0));
        e.transform3d = Some(Transform3D::new(Vec3::new(x, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE));
        e.renderable3d = Some(Renderable3D {
            mesh: MeshRef::Cube,
            material: MaterialRef::Solid {
                color: 0xFF_38BDF8,
                roughness: 0.4,
                metalness: 0.0,
            },
            cast_shadow: false,
            receive_shadow: false,
        });
        e
    }

    #[test]
    fn test_packet_from_entities_groups_by_mesh_material() {
        let cam = Camera3D::perspective(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 60.0, 0.1, 100.0, 1.0);
        let e1 = cube_entity(0, 0.0);
        let e2 = cube_entity(1, 2.0);
        let manifest = MeshManifest::builtin();
        let packet = packet_from_entities(&manifest, &cam, &[&e1, &e2], 1);
        assert_eq!(packet.batches.len(), 1, "même (mesh, material) → 1 seul batch");
        assert_eq!(packet.batches[0].instances.len(), 2);
        // La matrice de e2 est décalée de 2 sur X (élément 12 = translation X colonne-major).
        assert_eq!(packet.batches[0].instances[1].matrix[12], 2.0);
    }

    #[test]
    fn test_packet_from_entities_skips_2d_entities() {
        let cam = Camera3D::perspective(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 60.0, 0.1, 100.0, 1.0);
        let e2d = Entity::new(0, "2d", "player", Transform2D::new(0.0, 0.0, 32.0, 32.0));
        let e3d = cube_entity(1, 0.0);
        let manifest = MeshManifest::builtin();
        let packet = packet_from_entities(&manifest, &cam, &[&e2d, &e3d], 1);
        assert_eq!(packet.batches.len(), 1);
        assert_eq!(packet.batches[0].instances.len(), 1);
    }

    #[test]
    fn test_asset_resolution_expands_parts_and_falls_back() {
        let cam = Camera3D::perspective(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 60.0, 0.1, 100.0, 1.0);
        let manifest = MeshManifest::builtin();

        // Asset procédural connu : "pillar" = 2 parties (2 cubes) → 1 batch mesh_id 0.
        let mut pillar = Entity::new(0, "pillar", "pillar", Transform2D::new(0.0, 0.0, 1.0, 1.0));
        pillar.transform3d = Some(Transform3D::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE));
        pillar.renderable3d = Some(Renderable3D {
            mesh: MeshRef::Asset { id: "pillar".into(), node: None },
            material: MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
            cast_shadow: false,
            receive_shadow: false,
        });
        let packet = packet_from_entities(&manifest, &cam, &[&pillar], 1);
        assert_eq!(packet.batches.len(), 1);
        assert_eq!(packet.batches[0].instances.len(), 2, "pillar = 2 parties");

        // Asset inconnu : repli cube rouge (mesh_id 0, teinte d'erreur).
        let mut unknown = Entity::new(1, "u", "u", Transform2D::new(0.0, 0.0, 1.0, 1.0));
        unknown.transform3d = Some(Transform3D::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE));
        unknown.renderable3d = Some(Renderable3D {
            mesh: MeshRef::Asset { id: "does_not_exist".into(), node: None },
            material: MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
            cast_shadow: false,
            receive_shadow: false,
        });
        let packet = packet_from_entities(&manifest, &cam, &[&unknown], 1);
        assert_eq!(packet.batches.len(), 1);
        assert_eq!(packet.batches[0].mesh_id, 0);
        assert_eq!(packet.batches[0].instances[0].tint, 0xFFF43F5E);
    }

    #[test]
    fn test_demo_3d_packet_roundtrips() {
        let p = demo_3d_packet(1.0, 7);
        assert_eq!(p.batches.len(), 2);
        let instances: usize = p.batches.iter().map(|b| b.instances.len()).sum();
        assert_eq!(instances, 109, "100 cubes + 8 parties de pilliers + 1 sol");
        let decoded = RenderPacket::decode(&p.encode()).expect("roundtrip démo");
        assert_eq!(decoded, p);
    }
}


