//! Manifeste d'assets — la **frontière de confiance** du pipeline P2.
//!
//! Tout contenu externe (sprite, planche, SVG, son, maillage, texture) est déclaré
//! avant d'être accepté : identifiant stable, chemin relatif au projet, taille,
//! hash de contenu, provenance et licence. Le manifeste est **borné** par un
//! [`AssetBudget`] (nombre d'assets, octets globaux, octets par texture) et
//! **vérifiable** contre le disque via [`AssetsManifest::validate`].
//!
//! ## Décisions de conception
//!
//! - Le hash réutilise [`crate::ai_dictionary::hash_text`] (FNV-1a 64 bits, 16
//!   hexadécimaux). C'est une empreinte d'**intégrité** et de reproductibilité,
//!   pas une primitive cryptographique : elle détecte une modification
//!   accidentelle, pas un adversaire.
//! - [`AssetsManifest::register`] est le seul chemin d'ajout : il refuse les
//!   identifiants invalides ou dupliqués, les chemins absolus ou contenant
//!   `..`, les fichiers vides et les dépassements de budget globaux.
//! - [`AssetsManifest::from_json`] fait une **lecture stricte** : il refuse une
//!   version absente ou inconnue, puis ré-enregistre chaque entrée dans un
//!   manifeste neuf. Un JSON forgé à la main ne peut donc pas contourner les
//!   invariants de `register`. Il renvoie [`ManifestError`] (et non
//!   `serde_json::Error`) pour exposer [`ManifestError::VersionInconnue`].
//! - [`AssetsManifest::validate`] ne bloque jamais sur la licence : c'est un
//!   **avertissement** (`blocking = false`) qui n'empêche pas l'export.
//! - Le champ `max_mesh_triangles` est conservé pour l'import 3D à venir ;
//!   [`AssetEntry`] n'expose pas encore le nombre de triangles, donc `validate`
//!   ne peut pas le contrôler aujourd'hui (voir P4).
//! - `max_texture_bytes` s'applique aux entrées de type
//!   [`AssetKind::Texture`] ; les sprites et planches relèvent du budget global.

use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::ai_dictionary::hash_text;

/// Version du format de manifeste lue et écrite par ce moteur.
pub const ASSETS_MANIFEST_VERSION: u32 = 1;

/// Nombre maximal d'assets déclarables par défaut.
const MAX_ASSETS_DEFAUT: usize = 256;
/// Budget d'octets global par défaut : 8 Mio.
const MAX_TOTAL_BYTES_DEFAUT: u64 = 8 * 1024 * 1024;
/// Budget d'octets par texture par défaut : 1 Mio.
const MAX_TEXTURE_BYTES_DEFAUT: u64 = 1024 * 1024;
/// Budget de triangles par maillage par défaut.
const MAX_MESH_TRIANGLES_DEFAUT: u64 = 50_000;

/// Type d'asset externe accepté par le manifeste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// Sprite isolé (une image, une frame).
    Sprite,
    /// Planche de sprites (plusieurs frames dans une image).
    Sheet,
    /// Image vectorielle SVG.
    Svg,
    /// Son ou musique.
    Audio,
    /// Maillage 3D (glTF/GLB ou procédural).
    Mesh,
    /// Texture appliquée à un maillage.
    Texture,
}

/// Plafonds du manifeste : ils rendent l'import **borné** et prévisible.
///
/// Les budgets sont appliqués à l'enregistrement ([`AssetsManifest::register`])
/// et re-vérifiés à la validation ([`AssetsManifest::validate`]), ce qui couvre
/// aussi un manifeste modifié à la main après chargement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBudget {
    /// Nombre maximal d'assets déclarés (défaut : 256).
    pub max_assets: usize,
    /// Somme maximale des octets déclarés (défaut : 8 Mio).
    pub max_total_bytes: u64,
    /// Octets maximaux pour une texture (défaut : 1 Mio).
    pub max_texture_bytes: u64,
    /// Triangles maximaux par maillage (défaut : 50 000).
    ///
    /// Conservé pour l'import 3D : `AssetEntry` n'expose pas encore ce nombre,
    /// donc la validation ne peut pas le contrôler pour l'instant.
    pub max_mesh_triangles: u64,
}

impl Default for AssetBudget {
    fn default() -> Self {
        Self {
            max_assets: MAX_ASSETS_DEFAUT,
            max_total_bytes: MAX_TOTAL_BYTES_DEFAUT,
            max_texture_bytes: MAX_TEXTURE_BYTES_DEFAUT,
            max_mesh_triangles: MAX_MESH_TRIANGLES_DEFAUT,
        }
    }
}

