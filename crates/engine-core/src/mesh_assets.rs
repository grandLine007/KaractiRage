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

/// Partie sans rotation (position, échelle, teinte).
fn part(primitive: MeshPrimitive, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, tint: u32) -> MeshPart {
    MeshPart {
        primitive,
        position: Vec3::new(x, y, z),
        rotation_euler_deg: Vec3::ZERO,
        scale: Vec3::new(sx, sy, sz),
        tint,
    }
}

/// Partie avec rotation d'Euler en degrés (XYZ).
fn part_rot(primitive: MeshPrimitive, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, rot: (f32, f32, f32), tint: u32) -> MeshPart {
    MeshPart {
        primitive,
        position: Vec3::new(x, y, z),
        rotation_euler_deg: Vec3::new(rot.0, rot.1, rot.2),
        scale: Vec3::new(sx, sy, sz),
        tint,
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
        let mut add = |id: &str, parts: Vec<MeshPart>| {
            m.assets.push(MeshAssetDef { id: id.into(), kind: MeshAssetKind::Procedural { parts } });
        };

        // --- Commun ----------------------------------------------------------
        add("pillar", vec![
            part(MeshPrimitive::Cube, 0.0, 0.5, 0.0, 0.6, 1.0, 0.6, 0xFF64748B),
            part(MeshPrimitive::Cube, 0.0, 1.6, 0.0, 0.8, 0.2, 0.8, 0xFF94A3B8),
        ]);
        add("rock", vec![
            part(MeshPrimitive::Sphere, 0.0, 0.0, 0.0, 1.0, 0.7, 0.8, 0xFF78716C),
        ]);
        add("platform", vec![
            part(MeshPrimitive::Cube, 0.0, 0.0, 0.0, 3.0, 0.25, 1.5, 0xFF334155),
        ]);

        // --- RPG (donjon) ----------------------------------------------------
        add("rpg_tree", vec![
            part(MeshPrimitive::Capsule, 0.0, 1.0, 0.0, 0.3, 2.0, 0.3, 0xFF7C4A21),
            part(MeshPrimitive::Sphere, 0.0, 3.2, 0.0, 1.4, 1.4, 1.4, 0xFF2E7D32),
            part(MeshPrimitive::Sphere, 0.6, 2.8, 0.4, 0.9, 0.9, 0.9, 0xFF34A853),
        ]);
        add("rpg_crystal", vec![
            part_rot(MeshPrimitive::Cube, 0.0, 1.0, 0.0, 0.4, 2.0, 0.4, (0.0, 45.0, 0.0), 0xFFA855F7),
            part_rot(MeshPrimitive::Cube, 0.0, 2.2, 0.0, 0.25, 0.8, 0.25, (0.0, 45.0, 0.0), 0xFFC084FC),
        ]);
        add("rpg_column", vec![
            part(MeshPrimitive::Cube, 0.0, 0.2, 0.0, 0.9, 0.4, 0.9, 0xFF64748B),
            part(MeshPrimitive::Capsule, 0.0, 1.6, 0.0, 0.5, 2.6, 0.5, 0xFF94A3B8),
            part(MeshPrimitive::Cube, 0.0, 3.0, 0.0, 0.9, 0.4, 0.9, 0xFF64748B),
        ]);
        add("rpg_barrel", vec![
            part(MeshPrimitive::Capsule, 0.0, 0.6, 0.0, 0.9, 0.6, 0.9, 0xFFB45309),
            part(MeshPrimitive::Cube, 0.0, 0.6, 0.0, 1.9, 0.9, 0.9, 0xFF7C2D12),
        ]);
        add("rpg_brazier", vec![
            part(MeshPrimitive::Capsule, 0.0, 0.4, 0.0, 0.7, 0.8, 0.7, 0xFF64748B),
            part(MeshPrimitive::Sphere, 0.0, 1.2, 0.0, 0.45, 0.45, 0.45, 0xFFF97316),
            part(MeshPrimitive::Sphere, 0.0, 1.3, 0.0, 0.25, 0.25, 0.25, 0xFFFBBF24),
        ]);
        add("rpg_statue", vec![
            part(MeshPrimitive::Cube, 0.0, 0.2, 0.0, 1.0, 0.4, 1.0, 0xFF64748B),
            part(MeshPrimitive::Cube, 0.0, 1.0, 0.0, 0.5, 1.2, 0.5, 0xFF94A3B8),
            part(MeshPrimitive::Sphere, 0.0, 1.9, 0.0, 0.35, 0.35, 0.35, 0xFF94A3B8),
        ]);
        add("rpg_gate", vec![
            part(MeshPrimitive::Cube, -2.0, 1.5, 0.0, 0.6, 3.0, 0.6, 0xFF64748B),
            part(MeshPrimitive::Cube, 2.0, 1.5, 0.0, 0.6, 3.0, 0.6, 0xFF64748B),
            part(MeshPrimitive::Cube, 0.0, 3.2, 0.0, 5.0, 0.5, 0.6, 0xFF94A3B8),
        ]);

        // --- Platformer ------------------------------------------------------
        add("plat_spring", vec![
            part(MeshPrimitive::Cube, 0.0, 0.15, 0.0, 0.9, 0.3, 0.9, 0xFF64748B),
            part(MeshPrimitive::Capsule, 0.0, 0.6, 0.0, 0.7, 0.9, 0.7, 0xFF94A3B8),
            part(MeshPrimitive::Cube, 0.0, 1.1, 0.0, 0.9, 0.15, 0.9, 0xFFE2E8F0),
        ]);
        add("plat_cloud", vec![
            part(MeshPrimitive::Sphere, 0.0, 0.0, 0.0, 1.2, 0.7, 0.8, 0xFFF8FAFC),
            part(MeshPrimitive::Sphere, 0.9, -0.2, 0.2, 0.8, 0.6, 0.6, 0xFFE2E8F0),
            part(MeshPrimitive::Sphere, -0.9, -0.2, -0.2, 0.8, 0.6, 0.6, 0xFFE2E8F0),
        ]);
        add("plat_coin", vec![
            part_rot(MeshPrimitive::Sphere, 0.0, 0.0, 0.0, 1.0, 0.15, 1.0, (90.0, 0.0, 0.0), 0xFFFBBF24),
        ]);
        add("plat_flag", vec![
            part(MeshPrimitive::Capsule, 0.0, 1.5, 0.0, 0.1, 3.0, 0.1, 0xFF94A3B8),
            part(MeshPrimitive::Cube, 0.7, 2.5, 0.0, 1.4, 0.9, 0.1, 0xFFEF4444),
        ]);

        // --- SHMUP -----------------------------------------------------------
        add("shmup_asteroid_big", vec![
            part(MeshPrimitive::Sphere, 0.0, 0.0, 0.0, 1.3, 1.0, 1.1, 0xFF78716C),
            part(MeshPrimitive::Sphere, 0.8, 0.3, 0.4, 0.6, 0.5, 0.6, 0xFF64748B),
            part(MeshPrimitive::Sphere, -0.7, -0.2, -0.5, 0.7, 0.6, 0.6, 0xFF94A3B8),
        ]);
        add("shmup_ship", vec![
            part(MeshPrimitive::Capsule, 0.0, 0.0, 0.0, 0.6, 1.4, 0.6, 0xFF38BDF8),
            part(MeshPrimitive::Cube, 1.1, -0.1, 0.0, 1.6, 0.15, 0.8, 0xFF0EA5E9),
            part(MeshPrimitive::Cube, -1.1, -0.1, 0.0, 1.6, 0.15, 0.8, 0xFF0EA5E9),
        ]);
        add("shmup_mine", vec![
            part(MeshPrimitive::Sphere, 0.0, 0.0, 0.0, 0.7, 0.7, 0.7, 0xFFEF4444),
            part(MeshPrimitive::Cube, 0.9, 0.0, 0.0, 0.5, 0.1, 0.1, 0xFF94A3B8),
            part(MeshPrimitive::Cube, -0.9, 0.0, 0.0, 0.5, 0.1, 0.1, 0xFF94A3B8),
            part(MeshPrimitive::Cube, 0.0, 0.0, 0.9, 0.1, 0.1, 0.5, 0xFF94A3B8),
            part(MeshPrimitive::Cube, 0.0, 0.0, -0.9, 0.1, 0.1, 0.5, 0xFF94A3B8),
        ]);
        // --- Sokoban ---------------------------------------------------------
        add("soko_crate", vec![
            part(MeshPrimitive::Cube, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0xFFB45309),
            part(MeshPrimitive::Cube, 0.0, 0.0, 0.0, 1.1, 0.12, 1.1, 0xFF7C2D12),
            part(MeshPrimitive::Cube, 0.0, 0.0, 0.0, 0.12, 1.1, 1.1, 0xFF7C2D12),
        ]);
        add("soko_goal_pad", vec![
            part(MeshPrimitive::Sphere, 0.0, 0.05, 0.0, 0.9, 0.1, 0.9, 0xFF34D399),
            part(MeshPrimitive::Sphere, 0.0, 0.1, 0.0, 0.5, 0.08, 0.5, 0xFFF8FAFC),
        ]);
        add("soko_player", vec![
            part(MeshPrimitive::Capsule, 0.0, 0.7, 0.0, 0.45, 1.3, 0.45, 0xFF38BDF8),
            part(MeshPrimitive::Sphere, 0.0, 1.55, 0.0, 0.35, 0.35, 0.35, 0xFFF8FAFC),
        ]);

        // --- Boss ------------------------------------------------------------
        add("boss_obelisk", vec![
            part(MeshPrimitive::Cube, 0.0, 0.4, 0.0, 1.4, 0.8, 1.4, 0xFF450A0A),
            part(MeshPrimitive::Cube, 0.0, 1.4, 0.0, 1.0, 1.2, 1.0, 0xFF450A0A),
            part(MeshPrimitive::Cube, 0.0, 2.5, 0.0, 0.6, 1.0, 0.6, 0xFFEF4444),
            part(MeshPrimitive::Cube, 0.0, 3.3, 0.0, 0.3, 0.6, 0.3, 0xFFF97316),
        ]);
        add("boss_torch", vec![
            part(MeshPrimitive::Capsule, 0.0, 0.5, 0.0, 0.12, 1.0, 0.12, 0xFF64748B),
            part(MeshPrimitive::Sphere, 0.0, 1.2, 0.0, 0.4, 0.4, 0.4, 0xFFF97316),
            part(MeshPrimitive::Sphere, 0.0, 1.35, 0.0, 0.2, 0.2, 0.2, 0xFFFBBF24),
        ]);
        add("boss_wall", vec![
            part(MeshPrimitive::Cube, 0.0, 0.5, 0.0, 2.4, 1.0, 0.4, 0xFF475569),
            part(MeshPrimitive::Cube, 0.0, 0.1, 0.0, 2.6, 0.2, 0.6, 0xFF334155),
        ]);

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
