//! Manifest d'assets 3D — la *frontière de confiance* entre l'IA (ou l'auteur) et le
//! moteur. Un asset est soit **procédural** (assemblage de primitives, 100 % texte,
//! aucun binaire), soit une **référence glTF/GLB** chargée par le renderer.
//!
//! Philosophie : l'IA référence des identifiants (`MeshRef::Asset { id }`), jamais des
//! binaires. Un `id` absent du manifest est rejeté et remplacé par une primitive de
//! repli — exactement le pattern des sprites pxl (Lot 5/6 de la feuille de route 2D).

use glam::{Quat, Vec3};

/// Primitive 3D intégrée (brique de base d'un mesh procédural composite).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshPrimitive {
    Cube,
    Sphere,
    Plane,
    Capsule,
}

impl MeshPrimitive {
    /// Identifiant de primitive du `RenderPacket` (0-3).
    pub fn id(self) -> u16 {
        match self {
            Self::Cube => 0,
            Self::Sphere => 1,
            Self::Plane => 2,
            Self::Capsule => 3,
        }
    }
}

/// Une partie d'un mesh composite : primitive + transform local + teinte.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshPart {
    pub primitive: MeshPrimitive,
    pub position: Vec3,
    pub rotation_euler_deg: Vec3,
    pub scale: Vec3,
    /// Teinte prémultipliée `0xAARRGGBB`.
    pub tint: u32,
}

impl MeshPart {
    pub fn to_mat4(&self) -> glam::Mat4 {
        let rotation = Quat::from_euler(
            glam::EulerRot::XYZ,
            self.rotation_euler_deg.x.to_radians(),
            self.rotation_euler_deg.y.to_radians(),
            self.rotation_euler_deg.z.to_radians(),
        );
        glam::Mat4::from_scale_rotation_translation(self.scale, rotation, self.position)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MeshAssetKind {
    /// Assemblage de primitives (aucun binaire requis).
    Procedural { parts: Vec<MeshPart> },
    /// Référence à un fichier glTF/GLB (+ nœud optionnel), chargée par le renderer.
    Gltf { file: String, node: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshAssetDef {
    pub id: String,
    pub kind: MeshAssetKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshManifest {
    pub assets: Vec<MeshAssetDef>,
}

impl MeshManifest {
    pub fn resolve(&self, id: &str) -> Option<&MeshAssetDef> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// Manifest intégré : assets procéduraux de démonstration (aucun binaire requis).
    pub fn builtin() -> Self {
        let mut m = Self::default();

        m.assets.push(MeshAssetDef {
            id: "pillar".into(),
            kind: MeshAssetKind::Procedural {
                parts: vec![
                    MeshPart {
                        primitive: MeshPrimitive::Cube,
                        position: Vec3::new(0.0, 0.5, 0.0),
                        rotation_euler_deg: Vec3::ZERO,
                        scale: Vec3::new(0.6, 1.0, 0.6),
                        tint: 0xFF64748B,
                    },
                    MeshPart {
                        primitive: MeshPrimitive::Cube,
                        position: Vec3::new(0.0, 1.6, 0.0),
                        rotation_euler_deg: Vec3::ZERO,
                        scale: Vec3::new(0.8, 0.2, 0.8),
                        tint: 0xFF94A3B8,
                    },
                ],
            },
        });

        m.assets.push(MeshAssetDef {
            id: "rock".into(),
            kind: MeshAssetKind::Procedural {
                parts: vec![MeshPart {
                    primitive: MeshPrimitive::Sphere,
                    position: Vec3::ZERO,
                    rotation_euler_deg: Vec3::ZERO,
                    scale: Vec3::new(1.0, 0.7, 0.8),
                    tint: 0xFF78716C,
                }],
            },
        });

        m.assets.push(MeshAssetDef {
            id: "platform".into(),
            kind: MeshAssetKind::Procedural {
                parts: vec![MeshPart {
                    primitive: MeshPrimitive::Cube,
                    position: Vec3::ZERO,
                    rotation_euler_deg: Vec3::ZERO,
                    scale: Vec3::new(3.0, 0.25, 1.5),
                    tint: 0xFF334155,
                }],
            },
        });

        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_resolves_procedural_assets() {
        let m = MeshManifest::builtin();
        assert!(m.resolve("pillar").is_some());
        assert!(m.resolve("rock").is_some());
        assert!(m.resolve("platform").is_some());
        assert!(m.resolve("unknown").is_none());
        let pillar = m.resolve("pillar").unwrap();
        match &pillar.kind {
            MeshAssetKind::Procedural { parts } => assert_eq!(parts.len(), 2),
            _ => panic!("pillar doit être procédural"),
        }
    }

    #[test]
    fn test_mesh_part_rotation() {
        let part = MeshPart {
            primitive: MeshPrimitive::Cube,
            position: Vec3::ZERO,
            rotation_euler_deg: Vec3::new(0.0, 90.0, 0.0),
            scale: Vec3::ONE,
            tint: 0xFFFFFFFF,
        };
        let m = part.to_mat4();
        // Rotation +90° autour de Y : l'axe X local pointe vers -Z monde.
        assert!((m.x_axis.z + 1.0).abs() < 1e-5);
    }
}