/// Origine d'un asset : outil, générateur ou Studio.
///
/// La provenance est **obligatoire** et non négociable : c'est elle qui rend un
/// contenu reproductible et auditable. Le JSON utilise un champ `type`
/// (`studio`, `pxl`, `generated`, `imported`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProvenanceSource {
    /// Produit directement dans le Studio.
    Studio,
    /// Pixel-art généré par `pxl`.
    Pxl,
    /// Généré par un modèle (local ou non) : le trio modèle/prompt/graine est
    /// ce qui permet de rejouer la génération.
    Generated {
        /// Modèle utilisé, par exemple `pxl-local` ou `sd-turbo`.
        model: String,
        /// Prompt exact soumis au modèle.
        prompt: String,
        /// Graine de génération, si le modèle en accepte une.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
    },
    /// Importé depuis un outil externe (Blender, Tiled, Aseprite…).
    Imported {
        /// Nom de l'outil d'origine.
        tool: String,
    },
}

/// Provenance complète d'un asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Qui ou quoi a produit le contenu.
    pub source: ProvenanceSource,
    /// Licence déclarée, si elle est connue. Absente = avertissement de validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Date ISO courte `AAAA-MM-JJ`, **figée par l'appelant** (pas d'horloge ici,
    /// pour que deux imports du même projet donnent le même JSON).
    pub created_at: String,
}

/// Une entrée du manifeste : un asset déclaré, traçable et borné.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetEntry {
    /// Identifiant stable et unique, limité à `[a-z0-9_-]` (64 caractères max).
    pub id: String,
    /// Type d'asset.
    pub kind: AssetKind,
    /// Chemin relatif au projet, en `/`, par exemple `assets/sprites/hero.png`.
    pub path: String,
    /// Taille déclarée en octets (strictement positive).
    pub bytes: u64,
    /// Empreinte du contenu, produite par `hash_text` (16 hexadécimaux).
    pub hash: String,
    /// Provenance et licence.
    pub provenance: Provenance,
    /// Dimensions `(largeur, hauteur)` pour les images uniquement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<(u32, u32)>,
    /// Durée en millisecondes pour l'audio uniquement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
}

/// Manifeste d'assets versionné.
///
/// Construire avec [`AssetsManifest::new`] (ou `Default`) puis ajouter les
/// entrées avec [`AssetsManifest::register`] — jamais en modifiant `entries`
/// directement, sous peine de contourner les budgets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetsManifest {
    /// Version du format ; doit valoir [`ASSETS_MANIFEST_VERSION`].
    pub version: u32,
    /// Plafonds appliqués au manifeste.
    pub budget: AssetBudget,
    /// Entrées déclarées, dans l'ordre d'enregistrement.
    pub entries: Vec<AssetEntry>,
}

impl AssetsManifest {
    /// Crée un manifeste vide avec la version courante et le budget fourni.
    pub fn new(budget: AssetBudget) -> Self {
        Self {
            version: ASSETS_MANIFEST_VERSION,
            budget,
            entries: Vec::new(),
        }
    }

