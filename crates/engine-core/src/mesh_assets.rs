//! Manifest d'assets 3D — la *frontière de confiance* entre l'IA (ou l'auteur) et le
//! moteur. Un asset est soit **procédural** (assemblage de primitives, 100 % texte,
//! aucun binaire), soit une **référence glTF/GLB** chargée par le renderer.
//!
//! Philosophie : l'IA référence des identifiants (`MeshRef::Asset { id }`), jamais des
//! binaires. Un `id` absent du manifest est rejeté et remplacé par une primitive de
//! repli — exactement le pattern des sprites pxl (Lot 5/6 de la feuille de route 2D).

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use glam::{Quat, Vec3};

use crate::ai_dictionary::hash_text;
use crate::assets_manifest::{
    AssetEntry, AssetKind, AssetsManifest, ManifestError, Provenance, ProvenanceSource,
};

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

// ---------------------------------------------------------------------------
// Import d'un maillage glTF/GLB dans un projet (étape C2 du contrat global)
// ---------------------------------------------------------------------------

/// Taille maximale acceptée pour un fichier de maillage importé (4 Mio).
///
/// Le budget global du manifeste (8 Mio par défaut) reste appliqué en plus :
/// cette borne évite qu'un seul modèle consomme toute la place disponible.
pub const MAX_IMPORTED_MESH_BYTES: u64 = 4 * 1024 * 1024;

/// Format binaire reconnu à l'import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshFileFormat {
    /// Conteneur binaire glTF (magic `glTF`).
    Glb,
    /// glTF JSON autoporteur (buffers et images en `data:` URI).
    Gltf,
}

impl MeshFileFormat {
    /// Extension de fichier écrite dans `assets/meshes/`.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Glb => "glb",
            Self::Gltf => "gltf",
        }
    }

    /// Libellé lisible pour les messages du Studio.
    pub fn label(self) -> &'static str {
        match self {
            Self::Glb => "GLB",
            Self::Gltf => "glTF JSON autoporteur",
        }
    }
}

/// Résultat d'un import réussi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedMesh {
    /// Identifiant stable (slug du nom, `[a-z0-9_-]`).
    pub id: String,
    /// Chemin relatif au projet, par exemple `assets/meshes/robot.glb`.
    pub path: String,
    /// Taille en octets.
    pub bytes: u64,
    /// Empreinte FNV-1a 64 bits du contenu (16 hexadécimaux).
    pub hash: String,
    /// Format détecté par le magic/JSON, pas par l'extension du nom.
    pub format: MeshFileFormat,
}

/// Erreurs d'import d'un maillage, avec des messages orientés créateur.
#[derive(Debug, thiserror::Error)]
pub enum MeshImportError {
    /// Nom vide, trop long, ou contenant un séparateur de chemin.
    #[error(
        "nom de modèle 3D invalide : « {0} ». Utilise un nom de 1 à 64 caractères sans « / », « \\ » ni « .. »."
    )]
    NomInvalide(String),
    /// Le contenu n'est ni un GLB ni un glTF JSON autoporteur.
    #[error("format 3D non reconnu : {0}")]
    FormatInvalide(String),
    /// Fichier vide.
    #[error("le modèle 3D est vide (0 octet) : sélectionne un fichier GLB ou glTF valide.")]
    FichierVide,
    /// Fichier au-delà de [`MAX_IMPORTED_MESH_BYTES`].
    #[error(
        "modèle 3D trop volumineux : {recu} octets pour un plafond de {max} octets par maillage. Allège le modèle dans un outil externe (moins de sommets/textures) avant de réessayer."
    )]
    TailleDepassee {
        /// Taille reçue.
        recu: u64,
        /// Plafond par maillage.
        max: u64,
    },
    /// Un fichier du même nom existe déjà dans `assets/meshes/`.
    #[error(
        "un modèle « {0} » existe déjà dans assets/meshes/. Renomme le modèle ou supprime l'ancien fichier."
    )]
    ModeleDejaPresent(String),
    /// Base64 illisible.
    #[error("contenu base64 illisible : {0}")]
    Base64(String),
    /// Écriture disque impossible.
    #[error("écriture du modèle 3D impossible : {0}")]
    Ecriture(String),
    /// Refus du manifeste (budget, identifiant dupliqué, etc.).
    #[error(transparent)]
    Manifeste(#[from] ManifestError),
}