    /// Enregistre un asset après avoir vérifié les invariants du manifeste.
    ///
    /// Refuse, dans cet ordre : identifiant invalide, chemin invalide, fichier
    /// vide, identifiant dupliqué, budget d'assets dépassé, budget d'octets
    /// dépassé. La taille et le hash ne sont pas vérifiés ici (aucune lecture
    /// disque) : c'est le rôle de [`AssetsManifest::validate`].
    pub fn register(&mut self, entry: AssetEntry) -> Result<(), ManifestError> {
        if !id_valide(&entry.id) {
            return Err(ManifestError::IdInvalide(entry.id));
        }
        if !chemin_valide(&entry.path) {
            return Err(ManifestError::CheminInvalide(entry.path));
        }
        if entry.bytes == 0 {
            return Err(ManifestError::FichierVide(entry.id));
        }
        if self.entries.iter().any(|existante| existante.id == entry.id) {
            return Err(ManifestError::IdDuplique(entry.id));
        }
        if self.entries.len() >= self.budget.max_assets {
            return Err(ManifestError::BudgetAssets {
                max: self.budget.max_assets,
            });
        }
        if self.total_bytes().saturating_add(entry.bytes) > self.budget.max_total_bytes {
            return Err(ManifestError::BudgetOctets {
                max: self.budget.max_total_bytes,
            });
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Retrouve une entrée par identifiant.
    pub fn entry(&self, id: &str) -> Option<&AssetEntry> {
        self.entries.iter().find(|entree| entree.id == id)
    }

    /// Somme des octets déclarés, en saturant plutôt qu'en débordant.
    pub fn total_bytes(&self) -> u64 {
        self.entries
            .iter()
            .fold(0u64, |total, entree| total.saturating_add(entree.bytes))
    }

    /// Vérifie le manifeste contre le disque, sous la racine `root`.
    ///
    /// Chaque entrée produit zéro ou plusieurs [`ManifestIssue`] :
    /// - `chemin-invalide` (bloquant) : chemin absolu, `..` ou séparateur `\` —
    ///   présent seulement dans un JSON forgé, puisque `register` les refuse ;
    /// - `fichier-absent` (bloquant) : fichier introuvable ou illisible ;
    /// - `taille-different` (bloquant) : taille réelle ≠ taille déclarée ;
    /// - `hash-incoherent` (bloquant) : contenu modifié depuis l'enregistrement ;
    /// - `budget-depasse` (bloquant) : assets, octets globaux ou octets de
    ///   texture au-delà du budget ;
    /// - `licence-manquante` (**avertissement**) : licence absente ou vide.
    ///
    /// La licence ne bloque jamais : elle signale un risque juridique, pas un
    /// projet non exportable techniquement.
    pub fn validate(&self, root: &Path) -> Vec<ManifestIssue> {
        let mut issues = Vec::new();

        for entree in &self.entries {
            if !chemin_valide(&entree.path) {
                issues.push(ManifestIssue::bloquant(
                    "chemin-invalide",
                    format!(
                        "« {} » : le chemin « {} » doit être relatif au projet, en « / », sans `..` ni racine absolue.",
                        entree.id, entree.path
                    ),
                ));
            } else {
                let complet = root.join(&entree.path);
                match verify_file(&complet) {
                    Ok((taille, hash)) => {
                        if taille != entree.bytes {
                            issues.push(ManifestIssue::bloquant(
                                "taille-different",
                                format!(
                                    "« {} » : « {} » pèse {} octets alors que le manifeste en déclare {}. Réenregistre l'asset dans le Studio.",
                                    entree.id, entree.path, taille, entree.bytes
                                ),
                            ));
                        }
                        if hash != entree.hash {
                            issues.push(ManifestIssue::bloquant(
                                "hash-incoherent",
                                format!(
                                    "« {} » : le contenu de « {} » ne correspond pas au hash déclaré (attendu {}, trouvé {}). Le fichier a été modifié ou le manifeste est périmé.",
                                    entree.id, entree.path, entree.hash, hash
                                ),
                            ));
                        }
                    }
                    Err(erreur) => {
                        let cause = if erreur.kind() == std::io::ErrorKind::NotFound {
                            "est introuvable".to_string()
                        } else {
                            format!("est illisible ({erreur})")
                        };
                        issues.push(ManifestIssue::bloquant(
                            "fichier-absent",
                            format!(
                                "« {} » : le fichier « {} » {} sous « {} ». Ajoute-le au projet ou retire l'asset du manifeste.",
                                entree.id,
                                entree.path,
                                cause,
                                root.display()
                            ),
                        ));
                    }
                }

                if entree.kind == AssetKind::Texture
                    && entree.bytes > self.budget.max_texture_bytes
                {
                    issues.push(ManifestIssue::bloquant(
                        "budget-depasse",
                        format!(
                            "texture « {} » : {} pour un plafond de {} par texture. Réduis la résolution ou augmente le budget.",
                            entree.id,
                            octets_lisibles(entree.bytes),
                            octets_lisibles(self.budget.max_texture_bytes)
                        ),
                    ));
                }
            }

            if licence_absente(&entree.provenance) {
                issues.push(ManifestIssue::avertissement(
                    "licence-manquante",
                    format!(
                        "« {} » : aucune licence déclarée. Renseigne une licence (même « inconnue ») pour garder le projet publiable.",
                        entree.id
                    ),
                ));
            }
        }

        if self.entries.len() > self.budget.max_assets {
            issues.push(ManifestIssue::bloquant(
                "budget-depasse",
                format!(
                    "budget d'assets dépassé : {} assets déclarés pour un plafond de {}. Supprime un asset ou augmente le budget.",
                    self.entries.len(),
                    self.budget.max_assets
                ),
            ));
        }

        let total = self.total_bytes();
        if total > self.budget.max_total_bytes {
            issues.push(ManifestIssue::bloquant(
                "budget-depasse",
                format!(
                    "budget d'octets dépassé : {} déclarés pour un plafond de {}. Réduis les assets ou augmente le budget.",
                    octets_lisibles(total),
                    octets_lisibles(self.budget.max_total_bytes)
                ),
            ));
        }

        issues
    }

    /// Sérialise le manifeste en JSON lisible (`pretty`), champs optionnels omis.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Charge un manifeste depuis un fichier JSON, avec la même lecture stricte
    /// que [`AssetsManifest::from_json`].
    ///
    /// Le manifeste n'est **pas** validé contre le disque ici : seul le format
    /// est contrôlé. Utiliser [`AssetsManifest::validate`] pour vérifier
    /// l'existence et l'empreinte des fichiers référencés.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let contenu = fs::read_to_string(path)?;
        Self::from_json(&contenu)
    }

    /// Relit un manifeste JSON avec une **lecture stricte**.
    ///
    /// Refuse une version absente (`VersionInconnue(0)`) ou différente de
    /// [`ASSETS_MANIFEST_VERSION`], puis ré-enregistre chaque entrée : les
    /// identifiants, chemins et budgets sont donc revalidés comme à l'import.
    /// Renvoie [`ManifestError`] plutôt que `serde_json::Error` afin d'exposer
    /// l'erreur de version dédiée ; les erreurs de syntaxe JSON restent
    /// accessibles via [`ManifestError::Json`].
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        #[derive(Deserialize)]
        struct VersionSeule {
            #[serde(default)]
            version: Option<u32>,
        }

        let sonde: VersionSeule = serde_json::from_str(json)?;
        match sonde.version {
            None => return Err(ManifestError::VersionInconnue(0)),
            Some(version) if version != ASSETS_MANIFEST_VERSION => {
                return Err(ManifestError::VersionInconnue(version));
            }
            Some(_) => {}
        }

        let lu: Self = serde_json::from_str(json)?;
        let mut manifeste = Self::new(lu.budget);
        for entree in lu.entries {
            manifeste.register(entree)?;
        }
        Ok(manifeste)
    }
}

impl Default for AssetsManifest {
    fn default() -> Self {
        Self::new(AssetBudget::default())
    }
}

/// Diagnostic unitaire produit par [`AssetsManifest::validate`].
///
/// `code` est un identifiant technique stable et sans accent, destiné au Studio ;
/// `message` est rédigé pour le créateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestIssue {
    /// Code technique : `fichier-absent`, `hash-incoherent`, `taille-different`,
    /// `budget-depasse`, `licence-manquante` ou `chemin-invalide`.
    pub code: &'static str,
    /// `true` si le problème empêche un import/export fiable.
    pub blocking: bool,
    /// Message lisible en français.
    pub message: String,
}

impl ManifestIssue {
    /// Construit un diagnostic bloquant.
    fn bloquant(code: &'static str, message: String) -> Self {
        Self {
            code,
            blocking: true,
            message,
        }
    }

    /// Construit un diagnostic non bloquant.
    fn avertissement(code: &'static str, message: String) -> Self {
        Self {
            code,
            blocking: false,
            message,
        }
    }
}

/// Erreurs du manifeste, avec des messages orientés créateur.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// Identifiant vide, trop long ou contenant des caractères interdits.
    #[error(
        "identifiant d'asset invalide « {0} » : de 1 à 64 caractères parmi a-z, 0-9, _ et - (ex. hero_idle)."
    )]
    IdInvalide(String),
    /// Chemin vide, absolu, contenant `..` ou un séparateur `\`.
    #[error(
        "chemin d'asset invalide « {0} » : utilise un chemin relatif au projet, en « / », sans `..` (ex. assets/sprites/hero.png)."
    )]
    CheminInvalide(String),
    /// Identifiant déjà présent dans le manifeste.
    #[error("identifiant d'asset déjà utilisé : « {0} ». Chaque asset doit avoir un identifiant unique.")]
    IdDuplique(String),
    /// Fichier de taille nulle.
    #[error("l'asset « {0} » est vide (0 octet) : un fichier vide ne peut pas être référencé.")]
    FichierVide(String),
    /// Trop d'assets pour le budget.
    #[error(
        "budget d'assets dépassé : le manifeste accepte au plus {max} assets. Supprime un asset ou augmente le budget."
    )]
    BudgetAssets {
        /// Plafond configuré.
        max: usize,
    },
    /// Trop d'octets pour le budget global.
    #[error(
        "budget d'octets dépassé : ce fichier ferait franchir le plafond de {max} octets. Réduis les assets ou augmente le budget."
    )]
    BudgetOctets {
        /// Plafond configuré.
        max: u64,
    },
    /// Version de format absente (`0`) ou inconnue du moteur.
    #[error(
        "version de manifeste inconnue : {0} (le moteur lit la version 1 ; la valeur 0 signifie que le champ `version` est absent)."
    )]
    VersionInconnue(u32),
    /// Fichier de manifeste illisible (absent, permission refusée…).
    #[error("manifeste illisible : {0}")]
    Io(#[from] std::io::Error),
    /// JSON syntaxiquement invalide ou structurellement incorrect.
    #[error("manifeste JSON illisible : {0}")]
    Json(#[from] serde_json::Error),
}

/// Calcule la taille et le hash d'un fichier existant.
///
/// C'est la brique de comparaison du manifeste : deux imports du même fichier
/// donnent le même couple `(octets, hash)`.
pub fn verify_file(path: &Path) -> std::io::Result<(u64, String)> {
    let contenu = fs::read(path)?;
    Ok((contenu.len() as u64, hash_text(&contenu)))
}

/// Un identifiant est valide s'il est non vide, court et limité à `[a-z0-9_-]`.
fn id_valide(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Un chemin est valide s'il est relatif, portable et confiné au projet.
fn chemin_valide(chemin: &str) -> bool {
    if chemin.is_empty() || chemin.contains('\\') || chemin.contains('\0') {
        return false;
    }
    let path = Path::new(chemin);
    if path.is_absolute() {
        return false;
    }
    let mut au_moins_un_segment = false;
    for composant in path.components() {
        match composant {
            Component::Normal(_) => au_moins_un_segment = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    au_moins_un_segment
}

/// `true` si la licence est absente ou réduite à des espaces.
fn licence_absente(provenance: &Provenance) -> bool {
    provenance
        .license
        .as_deref()
        .map(str::trim)
        .map(str::is_empty)
        .unwrap_or(true)
}

/// Formate un nombre d'octets pour un message créateur, en Mio si utile.
fn octets_lisibles(octets: u64) -> String {
    if octets >= 1024 * 1024 {
        format!("{:.1} Mio", octets as f64 / (1024.0 * 1024.0)).replace('.', ",")
    } else if octets >= 1024 {
        format!("{:.1} Kio", octets as f64 / 1024.0).replace('.', ",")
    } else {
        format!("{octets} octets")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Dossier temporaire unique, supprimé automatiquement (même en cas d'échec
    /// d'assertion, grâce à `Drop`).
    struct Bac {
        dir: PathBuf,
    }

    impl Bac {
        fn neuf(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duree| duree.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!(
                "karacti_manifeste_{tag}_{}_{nanos}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("dossier temporaire créé");
            Self { dir }
        }

        fn ecrire(&self, rel: &str, octets: &[u8]) -> PathBuf {
            let chemin = self.dir.join(rel);
            if let Some(parent) = chemin.parent() {
                fs::create_dir_all(parent).expect("sous-dossier temporaire créé");
            }
            fs::write(&chemin, octets).expect("fichier temporaire écrit");
            chemin
        }
    }

    impl Drop for Bac {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    /// Entrée de test avec licence renseignée (donc sans avertissement).
    fn entree(id: &str, kind: AssetKind, path: &str, octets: &[u8]) -> AssetEntry {
        AssetEntry {
            id: id.to_string(),
            kind,
            path: path.to_string(),
            bytes: octets.len() as u64,
            hash: hash_text(octets),
            provenance: Provenance {
                source: ProvenanceSource::Studio,
                license: Some("CC0-1.0".to_string()),
                created_at: "2026-11-11".to_string(),
            },
            dimensions: None,
            duration_ms: None,
        }
    }

    #[test]
    fn budget_par_defaut_conforme_au_plan() {
        let budget = AssetBudget::default();
        assert_eq!(budget.max_assets, 256);
        assert_eq!(budget.max_total_bytes, 8 * 1024 * 1024);
        assert_eq!(budget.max_texture_bytes, 1024 * 1024);
        assert_eq!(budget.max_mesh_triangles, 50_000);
        assert_eq!(
            AssetsManifest::default().version,
            ASSETS_MANIFEST_VERSION
        );
    }

    #[test]
    fn hash_stable_et_sensible_au_contenu() {
        let bac = Bac::neuf("hash");
        let chemin = bac.ecrire("assets/hero.bin", b"karacti");
        let (taille1, hash1) = verify_file(&chemin).expect("premier hash");
        let (taille2, hash2) = verify_file(&chemin).expect("second hash");
        assert_eq!(taille1, 7);
        assert_eq!(taille1, taille2);
        assert_eq!(hash1, hash2, "mêmes octets → même hash");
        assert_eq!(hash1.len(), 16);

        let autre = bac.ecrire("assets/hero_autre.bin", b"karacti!");
        let (_, hash3) = verify_file(&autre).expect("hash du second fichier");
        assert_ne!(hash1, hash3, "octets différents → hash différent");
    }

    #[test]
    fn fichiers_temporaires_supprimes() {
        let bac = Bac::neuf("nettoyage");
        let dir = bac.dir.clone();
        let chemin = bac.ecrire("assets/hero.png", b"png");
        assert!(chemin.exists());
        drop(bac);
        assert!(!dir.exists(), "le dossier temporaire doit être supprimé");
    }

    #[test]
    fn register_refuse_un_id_duplique() {
        let mut manifeste = AssetsManifest::default();
        manifeste
            .register(entree("hero", AssetKind::Sprite, "assets/hero.png", b"a"))
            .expect("première entrée acceptée");
        let erreur = manifeste
            .register(entree(
                "hero",
                AssetKind::Sprite,
                "assets/hero2.png",
                b"b",
            ))
            .expect_err("doublon refusé");
        assert!(matches!(erreur, ManifestError::IdDuplique(ref id) if id == "hero"));
    }

    #[test]
    fn register_refuse_un_id_invalide() {
        let mut manifeste = AssetsManifest::default();
        let erreur = manifeste
            .register(entree("Héros", AssetKind::Sprite, "assets/hero.png", b"a"))
            .expect_err("identifiant accentué refusé");
        assert!(matches!(erreur, ManifestError::IdInvalide(ref id) if id == "Héros"));

        let erreur = manifeste
            .register(entree("hero.png", AssetKind::Sprite, "assets/hero.png", b"a"))
            .expect_err("point interdit");
        assert!(matches!(erreur, ManifestError::IdInvalide(_)));
    }

    #[test]
    fn register_refuse_un_chemin_avec_parent() {
        let mut manifeste = AssetsManifest::default();
        let erreur = manifeste
            .register(entree(
                "hero",
                AssetKind::Sprite,
                "assets/../secret.png",
                b"a",
            ))
            .expect_err("`..` refusé");
        assert!(matches!(erreur, ManifestError::CheminInvalide(ref chemin) if chemin == "assets/../secret.png"));
    }

    #[test]
    fn register_refuse_un_chemin_absolu() {
        let mut manifeste = AssetsManifest::default();
        let erreur = manifeste
            .register(entree(
                "hero",
                AssetKind::Sprite,
                "/tmp/hero.png",
                b"a",
            ))
            .expect_err("chemin absolu refusé");
        assert!(matches!(erreur, ManifestError::CheminInvalide(_)));

        let erreur = manifeste
            .register(entree(
                "hero2",
                AssetKind::Sprite,
                "assets\\hero.png",
                b"a",
            ))
            .expect_err("séparateur Windows refusé");
        assert!(matches!(erreur, ManifestError::CheminInvalide(_)));
    }

    #[test]
    fn register_refuse_un_fichier_vide() {
        let mut manifeste = AssetsManifest::default();
        let erreur = manifeste
            .register(entree("vide", AssetKind::Audio, "assets/vide.wav", b""))
            .expect_err("fichier vide refusé");
        assert!(matches!(erreur, ManifestError::FichierVide(ref id) if id == "vide"));
    }

    #[test]
    fn budget_assets_refuse_avec_la_bonne_variante() {
        let budget = AssetBudget {
            max_assets: 1,
            ..AssetBudget::default()
        };
        let mut manifeste = AssetsManifest::new(budget);
        manifeste
            .register(entree("un", AssetKind::Sprite, "assets/un.png", b"a"))
            .expect("premier asset accepté");
        let erreur = manifeste
            .register(entree("deux", AssetKind::Sprite, "assets/deux.png", b"b"))
            .expect_err("budget d'assets dépassé");
        assert!(matches!(erreur, ManifestError::BudgetAssets { max: 1 }));
    }

    #[test]
    fn budget_octets_refuse_avec_la_bonne_variante() {
        let budget = AssetBudget {
            max_total_bytes: 10,
            ..AssetBudget::default()
        };
        let mut manifeste = AssetsManifest::new(budget);
        manifeste
            .register(entree("six", AssetKind::Audio, "assets/six.wav", b"123456"))
            .expect("6 octets acceptés");
        let erreur = manifeste
            .register(entree("cinq", AssetKind::Audio, "assets/cinq.wav", b"12345"))
            .expect_err("budget d'octets dépassé");
        assert!(matches!(erreur, ManifestError::BudgetOctets { max: 10 }));
        assert_eq!(manifeste.total_bytes(), 6);
    }

    #[test]
    fn validate_detecte_un_fichier_absent() {
        let bac = Bac::neuf("absent");
        let mut manifeste = AssetsManifest::default();
        manifeste
            .register(entree(
                "hero",
                AssetKind::Sprite,
                "assets/sprites/absent.png",
                b"png",
            ))
            .expect("entrée acceptée");
        let issues = manifeste.validate(&bac.dir);
        let issue = issues
            .iter()
            .find(|issue| issue.code == "fichier-absent")
            .expect("fichier absent signalé");
        assert!(issue.blocking);
        assert!(issue.message.contains("absent.png"));
        assert!(issues.iter().all(|issue| issue.code != "hash-incoherent"));
    }

    #[test]
    fn validate_detecte_un_hash_incoherent() {
        let bac = Bac::neuf("hash_incoherent");
        bac.ecrire("assets/hero.png", b"v1");
        let mut manifeste = AssetsManifest::default();
        manifeste
            .register(entree("hero", AssetKind::Sprite, "assets/hero.png", b"v1"))
            .expect("entrée acceptée");
        // Même longueur : seul le hash doit diverger.
        bac.ecrire("assets/hero.png", b"v2");
        let issues = manifeste.validate(&bac.dir);
        let issue = issues
            .iter()
            .find(|issue| issue.code == "hash-incoherent")
            .expect("hash incohérent signalé");
        assert!(issue.blocking);
        assert!(!issues.iter().any(|issue| issue.code == "taille-different"));
    }

    #[test]
    fn validate_detecte_une_taille_differente() {
        let bac = Bac::neuf("taille");
        bac.ecrire("assets/hero.png", b"v1");
        let mut manifeste = AssetsManifest::default();
        manifeste
            .register(entree("hero", AssetKind::Sprite, "assets/hero.png", b"v1"))
            .expect("entrée acceptée");
        bac.ecrire("assets/hero.png", b"v1 plus long");
        let issues = manifeste.validate(&bac.dir);
        assert!(issues.iter().any(|issue| issue.code == "taille-different" && issue.blocking));
    }

    #[test]
    fn validate_signale_une_licence_manquante_en_avertissement() {
        let bac = Bac::neuf("licence");
        bac.ecrire("assets/hero.png", b"png");
        let mut manifeste = AssetsManifest::default();
        let mut sans_licence = entree("hero", AssetKind::Sprite, "assets/hero.png", b"png");
        sans_licence.provenance.license = None;
        manifeste.register(sans_licence).expect("entrée acceptée");

        let issues = manifeste.validate(&bac.dir);
        let issue = issues
            .iter()
            .find(|issue| issue.code == "licence-manquante")
            .expect("licence manquante signalée");
        assert!(!issue.blocking, "la licence est un avertissement");
        assert!(issues.iter().all(|issue| !issue.blocking), "aucun blocage attendu");
    }

    #[test]
    fn validate_signale_un_budget_texture_depasse() {
        let bac = Bac::neuf("texture");
        bac.ecrire("assets/texture.png", b"0123456789");
        let budget = AssetBudget {
            max_texture_bytes: 4,
            ..AssetBudget::default()
        };
        let mut manifeste = AssetsManifest::new(budget);
        manifeste
            .register(entree(
                "texture",
                AssetKind::Texture,
                "assets/texture.png",
                b"0123456789",
            ))
            .expect("entrée acceptée");
        let issues = manifeste.validate(&bac.dir);
        assert!(issues.iter().any(|issue| issue.code == "budget-depasse" && issue.blocking));
    }

    #[test]
    fn roundtrip_json_complet_avec_provenance_generee() {
        let budget = AssetBudget {
            max_assets: 32,
            max_total_bytes: 2_000_000,
            max_texture_bytes: 500_000,
            max_mesh_triangles: 12_000,
        };
        let mut manifeste = AssetsManifest::new(budget);

        let mut sprite = entree("hero_idle", AssetKind::Sprite, "assets/sprites/hero.png", b"png-hero");
        sprite.dimensions = Some((32, 48));
        sprite.provenance = Provenance {
            source: ProvenanceSource::Generated {
                model: "pxl-local".to_string(),
                prompt: "chevalier pixel-art, 32x48".to_string(),
                seed: Some(42),
            },
            license: Some("CC0-1.0".to_string()),
            created_at: "2026-11-10".to_string(),
        };
        manifeste.register(sprite).expect("sprite accepté");

        let mut musique = entree("theme", AssetKind::Audio, "assets/audio/theme.ogg", b"ogg");
        musique.duration_ms = Some(1_500);
        musique.provenance = Provenance {
            source: ProvenanceSource::Imported {
                tool: "Audacity".to_string(),
            },
            license: Some("CC-BY-4.0".to_string()),
            created_at: "2026-11-11".to_string(),
        };
        manifeste.register(musique).expect("audio accepté");

        let json = manifeste.to_json().expect("sérialisation");
        assert!(json.contains("\n  \"version\": 1"));
        assert!(json.contains("\"type\": \"generated\""));
        assert!(json.contains("\"seed\": 42"));

        let relu = AssetsManifest::from_json(&json).expect("relecture");
        assert_eq!(relu, manifeste);
    }

    #[test]
    fn from_file_lit_un_manifeste_et_signale_un_chemin_absent() {
        let bac = Bac::neuf("from_file");
        let mut manifeste = AssetsManifest::default();
        manifeste
            .register(entree("hero", AssetKind::Sprite, "assets/hero.png", b"png"))
            .expect("entrée acceptée");
        let chemin = bac.ecrire(
            "manifeste.json",
            manifeste.to_json().expect("sérialisation").as_bytes(),
        );

        let relu = AssetsManifest::from_file(&chemin).expect("manifeste lu depuis le disque");
        assert_eq!(relu, manifeste);

        let erreur = AssetsManifest::from_file(bac.dir.join("introuvable.json"))
            .expect_err("fichier absent refusé");
        assert!(matches!(erreur, ManifestError::Io(_)));
        assert!(erreur.to_string().contains("manifeste illisible"));
    }

    #[test]
    fn json_omet_les_champs_optionnels_absents() {
        let mut manifeste = AssetsManifest::default();
        manifeste
            .register(entree("hero", AssetKind::Sprite, "assets/hero.png", b"png"))
            .expect("entrée acceptée");
        manifeste.entries[0].provenance.license = None;
        let json = manifeste.to_json().expect("sérialisation");
        assert!(!json.contains("\"license\""));
        assert!(!json.contains("\"dimensions\""));
        assert!(!json.contains("\"duration_ms\""));
        assert!(!json.contains("\"seed\""));
    }

    #[test]
    fn from_json_refuse_une_version_inconnue_ou_absente() {
        let json = AssetsManifest::default().to_json().expect("sérialisation");
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("JSON de test");
        value["version"] = serde_json::json!(99);
        let erreur = AssetsManifest::from_json(&value.to_string()).expect_err("version 99 refusée");
        assert!(matches!(erreur, ManifestError::VersionInconnue(99)));

        let mut sans_version: serde_json::Value =
            serde_json::from_str(&json).expect("JSON de test");
        sans_version
            .as_object_mut()
            .expect("objet JSON")
            .remove("version");
        let erreur =
            AssetsManifest::from_json(&sans_version.to_string()).expect_err("version absente refusée");
        assert!(matches!(erreur, ManifestError::VersionInconnue(0)));
        assert!(erreur.to_string().contains("version"));

        let erreur = AssetsManifest::from_json("{").expect_err("JSON invalide refusé");
        assert!(matches!(erreur, ManifestError::Json(_)));
    }

    #[test]
    fn from_json_refuse_un_manifeste_hors_budget() {
        let budget = AssetBudget {
            max_assets: 1,
            ..AssetBudget::default()
        };
        let mut manifeste = AssetsManifest::new(budget);
        manifeste
            .register(entree("un", AssetKind::Sprite, "assets/un.png", b"a"))
            .expect("premier asset accepté");
        let json = manifeste.to_json().expect("sérialisation");

        let mut value: serde_json::Value = serde_json::from_str(&json).expect("JSON de test");
        let deuxieme = serde_json::json!({
            "id": "deux",
            "kind": "sprite",
            "path": "assets/deux.png",
            "bytes": 1,
            "hash": hash_text(b"b"),
            "provenance": {
                "source": { "type": "studio" },
                "license": "CC0-1.0",
                "created_at": "2026-11-11"
            }
        });
        value["entries"]
            .as_array_mut()
            .expect("tableau d'entrées")
            .push(deuxieme);
        let erreur = AssetsManifest::from_json(&value.to_string())
            .expect_err("budget contourné par le JSON refusé");
        assert!(matches!(erreur, ManifestError::BudgetAssets { max: 1 }));
    }
}