/// Décode le base64 envoyé par le Studio (IPC Tauri).
pub fn decoder_base64(texte: &str) -> Result<Vec<u8>, MeshImportError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(texte.trim())
        .map_err(|erreur| MeshImportError::Base64(erreur.to_string()))
}

/// Détecte le format d'après le **contenu**, jamais d'après l'extension.
pub fn detecter_format(bytes: &[u8]) -> Result<MeshFileFormat, MeshImportError> {
    if bytes.is_empty() {
        return Err(MeshImportError::FichierVide);
    }
    if bytes.starts_with(b"glTF") {
        valider_glb(bytes)?;
        return Ok(MeshFileFormat::Glb);
    }
    let premier = bytes
        .iter()
        .copied()
        .find(|octet| !octet.is_ascii_whitespace());
    if premier == Some(b'{') {
        valider_gltf_json(bytes)?;
        return Ok(MeshFileFormat::Gltf);
    }
    Err(MeshImportError::FormatInvalide(
        "le fichier n'est ni un GLB (magic « glTF ») ni un glTF JSON. Exporte le modèle en .glb ou en .gltf autoporteur.".to_string(),
    ))
}

/// Vérifie l'en-tête GLB (magic, version 2, longueur et chunk JSON).
fn valider_glb(bytes: &[u8]) -> Result<(), MeshImportError> {
    if bytes.len() < 20 {
        return Err(MeshImportError::FormatInvalide(
            "GLB tronqué : l'en-tête de 12 octets et le chunk JSON sont requis.".to_string(),
        ));
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != 2 {
        return Err(MeshImportError::FormatInvalide(format!(
            "version GLB {version} non prise en charge : seul glTF 2.0 est accepté."
        )));
    }
    let longueur = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if longueur != bytes.len() {
        return Err(MeshImportError::FormatInvalide(format!(
            "GLB incohérent : l'en-tête annonce {longueur} octets, le fichier en fait {}.",
            bytes.len()
        )));
    }
    let chunk_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let chunk_type = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    const JSON_CHUNK: u32 = 0x4E4F_534A; // "JSON"
    if chunk_type != JSON_CHUNK || chunk_len == 0 {
        return Err(MeshImportError::FormatInvalide(
            "GLB sans chunk JSON en tête de fichier.".to_string(),
        ));
    }
    let fin_json = 20usize.saturating_add(chunk_len);
    if fin_json > bytes.len() {
        return Err(MeshImportError::FormatInvalide(
            "GLB tronqué : le chunk JSON dépasse la fin du fichier.".to_string(),
        ));
    }
    let json = &bytes[20..fin_json];
    valider_version_gltf_json(json)
}

/// Vérifie qu'un glTF JSON déclare bien `asset.version = 2.x`.
fn valider_gltf_json(bytes: &[u8]) -> Result<(), MeshImportError> {
    let valeur: serde_json::Value = serde_json::from_slice(bytes).map_err(|erreur| {
        MeshImportError::FormatInvalide(format!("glTF JSON illisible : {erreur}"))
    })?;
    verifier_version_et_autonomie(&valeur)?;
    Ok(())
}

fn valider_version_gltf_json(json: &[u8]) -> Result<(), MeshImportError> {
    let valeur: serde_json::Value = serde_json::from_slice(json).map_err(|erreur| {
        MeshImportError::FormatInvalide(format!(
            "le chunk JSON du GLB n'est pas un glTF valide : {erreur}"
        ))
    })?;
    verifier_version_et_autonomie(&valeur)?;
    Ok(())
}

/// Contrôle la version glTF 2.0 et refuse les ressources externes.
///
/// Un `.gltf` multi-fichiers ne peut pas être enregistré tel quel : copier un
/// seul fichier laisserait des buffers/textures manquants au prochain export.
fn verifier_version_et_autonomie(valeur: &serde_json::Value) -> Result<(), MeshImportError> {
    let version = valeur
        .get("asset")
        .and_then(|asset| asset.get("version"))
        .and_then(|version| version.as_str())
        .unwrap_or("");
    if !version.starts_with("2.") {
        return Err(MeshImportError::FormatInvalide(format!(
            "version glTF « {version} » non prise en charge : seul glTF 2.0 est accepté."
        )));
    }
    for section in ["buffers", "images"] {
        if let Some(liste) = valeur.get(section).and_then(|v| v.as_array()) {
            for ressource in liste {
                let uri = ressource.get("uri").and_then(|uri| uri.as_str());
                match uri {
                    None | Some("") => {}
                    Some(uri) if uri.starts_with("data:") => {}
                    Some(uri) => {
                        return Err(MeshImportError::FormatInvalide(format!(
                            "glTF multi-fichiers : « {uri} » est une ressource externe. Exporte un .glb (ou un .gltf avec buffers/textures embarqués en data:) avant l'import."
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Normalise un nom de fichier en identifiant d'asset stable (`[a-z0-9_-]`).
///
/// Les accents français courants sont translittérés ; tout autre caractère
/// devient `_`. Un nom vide, trop long ou contenant un séparateur de chemin est
/// refusé plutôt que silencieusement transformé en autre chose.
pub fn normaliser_nom_mesh(nom: &str) -> Result<String, MeshImportError> {
    let nom = nom.trim();
    if nom.is_empty()
        || nom.contains('/')
        || nom.contains('\\')
        || nom.contains("..")
        || nom.contains('\0')
    {
        return Err(MeshImportError::NomInvalide(nom.to_string()));
    }
    let sans_extension = nom
        .strip_suffix(".glb")
        .or_else(|| nom.strip_suffix(".GLB"))
        .or_else(|| nom.strip_suffix(".gltf"))
        .or_else(|| nom.strip_suffix(".GLTF"))
        .unwrap_or(nom);
    let mut id = String::new();
    for caractere in sans_extension.chars() {
        let remplace = match caractere {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'À' | 'Á' | 'Â' | 'Ä' | 'Ã' | 'Å' => Some('a'),
            'ç' | 'Ç' => Some('c'),
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => Some('e'),
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => Some('i'),
            'ñ' | 'Ñ' => Some('n'),
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'Ò' | 'Ó' | 'Ô' | 'Ö' | 'Õ' => Some('o'),
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => Some('u'),
            'ý' | 'ÿ' | 'Ý' | 'Ÿ' => Some('y'),
            autre => Some(autre),
        };
        let c = remplace.unwrap_or(caractere).to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            id.push(c);
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    let id = id
        .trim_matches(|caractere| caractere == '_' || caractere == '-')
        .to_string();
    if id.is_empty() {
        return Err(MeshImportError::NomInvalide(nom.to_string()));
    }
    if id.len() > 64 {
        return Err(MeshImportError::NomInvalide(nom.to_string()));
    }
    Ok(id)
}

/// Date ISO courte `AAAA-MM-JJ` (UTC), sans dépendance d'horloge externe.
///
/// Elle est figée à l'import pour que deux imports du même projet produisent un
/// manifeste stable. La conversion jours → date suit l'algorithme civil de
/// Howard Hinnant (aucune allocation, aucune dépendance).
pub fn date_iso_aujourdhui() -> String {
    let secondes = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duree| duree.as_secs())
        .unwrap_or(0);
    date_depuis_secondes(secondes)
}

fn date_depuis_secondes(secondes: u64) -> String {
    let jours = (secondes / 86_400) as i64;
    // Algorithme civil_from_days (Hinnant), valable pour les dates modernes.
    let z = jours + 719_468;
    let ere = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let annee = yoe + ere * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = if mois <= 2 { annee + 1 } else { annee };
    format!("{annee:04}-{mois:02}-{jour:02}")
}

/// Écrit `assets/meshes/<id>.<ext>` sous `racine` et enregistre l'`AssetEntry`.
///
/// Ordre des contrôles : nom → base64 déjà décodé → format (magic) → taille →
/// collision de fichier → écriture → budgets du manifeste. Un refus du manifeste
/// supprime le fichier écrit afin de ne jamais laisser d'orphelin sur disque.
///
/// `licence` est optionnelle ; son absence produit un **avertissement** à la
/// validation, jamais un blocage (voir [`AssetsManifest::validate`]).
pub fn enregistrer_mesh_importe(
    racine: &Path,
    manifeste: &mut AssetsManifest,
    nom: &str,
    bytes: &[u8],
    licence: Option<&str>,
    cree_le: &str,
) -> Result<ImportedMesh, MeshImportError> {
    let id = normaliser_nom_mesh(nom)?;
    let format = detecter_format(bytes)?;
    let taille = bytes.len() as u64;
    if taille > MAX_IMPORTED_MESH_BYTES {
        return Err(MeshImportError::TailleDepassee {
            recu: taille,
            max: MAX_IMPORTED_MESH_BYTES,
        });
    }

    let relatif = format!("assets/meshes/{id}.{}", format.extension());
    let complet = racine.join(&relatif);
    if complet.exists() {
        return Err(MeshImportError::ModeleDejaPresent(id));
    }
    if let Some(parent) = complet.parent() {
        fs::create_dir_all(parent)
            .map_err(|erreur| MeshImportError::Ecriture(erreur.to_string()))?;
    }
    fs::write(&complet, bytes).map_err(|erreur| MeshImportError::Ecriture(erreur.to_string()))?;

    let licence = licence.map(str::trim).filter(|valeur| !valeur.is_empty());
    let entree = AssetEntry {
        id: id.clone(),
        kind: AssetKind::Mesh,
        path: relatif.clone(),
        bytes: taille,
        hash: hash_text(bytes),
        provenance: Provenance {
            source: ProvenanceSource::Imported {
                tool: "host".to_string(),
            },
            license: licence.map(str::to_string),
            created_at: cree_le.to_string(),
        },
        dimensions: None,
        duration_ms: None,
    };
    if let Err(erreur) = manifeste.register(entree.clone()) {
        // Pas de fichier orphelin si le manifeste refuse (budget, doublon…).
        let _ = fs::remove_file(&complet);
        return Err(MeshImportError::Manifeste(erreur));
    }

    Ok(ImportedMesh {
        id,
        path: relatif,
        bytes: taille,
        hash: entree.hash,
        format,
    })
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

    // --- Import GLB/glTF (étape C2) -----------------------------------------

    use crate::assets_manifest::{AssetBudget, ManifestIssue};

    /// Dossier temporaire unique, supprimé automatiquement (`Drop`), y compris
    /// quand une assertion échoue.
    struct Bac {
        dir: std::path::PathBuf,
    }

    impl Bac {
        fn neuf(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duree| duree.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!(
                "karacti_mesh_{tag}_{}_{nanos}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("dossier temporaire créé");
            Self { dir }
        }
    }

    impl Drop for Bac {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    /// GLB minimal valide : 1 mesh, 1 triangle, 1 matériau, 0 animation.
    ///
    /// Structure binaire : en-tête de 12 octets (`glTF`, version 2, longueur),
    /// chunk JSON (type `JSON`, paddé d'espaces sur 4 octets), chunk BIN
    /// (type `BIN\0`) avec 3 positions `f32` (36 octets).
    fn glb_minimal() -> Vec<u8> {
        let json = r#"{
            "asset": {"version": "2.0", "generator": "karacti-test"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "meshes": [{"name": "Triangle", "primitives": [{"attributes": {"POSITION": 0}, "material": 0}]}],
            "materials": [{"name": "Orange", "pbrMetallicRoughness": {"baseColorFactor": [1.0, 0.5, 0.2, 1.0]}}],
            "buffers": [{"byteLength": 36}],
            "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}],
            "accessors": [{
                "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                "max": [1.0, 1.0, 0.0], "min": [0.0, 0.0, 0.0]
            }]
        }"#;
        let mut json_bytes = json.as_bytes().to_vec();
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let mut bin = Vec::with_capacity(36);
        for sommet in [
            [0.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ] {
            for composante in sommet {
                bin.extend_from_slice(&composante.to_le_bytes());
            }
        }
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&0u32.to_le_bytes()); // longueur totale, remplie après
        glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F_534Au32.to_le_bytes()); // "JSON"
        glb.extend_from_slice(&json_bytes);
        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E_4942u32.to_le_bytes()); // "BIN\0"
        glb.extend_from_slice(&bin);
        let total = glb.len() as u32;
        glb[8..12].copy_from_slice(&total.to_le_bytes());
        glb
    }

    #[test]
    fn importe_glb_minimal_et_manifeste_correctement() {
        let bac = Bac::neuf("glb");
        let glb = glb_minimal();
        let mut manifeste = AssetsManifest::default();

        let importe = enregistrer_mesh_importe(
            &bac.dir,
            &mut manifeste,
            "Robot.glb",
            &glb,
            Some("CC0-1.0"),
            "2026-11-17",
        )
        .expect("GLB minimal accepté");

        assert_eq!(importe.id, "robot", "nom normalisé en identifiant stable");
        assert_eq!(importe.path, "assets/meshes/robot.glb");
        assert_eq!(importe.format, MeshFileFormat::Glb);
        assert_eq!(importe.bytes, glb.len() as u64);
        assert_eq!(importe.hash, hash_text(&glb));

        let entree = manifeste.entry("robot").expect("entrée enregistrée");
        assert_eq!(entree.kind, AssetKind::Mesh);
        assert_eq!(entree.path, "assets/meshes/robot.glb");
        assert_eq!(entree.bytes, glb.len() as u64);
        assert_eq!(entree.hash, hash_text(&glb));
        assert_eq!(entree.hash.len(), 16, "FNV-1a 64 bits = 16 hexadécimaux");
        assert!(matches!(
            &entree.provenance.source,
            ProvenanceSource::Imported { tool } if tool == "host"
        ));
        assert_eq!(entree.provenance.license.as_deref(), Some("CC0-1.0"));
        assert_eq!(entree.provenance.created_at, "2026-11-17");
        assert!(bac.dir.join(&entree.path).is_file(), "fichier écrit sur disque");

        // Le manifeste relu depuis le disque doit être propre (taille + hash OK).
        let issues: Vec<ManifestIssue> = manifeste.validate(&bac.dir);
        assert!(issues.is_empty(), "aucun diagnostic : {issues:?}");
    }

    #[test]
    fn hash_stable_entre_deux_imports_du_meme_contenu() {
        let glb = glb_minimal();
        let bac_a = Bac::neuf("hash_a");
        let bac_b = Bac::neuf("hash_b");
        let mut manifeste_a = AssetsManifest::default();
        let mut manifeste_b = AssetsManifest::default();

        let a = enregistrer_mesh_importe(
            &bac_a.dir, &mut manifeste_a, "robot", &glb, None, "2026-11-17",
        )
        .expect("premier import");
        let b = enregistrer_mesh_importe(
            &bac_b.dir, &mut manifeste_b, "robot", &glb, None, "2026-11-17",
        )
        .expect("second import");

        assert_eq!(a.hash, b.hash, "mêmes octets → même hash");
        assert_eq!(a.bytes, b.bytes);
    }

    #[test]
    fn refus_budget_depasse_sans_fichier_orphelin() {
        let bac = Bac::neuf("budget");
        let glb = glb_minimal();
        let budget = AssetBudget {
            max_total_bytes: 32,
            ..AssetBudget::default()
        };
        let mut manifeste = AssetsManifest::new(budget);

        let erreur = enregistrer_mesh_importe(
            &bac.dir,
            &mut manifeste,
            "robot",
            &glb,
            Some("CC0-1.0"),
            "2026-11-17",
        )
        .expect_err("budget d'octets dépassé");

        assert!(matches!(
            erreur,
            MeshImportError::Manifeste(ManifestError::BudgetOctets { max: 32 })
        ));
        assert!(manifeste.entries.is_empty(), "manifeste non modifié");
        assert!(
            !bac.dir.join("assets/meshes/robot.glb").exists(),
            "le fichier écrit doit être supprimé quand le manifeste refuse"
        );
    }

    #[test]
    fn refus_nom_invalide_et_format_inconnu() {
        let bac = Bac::neuf("refus");
        let mut manifeste = AssetsManifest::default();
        let glb = glb_minimal();

        let erreur = enregistrer_mesh_importe(
            &bac.dir, &mut manifeste, "   ", &glb, None, "2026-11-17",
        )
        .expect_err("nom vide refusé");
        assert!(matches!(erreur, MeshImportError::NomInvalide(_)));

        let erreur = enregistrer_mesh_importe(
            &bac.dir, &mut manifeste, "../evasion", &glb, None, "2026-11-17",
        )
        .expect_err("séparateur de chemin refusé");
        assert!(matches!(erreur, MeshImportError::NomInvalide(_)));

        let erreur = enregistrer_mesh_importe(
            &bac.dir, &mut manifeste, "robot", b"pas un modele 3d", None, "2026-11-17",
        )
        .expect_err("format inconnu refusé");
        assert!(matches!(erreur, MeshImportError::FormatInvalide(_)));
    }

    #[test]
    fn refus_gltf_multi_fichiers_avec_message_explicite() {
        let bac = Bac::neuf("gltf_externe");
        let gltf = br#"{
            "asset": {"version": "2.0"},
            "buffers": [{"uri": "scene.bin", "byteLength": 36}],
            "meshes": []
        }"#;
        let mut manifeste = AssetsManifest::default();

        let erreur = enregistrer_mesh_importe(
            &bac.dir, &mut manifeste, "robot.gltf", gltf, None, "2026-11-17",
        )
        .expect_err("glTF multi-fichiers refusé");
        match erreur {
            MeshImportError::FormatInvalide(message) => {
                assert!(message.contains("multi-fichiers"), "message : {message}");
                assert!(message.contains("scene.bin"), "message : {message}");
            }
            autre => panic!("variante inattendue : {autre:?}"),
        }
    }

    #[test]
    fn glb_tronque_est_refuse() {
        let mut glb = glb_minimal();
        glb.truncate(glb.len() - 8);
        assert!(matches!(
            detecter_format(&glb),
            Err(MeshImportError::FormatInvalide(_))
        ));
    }

    #[test]
    fn date_iso_suit_le_format_attendu() {
        // 2026-11-17T00:00:00Z.
        assert_eq!(date_depuis_secondes(1_794_873_600), "2026-11-17");
        let aujourdhui = date_iso_aujourdhui();
        assert_eq!(aujourdhui.len(), 10, "AAAA-MM-JJ");
        assert!(aujourdhui.starts_with("20"), "année plausible : {aujourdhui}");
    }
}
