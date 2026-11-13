use base64::{engine::general_purpose::STANDARD, Engine as _};
use crate::entity::{Entity, ShapeKind};
use crate::game::GameState;
use crate::scene::SceneData;
use crate::subsystems::UiScreen;
use kvl_core::{Graph, Value};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Nom du fichier de glue JS produit par `wasm-bindgen --target web`.
const RUNTIME_GLUE_FILE: &str = "engine_wasm.js";
/// Nom du binaire wasm produit par `wasm-bindgen`.
const RUNTIME_WASM_FILE: &str = "engine_wasm_bg.wasm";
/// Nom du manifeste de build écrit par `scripts/build_wasm.sh` à côté des
/// artefacts : version du moteur et empreintes des fichiers.
const RUNTIME_MANIFEST_FILE: &str = "runtime_manifest.json";

/// Runtime WASM embarquable dans un export HTML autonome.
///
/// Regroupe la glue JS `engine_wasm.js` et le binaire `engine_wasm_bg.wasm`
/// encodé en base64. L'exporteur les insère dans deux balises `<script>` non
/// exécutées ; le template les décode et charge la glue via une Blob URL, puis
/// instancie le module avec `initSync({ module: octets })`. Résultat : un seul
/// fichier `.html`, jouable même en `file://` (aucun `import()` de module
/// voisin, donc aucune requête CORS et aucune requête réseau).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRuntime {
    /// Contenu texte de `engine_wasm.js` (glue wasm-bindgen).
    pub glue: String,
    /// Contenu de `engine_wasm_bg.wasm` encodé en base64 standard.
    pub wasm_base64: String,
    /// Taille réellement embarquée (glue + base64), en octets.
    ///
    /// C'est le surcoût du runtime dans le HTML : le base64 pèse environ 4/3
    /// du binaire wasm. Sert à l'avertissement de pré-vol `runtime-embarque`.
    pub bytes: usize,
    /// Version du moteur ayant produit les artefacts, lue dans
    /// `runtime_manifest.json`. `None` quand le manifeste est absent (build
    /// antérieur) : l'export reste alors identique à l'historique.
    pub engine_version: Option<String>,
    /// Empreinte SHA-256 du binaire wasm, lue dans le manifeste. `None` quand
    /// le manifeste est absent.
    pub wasm_sha256: Option<String>,
}

impl EmbeddedRuntime {
    /// Taille lisible en français, par exemple `« 4,7 Mo »`.
    pub fn human_size(&self) -> String {
        taille_lisible(self.bytes)
    }
}

/// Longueur exacte d'un encodage base64 standard (remplissage compris).
fn longueur_base64(octets: usize) -> usize {
    if octets == 0 {
        0
    } else {
        4 * ((octets + 2) / 3)
    }
}

/// Formate une taille en octets à la française (`« 4,7 Mo »`).
///
/// Le seuil et le diviseur sont des multiples binaires (1 Mo = 1 024 Ko), comme
/// les tailles affichées par le reste du Studio.
fn taille_lisible(octets: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * KIB;
    let octets_f = octets as f64;
    if octets_f >= MIB {
        format!("{:.1} Mo", octets_f / MIB).replace('.', ",")
    } else if octets_f >= KIB {
        format!("{:.1} Ko", octets_f / KIB).replace('.', ",")
    } else {
        format!("{octets} o")
    }
}

/// Trie récursivement les clés d'un [`serde_json::Value`].
///
/// Les tables du package (`kvl_graphs`, `sprite_assets`, `sheet_assets`,
/// `initial_variables`) sont des `HashMap` : leur ordre d'itération dépend de
/// la graine aléatoire du processus. Sans tri, deux exports du **même** projet
/// produisent des JSON différents, donc des fichiers et des empreintes
/// différentes. Le tri rend l'export reproductible octet pour octet.
fn trier_cles_json(valeur: serde_json::Value) -> serde_json::Value {
    match valeur {
        serde_json::Value::Object(table) => {
            let mut entrees: Vec<(String, serde_json::Value)> = table.into_iter().collect();
            entrees.sort_by(|a, b| a.0.cmp(&b.0));
            let mut trie = serde_json::Map::with_capacity(entrees.len());
            for (cle, valeur) in entrees {
                trie.insert(cle, trier_cles_json(valeur));
            }
            serde_json::Value::Object(trie)
        }
        serde_json::Value::Array(elements) => {
            serde_json::Value::Array(elements.into_iter().map(trier_cles_json).collect())
        }
        autre => autre,
    }
}

/// Sérialise une valeur en JSON compact et **déterministe** (clés triées).
///
/// Utilisé pour le paquet embarqué dans le HTML : à projet et artefacts
/// identiques, le fichier exporté est identique à l'octet près.
fn serialiser_json_deterministe<T: Serialize>(valeur: &T) -> Result<String, serde_json::Error> {
    let value = serde_json::to_value(valeur)?;
    serde_json::to_string(&trier_cles_json(value))
}

/// Manifeste de build du runtime WASM (`dist/pkg/runtime_manifest.json`).
///
/// Écrit par `scripts/build_wasm.sh` juste après `wasm-bindgen`. Il date les
/// artefacts pour qu'un `dist/pkg` périmé devienne visible au lieu de produire
/// un export silencieusement bâti sur un ancien moteur.
///
/// Tous les champs sont optionnels et le fichier peut être absent : un ancien
/// `dist/pkg` (sans manifeste) continue de fonctionner sans avertissement.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
pub struct RuntimeManifest {
    /// Version de `engine-core` au moment du build.
    #[serde(default)]
    pub engine_version: Option<String>,
    /// Empreinte SHA-256 de `engine_wasm_bg.wasm`.
    #[serde(default)]
    pub wasm_sha256: Option<String>,
    /// Empreinte SHA-256 de `engine_wasm.js` (glue wasm-bindgen).
    #[serde(default)]
    pub glue_sha256: Option<String>,
    /// Cible de compilation, par exemple `wasm32-unknown-unknown`.
    #[serde(default)]
    pub target: Option<String>,
}

/// Lit `runtime_manifest.json` dans `dir`, sans toucher au wasm.
///
/// Renvoie `None` si le fichier est absent, illisible ou n'est pas du JSON :
/// l'appelant retombe alors sur le comportement historique (aucune version
/// connue, aucun avertissement).
pub fn load_runtime_manifest(dir: &Path) -> Option<RuntimeManifest> {
    let texte = std::fs::read_to_string(dir.join(RUNTIME_MANIFEST_FILE)).ok()?;
    serde_json::from_str(&texte).ok()
}

/// Charge le runtime WASM embarquable depuis `dir`.
///
/// Lit `engine_wasm.js` et `engine_wasm_bg.wasm`, puis encode le binaire en
/// base64. Renvoie `None` si l'un des deux fichiers est absent, illisible ou
/// vide : l'appelant produit alors un export sans runtime (carte explicite au
/// lieu du jeu). **Aucune lecture réseau** : uniquement le disque local.
///
/// Le HTML produit est déterministe pour un même contenu de `dir` : base64
/// standard sans saut de ligne ni horodatage. La version et l'empreinte du
/// wasm proviennent de `runtime_manifest.json` quand il existe ; elles ne sont
/// ni vérifiées ni recalculées ici (aucune dépendance de hachage embarquée).
pub fn load_embedded_runtime(dir: &Path) -> Option<EmbeddedRuntime> {
    let glue = std::fs::read_to_string(dir.join(RUNTIME_GLUE_FILE)).ok()?;
    let wasm = std::fs::read(dir.join(RUNTIME_WASM_FILE)).ok()?;
    if glue.trim().is_empty() || wasm.is_empty() {
        return None;
    }
    let manifeste = load_runtime_manifest(dir).unwrap_or_default();
    let wasm_base64 = STANDARD.encode(&wasm);
    let bytes = glue.len() + wasm_base64.len();
    debug_assert_eq!(wasm_base64.len(), longueur_base64(wasm.len()));
    Some(EmbeddedRuntime {
        glue,
        wasm_base64,
        bytes,
        engine_version: manifeste.engine_version,
        wasm_sha256: manifeste.wasm_sha256,
    })
}

/// Taille embarquée d'un runtime présent dans `dir`, **sans** encoder le wasm.
///
/// Utilisée par le pré-vol : inutile de lire et d'encoder 3,5 Mo juste pour
/// afficher un avertissement de taille. Le calcul reproduit exactement la
/// longueur base64, donc la taille annoncée est celle du HTML final.
fn taille_runtime_embarque(dir: &Path) -> Option<usize> {
    let glue = std::fs::metadata(dir.join(RUNTIME_GLUE_FILE)).ok()?;
    let wasm = std::fs::metadata(dir.join(RUNTIME_WASM_FILE)).ok()?;
    if !glue.is_file() || !wasm.is_file() || glue.len() == 0 || wasm.len() == 0 {
        return None;
    }
    let glue = usize::try_from(glue.len()).ok()?;
    let wasm = usize::try_from(wasm.len()).ok()?;
    Some(glue + longueur_base64(wasm))
}

/// Répertoire du runtime utilisé par [`StandaloneExporter::export_html`].
///
/// Ordre de résolution :
/// 1. `$KARACTI_RUNTIME_DIR` s'il est défini — même s'il ne contient rien, ce
///    qui est le moyen explicite de forcer un export sans runtime ;
/// 2. `dist/pkg` du répertoire courant ;
/// 3. `dist/pkg` à la racine du dépôt (`CARGO_MANIFEST_DIR/../..`), pour que
///    l'export fonctionne aussi quand le binaire est lancé depuis
///    `crates/…` ou `apps/studio/…`.
///
/// Aucune de ces pistes n'est une URL : tout vient du disque local.
pub fn default_runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("KARACTI_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    let racine_courante = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let local = racine_courante.join("dist/pkg");
    if local.join(RUNTIME_WASM_FILE).is_file() {
        return local;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/pkg")
}

/// Échappe les fins de balise `</script` d'un contenu inséré dans un
/// `<script type="text/plain">` : le parseur HTML couperait le script.
///
/// L'échappement `<\/script` est neutre en JavaScript (le `/` échappé vaut `/`
/// dans une chaîne ou une expression régulière) et ne modifie donc pas la glue.
/// La comparaison ignore la casse, comme le parseur HTML.
fn echapper_fin_de_script(source: &str) -> String {
    const MOTIF: &[u8] = b"</script";
    let octets = source.as_bytes();
    let mut sortie = String::with_capacity(source.len());
    let mut i = 0;
    while i < octets.len() {
        if i + MOTIF.len() <= octets.len()
            && octets[i..i + MOTIF.len()].eq_ignore_ascii_case(MOTIF)
        {
            sortie.push_str("<\\/script");
            i += MOTIF.len();
        } else {
            let caractere = source[i..]
                .chars()
                .next()
                .expect("indice toujours aligné sur une frontière UTF-8");
            sortie.push(caractere);
            i += caractere.len_utf8();
        }
    }
    sortie
}

/// Injecte les deux balises de runtime juste avant `</body>`.
///
/// Sans `</body>` (HTML étranger), le bloc est ajouté à la fin : la page reste
/// lisible et le runtime disponible.
fn injecter_runtime(html: String, runtime: &EmbeddedRuntime) -> String {
    let bloc = format!(
        "  <script id=\"runtime-glue\" type=\"text/plain\">\n{}\n  </script>\n  <script id=\"runtime-wasm\" type=\"application/base64\">\n{}\n  </script>\n",
        echapper_fin_de_script(&runtime.glue),
        runtime.wasm_base64
    );
    match html.rfind("</body>") {
        Some(position) => {
            let mut sortie = String::with_capacity(html.len() + bloc.len());
            sortie.push_str(&html[..position]);
            sortie.push_str(&bloc);
            sortie.push_str(&html[position..]);
            sortie
        }
        None => {
            let mut sortie = html;
            sortie.push_str(&bloc);
            sortie
        }
    }
}

/// Balise `<meta name="karacti-runtime">` décrivant le runtime embarqué.
///
/// Contenu `version=…;wasm=…` (l'un ou l'autre si une seule valeur est
/// connue), chaîne vide quand aucune ne l'est : un export sans manifeste ne
/// gagne donc aucune balise et reste identique à l'historique. La fonction est
/// déterministe : même manifeste, même balise.
fn balise_meta_runtime(runtime: &EmbeddedRuntime) -> String {
    let mut morceaux = Vec::with_capacity(2);
    if let Some(version) = &runtime.engine_version {
        morceaux.push(format!("version={}", assainir_valeur_meta(version)));
    }
    if let Some(sha) = &runtime.wasm_sha256 {
        morceaux.push(format!("wasm={}", assainir_valeur_meta(sha)));
    }
    if morceaux.is_empty() {
        return String::new();
    }
    format!(
        "  <meta name=\"karacti-runtime\" content=\"{}\">\n",
        morceaux.join(";")
    )
}

/// Neutralise les caractères qui casseraient l'attribut `content` d'une balise
/// (manifeste altéré ou rédigé à la main).
fn assainir_valeur_meta(valeur: &str) -> String {
    valeur
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
        .collect()
}

/// Diagnostic pré-vol du runtime embarqué (non bloquant).
fn avertissement_runtime(bytes: usize) -> PreflightIssue {
    PreflightIssue::warning(
        "runtime-embarque",
        format!(
            "runtime embarqué : {} — la glue JS et le binaire wasm sont inclus dans le HTML ; le jeu est jouable hors ligne en file:// (le fichier est plus lourd d'autant)",
            taille_lisible(bytes)
        ),
    )
}

/// Diagnostic pré-vol d'un runtime construit pour une autre version du moteur.
///
/// Non bloquant : l'export reste possible, mais il jouerait l'ancien moteur.
fn avertissement_runtime_perime(version_runtime: &str, version_moteur: &str) -> PreflightIssue {
    PreflightIssue::warning(
        "runtime-perime",
        format!(
            "le runtime embarqué a été construit pour la version {version_runtime}, le moteur est en {version_moteur} — relancez scripts/build_wasm.sh"
        ),
    )
}

/// Métadonnées descriptives du jeu exporté
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// Titre du jeu
    pub title: String,
    /// Auteur ou studio créateur
    pub author: String,
    /// Version du jeu (ex: "1.0.0")
    pub version: String,
    /// Genre prédominant (Aventure, Platformer, Shmup, Sokoban, Boss Fight)
    pub genre: String,
    /// Horodatage de génération de l'export
    pub created_at: String,
    /// Taux de rafraîchissement cible (par défaut 60 FPS)
    pub target_fps: u32,
    /// Largeur de rendu native (1200 px)
    pub screen_width: u32,
    /// Hauteur de rendu native (800 px)
    pub screen_height: u32,
    /// Démarrage direct en mode joueur
    pub play_mode_only: bool,
}

impl Default for PackageMetadata {
    fn default() -> Self {
        Self {
            title: "Jeu KaractiRage".to_string(),
            author: "Créateur Indépendant".to_string(),
            version: "1.0.0".to_string(),
            genre: "Aventure & Donjon".to_string(),
            created_at: "2026-10-16".to_string(),
            target_fps: 60,
            screen_width: 1200,
            screen_height: 800,
            play_mode_only: true,
        }
    }
}

/// Spritesheet embarquée dans un package autonome (atlas PNG + JSON).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SheetAssetExport {
    /// Atlas PNG en base64.
    pub png: String,
    /// JSON TexturePacker/Aseprite.
    pub json: String,
}

/// Package autonome universel (.karacti.json) embarquant scène et logique KVL
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KaractiGamePackage {
    /// Métadonnées du package
    pub metadata: PackageMetadata,
    /// Nom du niveau ou de la scène
    pub scene_name: String,
    /// Objectif narratif ou de victoire
    pub scene_objective: Option<String>,
    /// Liste complète des entités avec leurs composants ECS
    pub entities: Vec<Entity>,
    /// Niveaux embarqués du projet, dans l'ordre de jeu (`scenes[0]` = niveau 1).
    ///
    /// `entities`/`scene_name` restent la **scène courante** au moment de
    /// l'export (compatibilité des lecteurs historiques) ; `scenes` transporte
    /// l'intégralité du projet pour que le HTML contienne tous les niveaux.
    ///
    /// `#[serde(default)]` : un ancien paquet sans ce champ reste lisible, son
    /// `scenes` est vide et l'import conserve alors le comportement mono-scène.
    #[serde(default)]
    pub scenes: Vec<SceneData>,
    /// Écrans UI déclaratifs du projet (HUD, menu, pause, défaite, victoire).
    ///
    /// Sans eux, un HTML autonome importé restait figé sur l'écran `Menu` mais
    /// n'avait aucun menu à dessiner : le joueur voyait le monde sans interface
    /// de départ. `#[serde(default)]` garde les anciens paquets lisibles : le
    /// champ absent devient un `Vec` vide et l'import conserve son comportement
    /// historique (aucun écran déclaré, monde jouable immédiatement).
    #[serde(default)]
    pub ui_screens: Vec<UiScreen>,
    /// Table de tous les graphes KVL associés aux entités (tous niveaux confondus)
    pub kvl_graphs: HashMap<String, Graph>,
    /// Variables globales initiales de contexte
    pub initial_variables: HashMap<String, Value>,
    /// Sprites pixel-art embarqués (id -> PNG base64), pour l'export autonome.
    #[serde(default)]
    pub sprite_assets: HashMap<String, String>,
    /// Spritesheets embarquées (id -> atlas PNG base64 + JSON), pour l'export autonome.
    #[serde(default)]
    pub sheet_assets: HashMap<String, SheetAssetExport>,
    /// Sources SVG embarquées (id -> source SVG), pour l'export autonome.
    ///
    /// Le runtime wasm ne peut pas lire le disque : sans ces sources,
    /// `ShapeKind::SvgRef` retombait sur le rectangle teinté. Le texte est
    /// léger et conserve rigs/animations (rastérisés à l'import par `usvg`).
    /// `#[serde(default)]` garde lisibles les paquets produits avant ce champ.
    #[serde(default)]
    pub svg_assets: HashMap<String, String>,
    /// Clips d'animation personnalisés embarqués (id du sprite -> contenu brut
    /// du fichier `.anim.json`), pour l'export autonome.
    ///
    /// `SvgCache::load_clips` lisait `assets/sprites/{id}.anim.json` sur disque :
    /// sous wasm cette lecture échoue et le rig retombait sur les clips générés
    /// (`idle`/`walk`), donc un auteur perdait ses propres clips dans le jeu
    /// exporté. Ces contenus sont prioritaires sur le disque et sur la
    /// génération par défaut. `#[serde(default)]` garde lisibles les paquets
    /// produits avant ce champ.
    #[serde(default)]
    pub svg_anim_assets: HashMap<String, String>,
}

impl KaractiGamePackage {
    /// Convertit le package en chaîne JSON formatée
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convertit le package en chaîne JSON compacte
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Charge un package depuis une chaîne JSON
    #[allow(dead_code)]
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Génère un document HTML5 standalone complet intégrant le package et le lecteur Web
    pub fn generate_standalone_html(&self) -> String {
        self.generate_standalone_html_with_runtime(None)
    }

    /// Génère le document HTML5 autonome, en embarquant éventuellement le
    /// runtime WASM (glue + binaire base64) avant `</body>`.
    ///
    /// `None` produit le même document qu'historiquement : données du jeu
    /// seules, carte d'identification au démarrage, aucune dépendance externe.
    /// `Some(runtime)` produit un fichier réellement jouable en un seul fichier.
    ///
    /// Le paquet est sérialisé avec les **clés triées** : les `HashMap` du
    /// package n'exposent donc pas leur ordre d'itération aléatoire, et deux
    /// exports du même projet donnent exactement le même fichier.
    pub fn generate_standalone_html_with_runtime(
        &self,
        runtime: Option<&EmbeddedRuntime>,
    ) -> String {
        let package_json = serialiser_json_deterministe(self).unwrap_or_else(|_| "{}".to_string());
        let escaped_json = package_json.replace("</script>", "<\\/script>");
        let meta_runtime = runtime.map(balise_meta_runtime).unwrap_or_default();
        let template = include_str!("standalone_template.html");
        let html = template
            .replace("{{RUNTIME_META}}", &meta_runtime)
            .replace("{{TITLE}}", &self.metadata.title)
            .replace("{{AUTHOR}}", &self.metadata.author)
            .replace("{{GENRE}}", &self.metadata.genre)
            .replace("{{EMBEDDED_JSON}}", &escaped_json);
        match runtime {
            Some(runtime) => injecter_runtime(html, runtime),
            None => html,
        }
    }
}

/// Sévérité d'un diagnostic de pré-vol d'export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreflightSeverity {
    /// Problème qui empêche un export jouable (script ou asset introuvable).
    Blocking,
    /// Problème tolérable qui mérite l'attention du créateur (budget de performance).
    Warning,
}

/// Diagnostic unitaire produit par le pré-vol d'export.
///
/// Le `code` est volontairement court, stable et sans accent : il sert de clé
/// technique à l'interface Studio, tandis que `message` est destiné au créateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightIssue {
    /// Gravité du diagnostic.
    pub severity: PreflightSeverity,
    /// Code technique stable : `"script-manquant"`, `"asset-manquant"`,
    /// `"budget-entites"`, `"budget-kvl"`, `"manifeste-absent"`,
    /// `"runtime-embarque"`, `"runtime-perime"`, ou l'un des codes relayés du
    /// manifeste (`"fichier-absent"`, `"hash-incoherent"`, `"taille-different"`,
    /// `"budget-depasse"`, `"licence-manquante"`, `"chemin-invalide"`).
    pub code: &'static str,
    /// Message lisible en français, orienté créateur.
    pub message: String,
}

impl PreflightIssue {
    /// Construit un diagnostic bloquant.
    fn blocking(code: &'static str, message: String) -> Self {
        Self { severity: PreflightSeverity::Blocking, code, message }
    }

    /// Construit un diagnostic d'avertissement.
    fn warning(code: &'static str, message: String) -> Self {
        Self { severity: PreflightSeverity::Warning, code, message }
    }
}

/// Rapport de pré-vol d'export : l'état du jeu est-il exportable en l'état ?
///
/// `ok` vaut `true` quand aucun diagnostic [`PreflightSeverity::Blocking`] n'a
/// été relevé ; les avertissements de budget ne bloquent jamais l'export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    /// `true` si aucun bloquage n'a été relevé.
    pub ok: bool,
    /// Diagnostics détectés, dans l'ordre de détection (blocages puis budgets).
    pub issues: Vec<PreflightIssue>,
}

impl PreflightReport {
    /// Nombre de diagnostics bloquants (export refusé).
    pub fn blocking_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == PreflightSeverity::Blocking)
            .count()
    }

    /// Diagnostics non bloquants, dans l'ordre de détection.
    pub fn warnings(&self) -> Vec<&PreflightIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == PreflightSeverity::Warning)
            .collect()
    }

    /// Résumé lisible pour le créateur, par exemple
    /// `« 2 blocages, 1 avertissement : script-manquant — … ; budget-entites — … »`.
    pub fn summary(&self) -> String {
        if self.issues.is_empty() {
            return "Pré-vol réussi : aucun problème détecté.".to_string();
        }
        let blocages = self.blocking_count();
        let avertissements = self.warnings().len();
        let entete = format!(
            "{} blocage{}, {} avertissement{}",
            blocages,
            if blocages > 1 { "s" } else { "" },
            avertissements,
            if avertissements > 1 { "s" } else { "" }
        );
        let details = self
            .issues
            .iter()
            .map(|i| format!("{} — {}", i.code, i.message))
            .collect::<Vec<_>>()
            .join(" ; ");
        format!("{entete} : {details}")
    }
}

/// Itère sur toutes les entités que l'export doit couvrir : celles de la scène
/// courante, puis celles de chaque niveau embarqué (`GameState::levels`).
///
/// Les deux ensembles se recoupent souvent (`levels[0]` reflète la scène
/// courante) : les tables d'assets sont des `HashMap` indexées par identifiant
/// et le pré-vol déduplique par référence, donc aucun doublon n'est produit.
fn entites_exportees(game: &GameState) -> impl Iterator<Item = std::borrow::Cow<'_, Entity>> {
    let courantes = game.entities.iter().map(std::borrow::Cow::Borrowed);
    // `to_entities` matérialise aussi les scènes au format historique
    // (`player`/`coins`/`doors`/`enemies` sans tableau `entities`) : leurs
    // scripts et sprites doivent voyager comme ceux des scènes modernes.
    let niveaux = game.levels.iter().flat_map(|scene| {
        scene
            .to_entities()
            .into_iter()
            .map(std::borrow::Cow::Owned)
            .collect::<Vec<_>>()
            .into_iter()
    });
    courantes.chain(niveaux)
}

/// Lit le JSON d'un script KVL référencé par une entité.
///
/// Même ordre de résolution que `GameState::import_scripts` : chemin courant,
/// puis `../../` (tests exécutés depuis `crates/engine-core`), puis racine du
/// dépôt. Aucune lecture réseau. Un script introuvable renvoie `None` : c'est
/// le pré-vol qui le signale comme bloquant, pas la sérialisation.
fn lire_script_kvl(chemin: &str) -> Option<String> {
    let candidats = [
        PathBuf::from(chemin),
        PathBuf::from(format!("../../{chemin}")),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(chemin),
    ];
    candidats
        .iter()
        .find_map(|candidat| std::fs::read_to_string(candidat).ok())
}

/// Union des graphes KVL du paquet : graphes déjà chargés dans l'état, puis
/// tous les scripts référencés par les entités des niveaux embarqués.
///
/// Un paquet multi-niveaux doit transporter la logique de **toutes** ses
/// scènes : sans cette union, seul le niveau courant garderait ses graphes et
/// les portes des niveaux suivants resteraient muettes. La clé est le
/// `script_path` exact de l'entité (celle qu'utilise le runtime pour résoudre
/// le graphe).
fn collecter_graphes_kvl(game: &GameState) -> HashMap<String, Graph> {
    let mut graphes = game.kvl_graphs.clone();
    for entite in entites_exportees(game) {
        let Some(script) = &entite.script else { continue };
        let chemin = script.script_path.as_str();
        if chemin.trim().is_empty() || graphes.contains_key(chemin) {
            continue;
        }
        if let Some(contenu) = lire_script_kvl(chemin) {
            if let Ok(graphe) = Graph::from_json(&contenu) {
                graphes.insert(chemin.to_string(), graphe);
            }
        }
    }
    graphes
}

/// Décode les assets embarqués d'un paquet (`id -> octets` / `id -> (png, json)`).
///
/// Un base64 corrompu est ignoré silencieusement : le rendu retombe alors sur
/// le repli visuel, comme pour un asset absent du disque.
fn decoder_assets_embarques(
    package: &KaractiGamePackage,
) -> (
    HashMap<String, Vec<u8>>,
    HashMap<String, (Vec<u8>, String)>,
) {
    let mut sprites = HashMap::new();
    for (id, b64) in &package.sprite_assets {
        if let Ok(bytes) = STANDARD.decode(b64) {
            sprites.insert(id.clone(), bytes);
        }
    }
    let mut sheets = HashMap::new();
    for (id, sheet) in &package.sheet_assets {
        if let Ok(png) = STANDARD.decode(&sheet.png) {
            sheets.insert(id.clone(), (png, sheet.json.clone()));
        }
    }
    (sprites, sheets)
}

/// Packageur et exportateur autonome de jeu KaractiRage
pub struct StandaloneExporter;

impl StandaloneExporter {
    /// Crée un package complet prêt pour l'exportation à partir de l'état actuel du jeu
    pub fn create_package(game: &GameState, title: &str, author: &str) -> KaractiGamePackage {
        let genre = match game.scene_name.to_lowercase().as_str() {
            s if s.contains("platformer") => "Plateforme 2D",
            s if s.contains("space") => "Space Shooter Arcade",
            s if s.contains("puzzle") => "Casse-Tête Sokoban",
            s if s.contains("boss") => "Combat de Boss Épique",
            _ => "Aventure & Donjon KVL",
        };

        let metadata = PackageMetadata {
            title: if title.trim().is_empty() { "Mon Jeu KaractiRage".to_string() } else { title.trim().to_string() },
            author: if author.trim().is_empty() { "Créateur Indépendant".to_string() } else { author.trim().to_string() },
            version: "1.0.0".to_string(),
            genre: genre.to_string(),
            created_at: "2026-10-16".to_string(),
            target_fps: 60,
            screen_width: 1200,
            screen_height: 800,
            play_mode_only: true,
        };

        // Extraction des variables de contexte KVL
        let mut initial_vars = HashMap::new();
        let score_val = game.kvl_context.get_var("score").map_or(0, |v| match v { Value::Int(n) => *n, _ => 0 });
        initial_vars.insert("score".to_string(), Value::Int(score_val));
        if let Some(keys) = game.kvl_context.get_var("keys") {
            initial_vars.insert("keys".to_string(), keys.clone());
        }

        // Niveaux embarqués du projet : exporter « le jeu », pas seulement la
        // scène courante. Un état construit par `load_from_scene` (hors projet)
        // a `levels` vide : le paquet reste alors mono-scène, comme avant.
        let scenes = game.levels.clone();

        // Collecte des assets pixel-art référencés (sprites + spritesheets/anims)
        // pour rendre le package autonome (pas de dépendance au système de fichiers).
        // Tous les niveaux sont couverts : un sprite du niveau 3 doit voyager aussi.
        let mut sprite_assets = HashMap::new();
        let mut sheet_assets = HashMap::new();
        let mut svg_assets: HashMap<String, String> = HashMap::new();
        let mut svg_anim_assets: HashMap<String, String> = HashMap::new();
        for ent in entites_exportees(game) {
            if let Some(rend) = &ent.renderable {
                match &rend.shape {
                    ShapeKind::ImageRef { asset } => {
                        if let Some(bytes) = crate::vector::read_png_source(asset) {
                            sprite_assets.entry(asset.clone()).or_insert_with(|| STANDARD.encode(&bytes));
                        }
                    }
                    ShapeKind::SheetRef { asset, .. } | ShapeKind::AnimRef { asset } => {
                        if let Some((png, json)) = crate::sheet::read_sheet_source(asset) {
                            sheet_assets.entry(asset.clone()).or_insert_with(|| SheetAssetExport {
                                png: STANDARD.encode(&png),
                                json,
                            });
                        }
                    }
                    ShapeKind::SvgRef { asset } => {
                        // Même résolution que le Studio (disque + replis
                        // `include_str!`) : la source part dans le paquet pour
                        // que le runtime wasm puisse la rastériser sans disque.
                        if let Some(source) = crate::vector::read_svg_source(asset) {
                            svg_assets.entry(asset.clone()).or_insert(source);
                        }
                        // Le `.anim.json` voisin suit la même résolution : sans
                        // lui, `load_clips` générerait des clips par défaut et
                        // perdrait les animations de l'auteur sous wasm.
                        if let Some(anim) = crate::vector::read_anim_source(asset) {
                            svg_anim_assets.entry(asset.clone()).or_insert(anim);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Un état importé porte des images qui ne sont pas forcément sur disque
        // (jeu exporté puis ré-exporté) : on les ré-embarque pour ne pas les perdre.
        for (id, bytes) in &game.embedded_sprites {
            sprite_assets
                .entry(id.clone())
                .or_insert_with(|| STANDARD.encode(bytes));
        }
        for (id, (png, json)) in &game.embedded_sheets {
            sheet_assets
                .entry(id.clone())
                .or_insert_with(|| SheetAssetExport {
                    png: STANDARD.encode(png),
                    json: json.clone(),
                });
        }
        // Idem pour les sources SVG d'un état importé : elles ne sont pas
        // forcément sur disque (export → import → ré-export).
        for (id, source) in &game.embedded_svg_assets {
            svg_assets.entry(id.clone()).or_insert_with(|| source.clone());
        }
        // Idem pour les clips d'animation d'un état importé : `create_package`
        // les retrouve sur disque seulement s'ils y sont encore, un ré-export
        // doit les reprendre depuis la mémoire du paquet.
        for (id, anim) in &game.embedded_svg_anims {
            svg_anim_assets
                .entry(id.clone())
                .or_insert_with(|| anim.clone());
        }

        // Union des graphes : scène courante + scripts des niveaux 2/3.
        let kvl_graphs = collecter_graphes_kvl(game);

        // Écrans UI du projet : sans eux, l'import forcerait l'écran `Menu`
        // alors qu'aucun menu ne serait dessinable (monde figé sans interface).
        let ui_screens = game.ui_screens.clone();

        KaractiGamePackage {
            metadata,
            scene_name: game.scene_name.clone(),
            scene_objective: game.scene_objective.clone(),
            entities: game.entities.clone(),
            scenes,
            ui_screens,
            kvl_graphs,
            initial_variables: initial_vars,
            sprite_assets,
            sheet_assets,
            svg_assets,
            svg_anim_assets,
        }
    }

    /// Exporte le jeu en un fichier HTML standalone complet.
    ///
    /// Le runtime WASM est cherché via [`default_runtime_dir`]
    /// (`$KARACTI_RUNTIME_DIR`, sinon `dist/pkg`), **sur le disque local
    /// uniquement** : aucune lecture réseau. S'il est trouvé, le HTML contient
    /// la glue et le binaire wasm et le jeu démarre même en `file://`.
    pub fn export_html(game: &GameState, title: &str, author: &str) -> String {
        let runtime_dir = default_runtime_dir();
        Self::export_html_with_runtime(game, title, author, Some(runtime_dir.as_path()))
    }

    /// Exporte le jeu en HTML autonome avec un runtime explicite.
    ///
    /// `runtime_dir = Some(dir)` tente de charger `engine_wasm.js` et
    /// `engine_wasm_bg.wasm` depuis `dir` ; si les artefacts sont absents ou
    /// illisibles, l'export reste valide mais sans runtime (carte explicite).
    /// `None` produit volontairement un export sans runtime, comme
    /// [`KaractiGamePackage::generate_standalone_html`].
    pub fn export_html_with_runtime(
        game: &GameState,
        title: &str,
        author: &str,
        runtime_dir: Option<&Path>,
    ) -> String {
        let package = Self::create_package(game, title, author);
        let runtime = runtime_dir.and_then(load_embedded_runtime);
        package.generate_standalone_html_with_runtime(runtime.as_ref())
    }

    /// Pré-vol d'export : vérifie que chaque script et chaque asset référencés
    /// sont résolvables, puis signale les budgets de performance dépassés.
    ///
    /// Le contrôle couvre **tous les niveaux embarqués** (`game.levels`) et pas
    /// seulement la scène courante : un sprite manquant au niveau 2 bloque donc
    /// l'export, comme un asset manquant au niveau 1. Un projet multi-niveaux
    /// reçoit en plus l'avertissement informatif `"niveaux-exportes"`.
    ///
    /// Un script est considéré présent s'il figure dans `game.kvl_graphs` (graphe
    /// déjà chargé) ou si son fichier existe sur disque. Un asset de rendu est
    /// considéré présent s'il est connu du cache correspondant (`sprite_cache`
    /// pour les PNG/SVG, `sheet_cache` pour les spritesheets/anims) ou si son
    /// fichier source existe sous `assets/`.
    ///
    /// Quand le jeu porte un manifeste d'assets (`game.assets_manifest`), ses
    /// entrées sont en plus vérifiées contre le disque : un fichier absent, une
    /// taille ou un hash différent bloque l'export, une licence manquante
    /// n'émet qu'un avertissement. Un projet **sans** manifeste reste
    /// exportable ; il reçoit seulement l'avertissement `"manifeste-absent"`
    /// lorsqu'une entité référence un script ou un asset de rendu.
    ///
    /// Les dépassements de budget (> 500 entités, > 5000 connexions KVL
    /// cumulées) ne produisent que des avertissements : ils n'empêchent jamais
    /// l'export.
    ///
    /// Le runtime embarqué produit un avertissement `"runtime-embarque"` (taille
    /// ajoutée au HTML) et, quand `runtime_manifest.json` date les artefacts
    /// d'une autre version que `CARGO_PKG_VERSION`, un avertissement
    /// `"runtime-perime"` : l'export resterait possible, mais jouerait un
    /// ancien moteur.
    pub fn preflight(game: &GameState) -> PreflightReport {
        let runtime_dir = default_runtime_dir();
        Self::preflight_with_runtime_dir(game, Some(runtime_dir.as_path()))
    }

    /// Pré-vol identique à [`StandaloneExporter::preflight`], avec un
    /// répertoire de runtime explicite (`None` = pas de runtime à inspecter).
    ///
    /// Permet aux tests de rester indépendants du vrai `dist/pkg` : ils
    /// fabriquent un petit runtime factice dans un répertoire temporaire.
    pub fn preflight_with_runtime_dir(
        game: &GameState,
        runtime_dir: Option<&Path>,
    ) -> PreflightReport {
        const BUDGET_ENTITES: usize = 500;
        const BUDGET_CONNEXIONS_KVL: usize = 5000;

        let mut issues = Vec::new();
        // Évite de répéter le même diagnostic pour N entités partageant la même
        // référence (cas fréquent : un script de collectible sur 20 pièces).
        let mut deja_signales: HashSet<String> = HashSet::new();

        for ent in entites_exportees(game) {
            // --- Scripts KVL -------------------------------------------------
            if let Some(script) = &ent.script {
                let chemin = script.script_path.trim();
                if !chemin.is_empty()
                    && !game.kvl_graphs.contains_key(&script.script_path)
                    && !fichier_existe(chemin)
                {
                    let cle = format!("script-manquant\u{1f}{chemin}");
                    if deja_signales.insert(cle) {
                        issues.push(PreflightIssue::blocking(
                            "script-manquant",
                            format!(
                                "le script « {chemin} » est référencé par l'entité « {} » mais introuvable : aucun graphe KVL chargé et aucun fichier sur disque",
                                ent.name
                            ),
                        ));
                    }
                }
            }

            // --- Assets de rendu ---------------------------------------------
            let Some(rend) = &ent.renderable else { continue };
            let asset_absent = match &rend.shape {
                ShapeKind::SvgRef { asset } => {
                    // `vector_sprite` est renseigné à l'import quand le SVG a pu
                    // être rastérisé (y compris via un asset embarqué).
                    ent.vector_sprite.is_none()
                        && game.sprite_cache.get(asset).is_none()
                        && !asset_svg_existe(asset)
                }
                ShapeKind::ImageRef { asset } => {
                    game.sprite_cache.get(asset).is_none()
                        && crate::vector::read_png_source(asset).is_none()
                }
                ShapeKind::SheetRef { asset, .. } | ShapeKind::AnimRef { asset } => {
                    game.sheet_cache.get(asset).is_none()
                        && crate::sheet::read_sheet_source(asset).is_none()
                }
                _ => false,
            };
            if asset_absent {
                let (reference, libelle) = match &rend.shape {
                    ShapeKind::SvgRef { asset } | ShapeKind::ImageRef { asset } => (asset, "sprite"),
                    ShapeKind::SheetRef { asset, .. } | ShapeKind::AnimRef { asset } => {
                        (asset, "spritesheet")
                    }
                    _ => unreachable!("seuls les assets de rendu sont vérifiés"),
                };
                let cle = format!("asset-manquant\u{1f}{reference}");
                if deja_signales.insert(cle) {
                    issues.push(PreflightIssue::blocking(
                        "asset-manquant",
                        format!(
                            "le {libelle} « {reference} » est référencé par l'entité « {} » mais introuvable : absent du cache et absent sous assets/",
                            ent.name
                        ),
                    ));
                }
            }
        }

        // --- Manifeste d'assets (P2.1) --------------------------------------
        // Un projet sans manifeste reste exportable ; l'absence n'est signalée
        // que si des entités référencent réellement des scripts ou des assets.
        if let Some(manifeste) = &game.assets_manifest {
            let racine = racine_du_manifeste(manifeste);
            for issue in manifeste.validate(&racine) {
                let severity = if issue.blocking {
                    PreflightSeverity::Blocking
                } else {
                    PreflightSeverity::Warning
                };
                issues.push(PreflightIssue {
                    severity,
                    code: issue.code,
                    message: format!("manifeste : {}", issue.message),
                });
            }
        } else if entites_referencent_des_assets(game) {
            issues.push(PreflightIssue::warning(
                "manifeste-absent",
                "des entités référencent des scripts ou des assets de rendu, mais le projet ne déclare aucun manifeste d'assets : leur provenance et leur intégrité ne sont pas vérifiées. Ajoute un manifeste au projet.".to_string(),
            ));
        }

        // --- Niveaux embarqués (avertissement informatif) ---------------------
        // Depuis l'export multi-niveaux, le HTML contient toutes les scènes du
        // projet. Le créateur doit le savoir : la taille et le contenu du
        // fichier ne correspondent plus à la seule scène courante.
        if game.levels.len() > 1 {
            issues.push(PreflightIssue::warning(
                "niveaux-exportes",
                format!(
                    "{} niveaux seront embarqués dans le HTML (toutes les scènes du projet, pas seulement la scène courante)",
                    game.levels.len()
                ),
            ));
        }

        // --- Runtime embarqué (avertissements uniquement) --------------------
        // Sans artefacts wasm, l'export reste valide mais non jouable : jamais
        // bloquant. La taille annoncée est celle réellement ajoutée au HTML
        // (glue + binaire base64), calculée sans encoder le wasm.
        if let Some(dir) = runtime_dir {
            if let Some(bytes) = taille_runtime_embarque(dir) {
                issues.push(avertissement_runtime(bytes));
                // Le manifeste peut dater les artefacts d'une autre version :
                // l'export fonctionnerait, mais avec l'ancien moteur.
                if let Some(version_runtime) =
                    load_runtime_manifest(dir).and_then(|manifeste| manifeste.engine_version)
                {
                    let version_moteur = env!("CARGO_PKG_VERSION");
                    if version_runtime != version_moteur {
                        issues.push(avertissement_runtime_perime(
                            &version_runtime,
                            version_moteur,
                        ));
                    }
                }
            }
        }

        // --- Budgets (avertissements uniquement) -----------------------------
        if game.entities.len() > BUDGET_ENTITES {
            issues.push(PreflightIssue::warning(
                "budget-entites",
                format!(
                    "{} entités : au-delà de {BUDGET_ENTITES}, les profils Bas de gamme et Batterie risquent de passer sous 60 FPS",
                    game.entities.len()
                ),
            ));
        }
        let connexions: usize = game.kvl_graphs.values().map(|g| g.connections.len()).sum();
        if connexions > BUDGET_CONNEXIONS_KVL {
            issues.push(PreflightIssue::warning(
                "budget-kvl",
                format!(
                    "{connexions} connexions KVL cumulées : au-delà de {BUDGET_CONNEXIONS_KVL}, l'interpréteur peut coûter cher à chaque frame"
                ),
            ));
        }

        PreflightReport {
            ok: !issues
                .iter()
                .any(|i| i.severity == PreflightSeverity::Blocking),
            issues,
        }
    }

    /// Export HTML protégé par le pré-vol : renvoie le rapport en `Err` dès qu'un
    /// diagnostic bloquant est présent, sinon produit l'export standard.
    pub fn export_html_checked(
        game: &GameState,
        title: &str,
        author: &str,
    ) -> Result<String, PreflightReport> {
        let report = Self::preflight(game);
        if !report.ok {
            return Err(report);
        }
        Ok(Self::export_html(game, title, author))
    }

    /// Empreinte stable du contenu exporté (FNV-1a 64 bits, 16 caractères hex).
    ///
    /// Le paquet JSON embarqué dans le HTML est isolé et haché en priorité : la
    /// coquille HTML (titre, styles, lecteur) n'influence donc pas l'empreinte et
    /// une éventuelle date volatile ajoutée dans la page (pied de page, bannière
    /// « généré le … ») ne la fait pas dériver. Si le marqueur du paquet est
    /// absent (HTML étranger), l'empreinte porte sur la chaîne entière.
    ///
    /// Limite connue : `PackageMetadata::created_at` est actuellement une
    /// constante (`"2026-10-16"`), donc l'export est reproductible. Si ce champ
    /// devenait un horodatage réel, il faudrait l'exclure du paquet avant
    /// hachage, sinon deux exports identiques produiraient deux empreintes.
    ///
    /// Limite assumée : le **runtime embarqué** (`#runtime-glue` + le wasm
    /// base64, soit ~4,7 Mo) n'entre pas dans l'empreinte. Deux exports du même
    /// jeu produits avec des artefacts wasm différents partagent donc la même
    /// empreinte alors que leurs fichiers diffèrent : c'est voulu, l'empreinte
    /// identifie le contenu du jeu (entités, graphes KVL, assets), pas le
    /// binaire du moteur. En revanche, à projet et artefacts identiques, la
    /// sortie est octet pour octet identique (base64 déterministe, aucune date
    /// volatile).
    ///
    /// Ce n'est pas un hash cryptographique : il sert à détecter une dérive de
    /// contenu, pas à authentifier un export.
    pub fn export_fingerprint(html: &str) -> String {
        let empreinte = paquet_embarque(html).unwrap_or(html);
        crate::ai_dictionary::hash_text(empreinte.as_bytes())
    }

    /// Exporte le package en JSON universel
    pub fn export_package_json(game: &GameState, title: &str, author: &str) -> Result<String, serde_json::Error> {
        let package = Self::create_package(game, title, author);
        package.to_json_pretty()
    }

    /// Importe et applique un package de jeu dans l'état du jeu.
    ///
    /// **Paquet multi-niveaux** (`scenes` non vide) : reconstruit un jeu complet
    /// à partir du niveau 1 (`scenes[0]`), installe tous les niveaux, l'union
    /// des graphes KVL, les variables initiales, les assets embarqués et les
    /// écrans UI déclaratifs, puis place le joueur sur l'écran `Menu` (il lance
    /// la partie avec `Entrée`).
    ///
    /// **Ancien paquet** (`scenes` vide, champ absent) : comportement historique
    /// inchangé — la scène du paquet remplace l'état courant et l'éditeur passe
    /// en mode Jeu, sans forcer d'écran. Les écrans du paquet sont installés
    /// s'il en transporte ; un paquet d'avant `ui_screens` laisse ceux de
    /// l'instance cible intacts (champ absent = `Vec` vide).
    #[allow(dead_code)]
    pub fn import_package(game: &mut GameState, package: &KaractiGamePackage) {
        // --- Format multi-niveaux ------------------------------------------
        if let Some(premiere) = package.scenes.first() {
            if let Ok(mut nouveau) =
                GameState::from_scene_data(premiere.clone(), "paquet:niveau_1")
            {
                nouveau.levels = package.scenes.clone();
                nouveau.level_index = 0;
                nouveau.scene_name = premiere.name.clone();
                nouveau.scene_objective = premiere.objective.clone();
                // Les écrans voyagent avec le paquet comme les niveaux : l'état
                // reconstruit part d'une scène nue (`ui_screens` vide), il faut
                // donc les y recopier. Les reconstructions suivantes de
                // `start_game`, `next_level` et `reload` recopient à leur tour
                // `ui_screens` depuis l'état courant (même traitement que
                // `levels`/`project_id`), donc ils survivent au jeu.
                nouveau.ui_screens = package.ui_screens.clone();
                // Union calculée à l'export : elle couvre aussi les niveaux que
                // le disque local ne peut pas résoudre (cas wasm/navigateur).
                nouveau.kvl_graphs = package.kvl_graphs.clone();
                for (k, v) in &package.initial_variables {
                    nouveau.kvl_context.set_var(k.clone(), v.clone());
                }

                // Mémoire du paquet : graphes, variables initiales et assets
                // bruts doivent survivre aux reconstructions de `start_game`,
                // `next_level` et `reload` (qui repartent d'une scène nue).
                nouveau.exported_kvl_graphs = package.kvl_graphs.clone();
                nouveau.exported_initial_variables = package.initial_variables.clone();

                let (sprites, sheets) = decoder_assets_embarques(package);
                nouveau.embedded_sprites = sprites.clone();
                nouveau.embedded_sheets = sheets.clone();
                nouveau.apply_embedded_assets(sprites, sheets);
                // Sources SVG : `from_scene_data` les a cherchées sur disque
                // (échec garanti sous wasm) ; on les réinstalle depuis le
                // paquet pour que les `SvgRef` soient rastérisés malgré tout.
                // Les clips `.anim.json` voyagent dans la même passe : sans
                // eux, le rig importé retomberait sur `idle`/`walk` générés.
                nouveau.apply_embedded_svg_package(
                    package.svg_assets.clone(),
                    package.svg_anim_assets.clone(),
                );

                // Réinitialise la caméra et la sélection du Studio.
                nouveau.editor.selected = crate::editor::SelectedEntity::None;
                nouveau.editor.mode = crate::editor::EditorMode::Play;

                // Le joueur voit le menu, puis `Entrée` démarre le niveau 1.
                nouveau.screen = crate::screens::GameScreen::Menu;
                nouveau.sync_screen_variables();

                // Le mode embarqué est une propriété de session (comme l'audio) :
                // dans le HTML autonome, l'instance est embarquée et l'import ne
                // doit pas réafficher le chrome du Studio autour du jeu.
                nouveau.editor.embedded_mode = game.editor.embedded_mode;

                // L'audio et le replay sont des ressources de session : on garde
                // le même périphérique et la même session qu'avant l'import.
                std::mem::swap(&mut nouveau.audio, &mut game.audio);
                std::mem::swap(&mut nouveau.replay, &mut game.replay);

                let total_entites: usize =
                    package.scenes.iter().map(|scene| scene.entities.len()).sum();
                *game = nouveau;
                game.set_status(format!(
                    "🎮 Jeu '{}' chargé avec succès ! ({} niveaux, {} entités, {} graphes KVL)",
                    package.metadata.title,
                    package.scenes.len(),
                    total_entites,
                    package.kvl_graphs.len()
                ));
                return;
            }
            // Reconstruction impossible (scène invalide) : on retombe sur le
            // comportement historique plutôt que de laisser l'état à moitié importé.
        }

        // --- Ancien format mono-scène (scenes vide) --------------------------
        game.scene_name = package.scene_name.clone();
        game.scene_objective = package.scene_objective.clone();
        game.entities = package.entities.clone();
        game.kvl_graphs = package.kvl_graphs.clone();
        // Les écrans d'un ancien paquet qui les transporte sont réinstallés.
        // Un paquet antérieur au champ (`ui_screens` vide) ne doit pas effacer
        // les écrans de l'instance cible : on ne remplace que s'il en fournit.
        if !package.ui_screens.is_empty() {
            game.ui_screens = package.ui_screens.clone();
        }

        for (k, v) in &package.initial_variables {
            game.kvl_context.set_var(k.clone(), v.clone());
        }

        // Même mémoire de paquet que le format multi-niveaux : un futur
        // `start_game`/`reload` ne doit pas perdre les assets déjà installés.
        game.exported_kvl_graphs = package.kvl_graphs.clone();
        game.exported_initial_variables = package.initial_variables.clone();

        // Décodage des assets embarqués + re-résolution des sprites/spritesheets.
        let (sprites, sheets) = decoder_assets_embarques(package);
        game.embedded_sprites = sprites.clone();
        game.embedded_sheets = sheets.clone();
        game.apply_embedded_assets(sprites, sheets);
        // Même installation des sources SVG et des clips d'animation que le
        // format multi-niveaux : un ancien paquet sans ces champs laisse
        // simplement la mémoire vide et les sprites déjà résolus sur disque
        // intacts.
        game.apply_embedded_svg_package(
            package.svg_assets.clone(),
            package.svg_anim_assets.clone(),
        );

        // Réinitialise la caméra et la sélection du Studio
        game.editor.selected = crate::editor::SelectedEntity::None;
        game.editor.mode = crate::editor::EditorMode::Play;
        game.set_status(format!("🎮 Jeu '{}' chargé avec succès ! ({} entités, {} graphes KVL)", package.metadata.title, package.entities.len(), package.kvl_graphs.len()));
    }
}

/// Vérifie l'existence d'un fichier relatif au dépôt.
///
/// Même motif de résolution que le reste du projet : chemin courant, puis
/// `../../<chemin>` (les tests s'exécutent depuis `crates/engine-core`), puis la
/// racine du dépôt pour rester indépendant du cwd.
fn fichier_existe(rel: &str) -> bool {
    if Path::new(rel).is_file() {
        return true;
    }
    if Path::new(&format!("../../{rel}")).is_file() {
        return true;
    }
    let racine = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    Path::new(&format!("{racine}/{rel}")).is_file()
}

/// Racine disque du manifeste : répertoire courant, puis `../../` quand les
/// chemins n'y répondent pas (même motif que [`fichier_existe`]).
///
/// Une entrée absente partout laisse la racine sur le répertoire courant : le
/// diagnostic `fichier-absent` cite alors un chemin stable et compréhensible.
fn racine_du_manifeste(manifeste: &crate::assets_manifest::AssetsManifest) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if manifeste
        .entries
        .iter()
        .any(|entree| cwd.join(&entree.path).is_file())
    {
        return cwd;
    }
    let remontee = cwd.join("../..");
    if manifeste
        .entries
        .iter()
        .any(|entree| remontee.join(&entree.path).is_file())
    {
        return remontee;
    }
    cwd
}

/// `true` si au moins une entité référence un script ou un asset de rendu
/// externe (SVG, PNG, spritesheet, animation) : c'est le cas où l'absence de
/// manifeste mérite un avertissement de pré-vol.
fn entites_referencent_des_assets(game: &GameState) -> bool {
    entites_exportees(game).any(|entite| {
        entite
            .script
            .as_ref()
            .is_some_and(|script| !script.script_path.trim().is_empty())
            || entite.renderable.as_ref().is_some_and(|rendu| {
                matches!(
                    rendu.shape,
                    ShapeKind::SvgRef { .. }
                        | ShapeKind::ImageRef { .. }
                        | ShapeKind::SheetRef { .. }
                        | ShapeKind::AnimRef { .. }
                )
            })
    })
}

/// Vérifie qu'un sprite vectoriel référencé par `SvgRef` est résolvable sur
/// disque (chemin explicite en `.svg` ou identifiant sous `assets/sprites/`).
fn asset_svg_existe(asset: &str) -> bool {
    if fichier_existe(asset) {
        return true;
    }
    let rel = if asset.ends_with(".svg") {
        asset.to_string()
    } else {
        format!("assets/sprites/{asset}.svg")
    };
    fichier_existe(&rel)
}

/// Marqueur du paquet JSON embarqué dans le template HTML standalone.
const BALISE_PAQUET: &str = "<script id=\"embedded-package\" type=\"application/json\">";

/// Extrait le JSON embarqué entre `BALISE_PAQUET` et `</script>`, s'il existe.
fn paquet_embarque(html: &str) -> Option<&str> {
    let debut = html.find(BALISE_PAQUET)? + BALISE_PAQUET.len();
    let fin = html[debut..].find("</script>")? + debut;
    let paquet = html[debut..fin].trim();
    if paquet.is_empty() {
        None
    } else {
        Some(paquet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_metadata_serialization() {
        let meta = PackageMetadata::default();
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: PackageMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, deserialized);
        assert_eq!(deserialized.target_fps, 60);
        assert_eq!(deserialized.screen_width, 1200);
    }

    #[test]
    fn test_standalone_package_creation_and_json_roundtrip() {
        let mut graphs = HashMap::new();
        graphs.insert("script_door".to_string(), Graph::new("door_interaction", "Interactions Porte", "Logique de porte"));

        let package = KaractiGamePackage {
            metadata: PackageMetadata {
                title: "L'Éveil du Héros".to_string(),
                author: "Studio Karacti".to_string(),
                version: "1.0.0".to_string(),
                genre: "Aventure".to_string(),
                created_at: "2026-10-16".to_string(),
                target_fps: 60,
                screen_width: 1200,
                screen_height: 800,
                play_mode_only: true,
            },
            scene_name: "Donjon Oublié".to_string(),
            scene_objective: Some("Trouver la clé et vaincre le gardien".to_string()),
            entities: Vec::new(),
            scenes: Vec::new(),
            ui_screens: Vec::new(),
            kvl_graphs: graphs,
            initial_variables: HashMap::new(),
            sprite_assets: HashMap::new(),
            sheet_assets: HashMap::new(),
            svg_assets: HashMap::new(),
            svg_anim_assets: HashMap::new(),
        };

        let json = package.to_json_pretty().expect("Sérialisation package");
        assert!(json.contains("L'Éveil du Héros"));
        assert!(json.contains("Studio Karacti"));

        let reloaded = KaractiGamePackage::from_json(&json).expect("Désérialisation package");
        assert_eq!(package, reloaded);
    }

    #[test]
    fn test_generate_standalone_html_content() {
        let package = KaractiGamePackage {
            metadata: PackageMetadata {
                title: "Test Quest".to_string(),
                author: "Tester".to_string(),
                version: "1.0.0".to_string(),
                genre: "Platformer".to_string(),
                created_at: "2026-10-16".to_string(),
                target_fps: 60,
                screen_width: 1200,
                screen_height: 800,
                play_mode_only: true,
            },
            scene_name: "Niveau 1".to_string(),
            scene_objective: None,
            entities: Vec::new(),
            scenes: Vec::new(),
            ui_screens: Vec::new(),
            kvl_graphs: HashMap::new(),
            initial_variables: HashMap::new(),
            sprite_assets: HashMap::new(),
            sheet_assets: HashMap::new(),
            svg_assets: HashMap::new(),
            svg_anim_assets: HashMap::new(),
        };

        let html = package.generate_standalone_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Quest"));
        assert!(html.contains("Tester"));
        assert!(html.contains("canvas id=\"game-canvas\""));
        assert!(html.contains("id=\"embedded-package\""));
    }

    /// Charge le niveau de référence P1 depuis le cwd, avec repli `../../`
    /// (cargo exécute les tests depuis `crates/engine-core`).
    fn jeu_reference() -> GameState {
        let candidats = [
            "assets/scenes/ref_niveau_1.scene.json",
            "../../assets/scenes/ref_niveau_1.scene.json",
        ];
        for chemin in candidats {
            if Path::new(chemin).is_file() {
                return GameState::load_from_scene(chemin)
                    .expect("la scène de référence doit être chargeable");
            }
        }
        panic!("scène de référence introuvable : assets/scenes/ref_niveau_1.scene.json");
    }

    /// Charge le projet de référence P1 (3 niveaux) dans un `GameState`.
    fn jeu_reference_projet() -> GameState {
        let candidats = [
            "assets/projects/jeu_reference.karacti",
            "../../assets/projects/jeu_reference.karacti",
        ];
        for chemin in candidats {
            if Path::new(chemin).is_file() {
                let projet = crate::project::KaractiProject::load_from_file(chemin)
                    .expect("le projet de référence doit être lisible");
                let mut jeu = jeu_reference();
                jeu.load_project(projet);
                return jeu;
            }
        }
        panic!("projet de référence introuvable : assets/projects/jeu_reference.karacti");
    }

    /// Scripts référencés par les trois niveaux du jeu de référence.
    const SCRIPTS_REFERENCE: [&str; 4] = [
        "assets/scripts/coin_pickup.kvl.json",
        "assets/scripts/ref_porte_niveau.kvl.json",
        "assets/scripts/ref_boss_verrou.kvl.json",
        "assets/scripts/ref_porte_boss.kvl.json",
    ];

    #[test]
    fn test_paquet_jeu_reference_contient_les_trois_niveaux() {
        let jeu = jeu_reference_projet();
        assert_eq!(jeu.levels.len(), 3, "le projet de référence déclare 3 scènes");

        let package = StandaloneExporter::create_package(&jeu, "Référence", "Testeur");
        assert_eq!(package.scenes.len(), 3, "les 3 niveaux sont sérialisés");
        assert_eq!(package.scenes[0].name, jeu.levels[0].name);
        assert_eq!(package.scenes[2].name, jeu.levels[2].name);
        // Compatibilité : entities/scene_name restent la scène courante.
        assert_eq!(package.scene_name, jeu.scene_name);
        assert_eq!(package.entities.len(), jeu.entities.len());

        // Union des graphes des niveaux 1, 2 et 3.
        for script in SCRIPTS_REFERENCE {
            assert!(
                package.kvl_graphs.contains_key(script),
                "graphe « {script} » absent de l'union : {:?}",
                package.kvl_graphs.keys().collect::<Vec<_>>()
            );
        }
        assert_eq!(package.kvl_graphs.len(), SCRIPTS_REFERENCE.len());

        // Le JSON conserve les niveaux et reste relisible.
        let relu = KaractiGamePackage::from_json(&package.to_json().expect("sérialisation"))
            .expect("désérialisation");
        assert_eq!(relu.scenes.len(), 3, "scenes traverse le JSON");
        assert_eq!(relu.kvl_graphs.len(), SCRIPTS_REFERENCE.len());
        assert_eq!(relu.ui_screens.len(), 4, "ui_screens traverse le JSON");
    }

    #[test]
    fn test_paquet_jeu_reference_embarque_les_ecrans_ui() {
        let jeu = jeu_reference_projet();
        let package = StandaloneExporter::create_package(&jeu, "Référence", "Testeur");

        assert!(
            !package.ui_screens.is_empty(),
            "le paquet doit embarquer les écrans ui_screens du projet"
        );
        assert_eq!(
            package.ui_screens.len(),
            4,
            "menu, pause, game_over et victory : {:?}",
            package
                .ui_screens
                .iter()
                .map(|screen| screen.kind)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            package.ui_screens,
            jeu.ui_screens,
            "les écrans du paquet sont ceux du projet, dans le même ordre"
        );
        // Le menu doit voyager : sans lui, l'import forcerait `Menu` sans rien
        // pouvoir dessiner.
        let menu = package
            .ui_screens
            .iter()
            .find(|screen| screen.kind == crate::subsystems::UiScreenKind::Menu)
            .expect("l'écran Menu est embarqué");
        assert_eq!(menu.title.as_deref(), Some("LES ÉMERAUDES DU TEMPLE"));
    }

    #[test]
    fn test_paquet_sans_scenes_reste_importable_et_garde_l_ancien_comportement() {
        let jeu = jeu_reference();
        let package = StandaloneExporter::create_package(&jeu, "Ancien format", "Testeur");
        assert!(package.scenes.is_empty(), "scène isolée : aucun niveau embarqué");

        // Simule un ancien paquet : les champs `scenes` et `ui_screens` sont
        // totalement absents (format antérieur aux deux fonctionnalités).
        let mut valeur = serde_json::to_value(&package).expect("package en JSON");
        let objet = valeur.as_object_mut().expect("objet JSON");
        assert!(objet.remove("scenes").is_some(), "champ scenes présent avant retrait");
        assert!(objet.remove("ui_screens").is_some(), "champ ui_screens présent avant retrait");
        let json_ancien = serde_json::to_string(&valeur).expect("JSON compact");
        assert!(!json_ancien.contains("\"scenes\""), "vieux format sans scenes");
        assert!(!json_ancien.contains("\"ui_screens\""), "vieux format sans ui_screens");

        let ancien = KaractiGamePackage::from_json(&json_ancien)
            .expect("un ancien paquet reste importable");
        assert!(ancien.scenes.is_empty(), "#[serde(default)] remplit avec un Vec vide");
        assert!(
            ancien.ui_screens.is_empty(),
            "#[serde(default)] remplit ui_screens avec un Vec vide"
        );

        // Import dans un état neuf : comportement historique (mono-scène, mode Jeu).
        let mut cible = jeu_reference();
        cible.levels.clear();
        cible.screen = crate::screens::GameScreen::Playing;
        // Écran préexistant de l'instance cible : un vieux paquet ne doit pas
        // l'effacer (champ absent = aucun écran transporté).
        cible.ui_screens = vec![crate::subsystems::UiScreen {
            id: "menu_cible".to_string(),
            kind: crate::subsystems::UiScreenKind::Menu,
            title: Some("MENU DE L'INSTANCE".to_string()),
            elements: Vec::new(),
        }];
        StandaloneExporter::import_package(&mut cible, &ancien);
        assert_eq!(cible.entities.len(), package.entities.len());
        assert_eq!(cible.scene_name, package.scene_name);
        assert_eq!(cible.kvl_graphs.len(), package.kvl_graphs.len());
        assert_eq!(cible.editor.mode, crate::editor::EditorMode::Play);
        assert_eq!(
            cible.ui_screens.len(),
            1,
            "un ancien paquet sans ui_screens laisse les écrans de la cible intacts"
        );
        assert_eq!(cible.ui_screens[0].id, "menu_cible");
        assert_eq!(
            cible.screen,
            crate::screens::GameScreen::Playing,
            "l'ancien format ne force pas l'écran Menu"
        );
    }

    #[test]
    fn test_import_paquet_jeu_reference_reconstruit_trois_niveaux() {
        let source = jeu_reference_projet();
        let package = StandaloneExporter::create_package(&source, "Référence", "Testeur");

        // Instance neuve, comme au démarrage du runtime wasm.
        let mut neuf = jeu_reference();
        assert!(neuf.levels.is_empty());
        neuf.editor.embedded_mode = true; // HTML autonome : pas de chrome Studio

        StandaloneExporter::import_package(&mut neuf, &package);

        assert!(
            neuf.editor.embedded_mode,
            "le mode embarqué de la session survit à l'import (HTML autonome)"
        );

        assert_eq!(neuf.levels.len(), 3, "les 3 niveaux sont installés");
        assert_eq!(neuf.level_index, 0);
        assert_eq!(neuf.scene_name, package.scenes[0].name);
        assert_eq!(
            neuf.entities.len(),
            package.scenes[0].to_entities().len(),
            "le niveau 1 est instancié"
        );
        assert_eq!(
            neuf.kvl_graphs.len(),
            package.kvl_graphs.len(),
            "l'union des graphes du paquet est installée"
        );
        assert_eq!(
            neuf.ui_screens, package.ui_screens,
            "les écrans UI du paquet sont installés dans l'instance neuve"
        );
        assert!(!neuf.ui_screens.is_empty(), "l'instance importée a bien des écrans");
        assert_eq!(neuf.screen, crate::screens::GameScreen::Menu);
        assert_eq!(neuf.editor.mode, crate::editor::EditorMode::Play);

        // `Entrée` sur le menu → niveau 1 en jeu. Les écrans survivent à la
        // reconstruction de `start_game` (recopiés comme `levels`/`project_id`).
        neuf.start_game();
        assert_eq!(neuf.screen, crate::screens::GameScreen::Playing);
        assert_eq!(neuf.level_index, 0);
        assert_eq!(neuf.scene_name, "Niveau 1 : Salle des Émeraudes");
        assert!(
            neuf.editor.embedded_mode,
            "start_game conserve le mode embarqué (pas de chrome Studio dans l'export)"
        );
        assert_eq!(
            neuf.ui_screens, package.ui_screens,
            "start_game conserve les écrans UI"
        );

        // Les niveaux suivants restent disponibles (le jeu est complet).
        assert!(neuf.next_level(), "passage au niveau 2");
        assert_eq!(neuf.level_index, 1);
        assert!(
            neuf.editor.embedded_mode,
            "next_level conserve le mode embarqué"
        );
        assert_eq!(
            neuf.ui_screens, package.ui_screens,
            "next_level conserve les écrans UI"
        );
        assert!(neuf.next_level(), "passage au niveau 3");
        assert_eq!(neuf.level_index, 2);
        assert_eq!(neuf.scene_name, "Niveau 3 : Arène du Titan");
        assert!(!neuf.next_level(), "dernier niveau : plus de suivant");
    }

    #[test]
    fn test_rendu_apres_import_dessine_le_menu() {
        use crate::screens::GameScreen;

        let source = jeu_reference_projet();
        let package = StandaloneExporter::create_package(&source, "Référence", "Testeur");
        let mut neuf = jeu_reference();
        neuf.editor.embedded_mode = true; // comme le runtime du HTML autonome
        StandaloneExporter::import_package(&mut neuf, &package);
        assert_eq!(neuf.screen, GameScreen::Menu);
        assert!(!neuf.ui_screens.is_empty());

        // Canvas factice : le rendu ne doit pas paniquer et doit réellement
        // dessiner le menu (voile + panneau + titre + entrées).
        let mut canvas = crate::renderer::Canvas::new(1200, 800);
        neuf.render(&mut canvas);

        let premiere = canvas.buffer[0];
        assert!(
            canvas.buffer.iter().any(|&pixel| pixel != premiere),
            "le rendu après import doit produire des pixels non uniformes (menu dessiné)"
        );
        let couleurs: HashSet<u32> = canvas.buffer.iter().copied().collect();
        assert!(
            couleurs.len() > 8,
            "le menu doit écrire plusieurs couleurs (titre, entrées, voile), trouvé {}",
            couleurs.len()
        );
        // Le voile de l'overlay recouvre tout le viewport : preuve que l'écran
        // courant est bien dessiné depuis `ui_screens`.
        let voile = canvas
            .buffer
            .iter()
            .filter(|&&pixel| pixel == 0x00060911)
            .count();
        assert!(voile > 100_000, "voile du menu attendu, trouvé {voile} pixels");
    }

    #[test]
    fn test_paquet_multi_niveaux_sans_ui_screens_reste_importable() {
        use crate::screens::GameScreen;

        let source = jeu_reference_projet();
        let package = StandaloneExporter::create_package(&source, "Ancien format", "Testeur");
        assert!(!package.scenes.is_empty());

        // Simule un paquet multi-niveaux d'avant `ui_screens`.
        let mut valeur = serde_json::to_value(&package).expect("package en JSON");
        valeur
            .as_object_mut()
            .expect("objet JSON")
            .remove("ui_screens")
            .expect("champ ui_screens présent avant retrait");
        let ancien = KaractiGamePackage::from_json(&serde_json::to_string(&valeur).expect("JSON"))
            .expect("un ancien paquet multi-niveaux reste importable");
        assert!(ancien.ui_screens.is_empty());

        let mut neuf = jeu_reference();
        StandaloneExporter::import_package(&mut neuf, &ancien);
        assert_eq!(neuf.levels.len(), 3, "les niveaux restent importés");
        assert!(
            neuf.ui_screens.is_empty(),
            "aucun écran n'est inventé pour un ancien paquet"
        );
        assert_eq!(neuf.screen, GameScreen::Menu, "l'import force toujours Menu");
        // Aucun écran à dessiner : le rendu ne doit pas paniquer.
        let mut canvas = crate::renderer::Canvas::new(1200, 800);
        neuf.render(&mut canvas);
        neuf.start_game();
        assert_eq!(neuf.screen, GameScreen::Playing);
        assert!(neuf.ui_screens.is_empty());
    }

    /// Script volontairement absent du disque : seul le paquet le fournit.
    const SCRIPT_PAQUET: &str = "assets/scripts/script_du_paquet_absent_du_disque.kvl.json";
    /// Script réellement présent sur disque : sa version doit gagner.
    const SCRIPT_DISQUE: &str = "assets/scripts/coin_pickup.kvl.json";
    const SPRITE_PAQUET: &str = "sprite_du_paquet";
    const SHEET_PAQUET: &str = "sheet_du_paquet";

    /// PNG RGBA 1×1 valide : les caches ne retiennent que les assets décodables.
    fn png_un_pixel() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 1, 1);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("en-tête PNG");
            writer.write_image_data(&[11, 22, 33, 255]).expect("données PNG");
        }
        out
    }

    /// Scène de test : un script fourni par le paquet (absent du disque), un
    /// sprite et une spritesheet embarqués, et un script résolu sur disque.
    fn scene_du_paquet(nom: &str) -> SceneData {
        use crate::entity::{Entity, KVLScriptComponent, Renderable2D, Transform2D};

        SceneData {
            name: nom.to_string(),
            description: String::new(),
            objective: Some("Vérifier la réinjection du paquet".to_string()),
            entities: vec![
                Entity::new(
                    1,
                    "Porte du paquet",
                    "door",
                    Transform2D::new(64.0, 64.0, 32.0, 32.0),
                )
                .with_renderable(Renderable2D::image_ref(SPRITE_PAQUET, 1))
                .with_script(KVLScriptComponent::new(SCRIPT_PAQUET).with_interact("OnDoorInteract")),
                Entity::new(
                    2,
                    "Sprite animé du paquet",
                    "decor",
                    Transform2D::new(96.0, 64.0, 16.0, 16.0),
                )
                .with_renderable(Renderable2D::sheet_ref(SHEET_PAQUET, "idle", 2)),
                Entity::new(
                    3,
                    "Pièce sur disque",
                    "collectible",
                    Transform2D::new(128.0, 64.0, 16.0, 16.0),
                )
                .with_script(KVLScriptComponent::new(SCRIPT_DISQUE).with_overlap("OnPlayerOverlap")),
            ],
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        }
    }

    /// Paquet multi-niveaux minimal, construit « depuis une scène de test ».
    fn paquet_de_test() -> KaractiGamePackage {
        let scenes = vec![
            scene_du_paquet("NIVEAU PAQUET 1"),
            scene_du_paquet("NIVEAU PAQUET 2"),
        ];

        let mut kvl_graphs = HashMap::new();
        let mut graphe = Graph::new(
            "graphe_paquet",
            "Graphe du paquet",
            "Logique embarquée dans le paquet",
        );
        graphe.add_variable("variable_graphe_paquet", Value::Int(7));
        kvl_graphs.insert(SCRIPT_PAQUET.to_string(), graphe);

        // Version « paquet » du script présent sur disque : elle ne doit jamais
        // écraser la version résolue localement.
        let mut graphe_disque = Graph::new(
            "version_paquet",
            "Version paquet",
            "Ne doit pas écraser le disque",
        );
        graphe_disque.add_variable("variable_ecrasee_par_le_paquet", Value::Int(123));
        kvl_graphs.insert(SCRIPT_DISQUE.to_string(), graphe_disque);

        let mut sprite_assets = HashMap::new();
        sprite_assets.insert(SPRITE_PAQUET.to_string(), STANDARD.encode(png_un_pixel()));
        let mut sheet_assets = HashMap::new();
        sheet_assets.insert(
            SHEET_PAQUET.to_string(),
            SheetAssetExport {
                png: STANDARD.encode(png_un_pixel()),
                json: r#"{"frames":{"idle":{"frame":{"x":0,"y":0,"w":1,"h":1}}}}"#
                    .to_string(),
            },
        );
        let mut initial_variables = HashMap::new();
        initial_variables.insert("variable_initiale_paquet".to_string(), Value::Int(99));

        KaractiGamePackage {
            metadata: PackageMetadata {
                title: "Paquet de test".to_string(),
                ..PackageMetadata::default()
            },
            scene_name: scenes[0].name.clone(),
            scene_objective: scenes[0].objective.clone(),
            entities: scenes[0].to_entities(),
            scenes,
            ui_screens: Vec::new(),
            kvl_graphs,
            initial_variables,
            sprite_assets,
            sheet_assets,
            svg_assets: HashMap::new(),
            svg_anim_assets: HashMap::new(),
        }
    }

    #[test]
    fn test_scene_native_ne_porte_aucune_memoire_de_paquet() {
        let jeu = GameState::from_scene_data(scene_du_paquet("NATIF"), "test:natif")
            .expect("scène de test");

        assert!(jeu.exported_kvl_graphs.is_empty());
        assert!(jeu.exported_initial_variables.is_empty());
        assert!(jeu.embedded_sprites.is_empty());
        assert!(jeu.embedded_sheets.is_empty());
        assert!(jeu.embedded_svg_assets.is_empty());
        assert!(jeu.embedded_svg_anims.is_empty());
    }

    #[test]
    fn test_import_package_reinjecte_graphes_variables_et_assets_a_travers_les_niveaux() {
        use crate::screens::GameScreen;

        let package = paquet_de_test();
        let mut jeu = GameState::from_scene_data(scene_du_paquet("NEUF"), "test:neuf")
            .expect("état de test");
        assert!(jeu.levels.is_empty());

        StandaloneExporter::import_package(&mut jeu, &package);
        assert_eq!(jeu.screen, GameScreen::Menu);
        assert_eq!(jeu.exported_kvl_graphs.len(), package.kvl_graphs.len());
        assert_eq!(jeu.embedded_sprites.len(), 1);
        assert_eq!(jeu.embedded_sheets.len(), 1);

        // `Entrée` (start_game) : niveau 1 reconstruit, mémoire réinjectée.
        jeu.start_game();
        assert_eq!(jeu.screen, GameScreen::Playing);
        assert!(
            jeu.kvl_graphs.contains_key(SCRIPT_PAQUET),
            "graphe du paquet présent après start_game : {:?}",
            jeu.kvl_graphs.keys().collect::<Vec<_>>()
        );
        assert!(
            jeu.sprite_cache.get(SPRITE_PAQUET).is_some(),
            "sprite embarqué installé après start_game"
        );
        assert!(
            jeu.sheet_cache.get(SHEET_PAQUET).is_some(),
            "spritesheet embarquée installée après start_game"
        );
        assert_eq!(
            jeu.kvl_context.get_var("variable_graphe_paquet"),
            Some(&Value::Int(7)),
            "variables initiales du graphe du paquet"
        );
        assert_eq!(
            jeu.kvl_context.get_var("variable_initiale_paquet"),
            Some(&Value::Int(99)),
            "variables initiales du paquet"
        );

        // `next_level` : la scène du niveau 2 est reconstruite à son tour.
        assert!(jeu.next_level(), "passage au niveau 2");
        assert!(
            jeu.kvl_graphs.contains_key(SCRIPT_PAQUET),
            "graphe du paquet présent après next_level"
        );
        assert!(
            jeu.sprite_cache.get(SPRITE_PAQUET).is_some(),
            "sprite embarqué toujours installé après next_level"
        );
        assert!(
            jeu.sheet_cache.get(SHEET_PAQUET).is_some(),
            "spritesheet embarquée toujours installée après next_level"
        );
        assert_eq!(
            jeu.kvl_context.get_var("variable_initiale_paquet"),
            Some(&Value::Int(99)),
            "variables du paquet après next_level"
        );
        assert!(
            !jeu.exported_kvl_graphs.is_empty(),
            "la mémoire du paquet survit à la reconstruction"
        );

        // `reload` : même scène, même exigence.
        jeu.reload();
        assert!(
            jeu.kvl_graphs.contains_key(SCRIPT_PAQUET),
            "graphe du paquet présent après reload"
        );
        assert!(
            jeu.sprite_cache.get(SPRITE_PAQUET).is_some(),
            "sprite embarqué toujours installé après reload"
        );
        assert!(
            jeu.sheet_cache.get(SHEET_PAQUET).is_some(),
            "spritesheet embarquée toujours installée après reload"
        );
        assert_eq!(
            jeu.kvl_context.get_var("variable_graphe_paquet"),
            Some(&Value::Int(7)),
            "variables du graphe du paquet après reload"
        );
        assert_eq!(
            jeu.kvl_context.get_var("variable_initiale_paquet"),
            Some(&Value::Int(99)),
            "variables initiales du paquet après reload"
        );
    }

    #[test]
    fn test_graphe_resolu_sur_disque_n_est_pas_ecrase_par_le_paquet() {
        let package = paquet_de_test();
        let mut jeu = GameState::from_scene_data(scene_du_paquet("NEUF"), "test:neuf")
            .expect("état de test");
        StandaloneExporter::import_package(&mut jeu, &package);
        jeu.start_game();

        // Version réellement résolue depuis le disque pour le même chemin.
        let disque = GameState::from_scene_data(jeu.levels[0].clone(), "test:reference")
            .expect("scène fraîche")
            .kvl_graphs
            .get(SCRIPT_DISQUE)
            .cloned()
            .expect("coin_pickup doit être résolu sur disque");

        assert_eq!(
            jeu.kvl_graphs.get(SCRIPT_DISQUE),
            Some(&disque),
            "le graphe du disque garde la priorité après reconstruction"
        );
        assert_ne!(
            jeu.kvl_graphs[SCRIPT_DISQUE].id, "version_paquet",
            "la version du paquet n'a pas écrasé le disque"
        );
        assert!(
            jeu.kvl_context
                .get_var("variable_ecrasee_par_le_paquet")
                .is_none(),
            "les variables du graphe écarté ne sont pas appliquées"
        );
    }

    #[test]
    fn test_paquet_sans_graphes_ni_assets_ne_casse_pas_les_reconstructions() {
        let mut package = paquet_de_test();
        package.kvl_graphs.clear();
        package.initial_variables.clear();
        package.sprite_assets.clear();
        package.sheet_assets.clear();

        let mut jeu = GameState::from_scene_data(scene_du_paquet("NEUF"), "test:neuf")
            .expect("état de test");
        StandaloneExporter::import_package(&mut jeu, &package);
        assert!(jeu.exported_kvl_graphs.is_empty());
        assert!(jeu.exported_initial_variables.is_empty());
        assert!(jeu.embedded_sprites.is_empty());
        assert!(jeu.embedded_sheets.is_empty());
        assert!(jeu.embedded_svg_assets.is_empty());
        assert!(jeu.embedded_svg_anims.is_empty());

        jeu.start_game();
        assert!(!jeu.kvl_graphs.contains_key(SCRIPT_PAQUET));
        assert!(jeu.sprite_cache.get(SPRITE_PAQUET).is_none());
        assert!(jeu.sheet_cache.get(SHEET_PAQUET).is_none());
        assert!(jeu.next_level(), "les niveaux du paquet restent jouables");
        jeu.reload();
        assert_eq!(jeu.levels.len(), 2, "aucun niveau n'a été perdu");
        assert_eq!(jeu.level_index, 1, "le niveau courant est conservé");
    }

    #[test]
    fn test_export_html_projet_multi_niveaux_est_deterministe() {
        let jeu = jeu_reference_projet();
        let premier =
            StandaloneExporter::export_html_with_runtime(&jeu, "Trois niveaux", "Testeur", None);
        let second =
            StandaloneExporter::export_html_with_runtime(&jeu, "Trois niveaux", "Testeur", None);
        assert_eq!(premier, second, "même projet = même fichier");
        assert_eq!(
            StandaloneExporter::export_fingerprint(&premier),
            StandaloneExporter::export_fingerprint(&second)
        );
        // Diagnostic lu par export-smoke.mjs --expect-levels.
        assert!(premier.contains("__karacti_levels"));
    }

    #[test]
    fn test_preflight_projet_multi_niveaux_signale_les_niveaux_sans_bloquer() {
        let jeu = jeu_reference_projet();
        let report = StandaloneExporter::preflight_with_runtime_dir(&jeu, None);
        assert!(report.ok, "pré-vol : {}", report.summary());
        assert_eq!(report.blocking_count(), 0, "rapport : {}", report.summary());
        let issue = report
            .warnings()
            .into_iter()
            .find(|issue| issue.code == "niveaux-exportes")
            .expect("avertissement niveaux-exportes attendu");
        assert!(issue.message.contains('3'), "message : {}", issue.message);
    }

    #[test]
    fn test_preflight_bloque_un_asset_manquant_dans_un_niveau_ulterieur() {
        let mut jeu = jeu_reference_projet();
        let entite = jeu.levels[1].entities.get_mut(0).expect("niveau 2 non vide");
        entite.renderable = Some(crate::entity::Renderable2D::image_ref(
            "assets/sprites/asset_fantome_niveau_2.png",
            1,
        ));

        let report = StandaloneExporter::preflight_with_runtime_dir(&jeu, None);
        assert!(!report.ok, "un asset manquant au niveau 2 doit bloquer");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "asset-manquant"));
    }

    #[test]
    fn test_preflight_bloque_un_script_manquant_dans_un_niveau_ulterieur() {
        let mut jeu = jeu_reference_projet();
        let entite = jeu.levels[2].entities.get_mut(0).expect("niveau 3 non vide");
        entite.script = Some(crate::entity::KVLScriptComponent::new(
            "assets/scripts/script_fantome_niveau_3.kvl.json",
        ));

        let report = StandaloneExporter::preflight_with_runtime_dir(&jeu, None);
        assert!(!report.ok, "un script manquant au niveau 3 doit bloquer");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "script-manquant"));
    }

    #[test]
    fn test_preflight_projet_sain() {
        let jeu = jeu_reference();
        // Répertoire de runtime explicite vide : le test reste indépendant du
        // vrai `dist/pkg` (ni `runtime-embarque`, ni `runtime-perime`).
        let report = StandaloneExporter::preflight_with_runtime_dir(&jeu, None);
        assert!(report.ok, "pré-vol du niveau de référence : {}", report.summary());
        assert_eq!(report.blocking_count(), 0, "aucun blocage attendu : {}", report.summary());
        // Le niveau de référence référence des scripts mais ne déclare pas de
        // manifeste : seul l'avertissement `manifeste-absent` est attendu.
        assert_eq!(report.warnings().len(), 1, "rapport : {}", report.summary());
        assert_eq!(report.issues[0].code, "manifeste-absent");
    }

    #[test]
    fn test_preflight_signale_manifeste_absent_sans_bloquer() {
        let jeu = jeu_reference();
        assert!(
            jeu.entities.iter().any(|entite| entite.script.is_some()),
            "le jeu de référence doit référencer au moins un script"
        );
        assert!(jeu.assets_manifest.is_none(), "aucun manifeste par défaut");

        let report = StandaloneExporter::preflight(&jeu);
        let avertissement = report
            .warnings()
            .into_iter()
            .find(|issue| issue.code == "manifeste-absent")
            .expect("un avertissement manifeste-absent est attendu");
        assert!(avertissement.message.contains("manifeste"));
        assert!(report.ok, "l'absence de manifeste ne bloque pas l'export");
        assert_eq!(report.blocking_count(), 0);
    }

    #[test]
    fn test_preflight_sans_manifeste_ni_assets_reste_silencieux() {
        let mut jeu = jeu_reference();
        for entite in &mut jeu.entities {
            entite.script = None;
            if let Some(rendu) = &mut entite.renderable {
                rendu.shape = ShapeKind::Rect;
            }
        }
        assert!(!entites_referencent_des_assets(&jeu));

        let report = StandaloneExporter::preflight(&jeu);
        assert!(report.ok, "pré-vol : {}", report.summary());
        // Aucun diagnostic lié au contenu ; l'éventuel `runtime-embarque` ne
        // dépend que de la présence des artefacts wasm sur la machine.
        assert!(
            report
                .issues
                .iter()
                .all(|issue| issue.code == "runtime-embarque"),
            "aucun diagnostic attendu sans asset référencé : {}",
            report.summary()
        );
    }

    /// Chemin disque réel d'un asset du dépôt, quel que soit le cwd des tests.
    fn chemin_asset_reference(rel: &str) -> PathBuf {
        for candidat in [PathBuf::from(rel), PathBuf::from(format!("../../{rel}"))] {
            if candidat.is_file() {
                return candidat;
            }
        }
        panic!("asset de référence introuvable : {rel}");
    }

    #[test]
    fn test_preflight_manifeste_fichier_absent_bloque() {
        let mut jeu = jeu_reference();
        let mut manifeste = crate::assets_manifest::AssetsManifest::default();
        manifeste
            .register(crate::assets_manifest::AssetEntry {
                id: "fantome".to_string(),
                kind: crate::assets_manifest::AssetKind::Sprite,
                // Nom unique et volontairement inexistant sous le cwd et ../../.
                path: format!(
                    "assets/sprites/manifeste_prevol_fantome_{}.png",
                    std::process::id()
                ),
                bytes: 128,
                hash: crate::ai_dictionary::hash_text(b"fantome"),
                provenance: crate::assets_manifest::Provenance {
                    source: crate::assets_manifest::ProvenanceSource::Studio,
                    license: Some("CC0-1.0".to_string()),
                    created_at: "2026-11-11".to_string(),
                },
                dimensions: Some((16, 16)),
                duration_ms: None,
            })
            .expect("entrée acceptée");
        jeu.assets_manifest = Some(manifeste);

        let report = StandaloneExporter::preflight(&jeu);
        assert!(!report.ok, "un fichier déclaré absent doit bloquer");
        assert_eq!(report.blocking_count(), 1, "rapport : {}", report.summary());
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.code == "fichier-absent")
            .expect("le diagnostic fichier-absent du manifeste est attendu");
        assert_eq!(issue.severity, PreflightSeverity::Blocking);
        assert!(
            issue.message.starts_with("manifeste : "),
            "message attendu préfixé : {}",
            issue.message
        );
        assert!(issue.message.contains("fantome"));
    }

    #[test]
    fn test_preflight_manifeste_fichier_present_ne_signale_rien() {
        let mut jeu = jeu_reference();
        let rel = "assets/scenes/ref_niveau_1.scene.json";
        let chemin = chemin_asset_reference(rel);
        let (bytes, hash) =
            crate::assets_manifest::verify_file(&chemin).expect("fichier de référence lisible");

        let mut manifeste = crate::assets_manifest::AssetsManifest::default();
        manifeste
            .register(crate::assets_manifest::AssetEntry {
                id: "niveau_reference".to_string(),
                kind: crate::assets_manifest::AssetKind::Sprite,
                path: rel.to_string(),
                bytes,
                hash,
                provenance: crate::assets_manifest::Provenance {
                    source: crate::assets_manifest::ProvenanceSource::Studio,
                    license: Some("CC0-1.0".to_string()),
                    created_at: "2026-11-11".to_string(),
                },
                dimensions: None,
                duration_ms: None,
            })
            .expect("entrée acceptée");
        jeu.assets_manifest = Some(manifeste);

        let report = StandaloneExporter::preflight(&jeu);
        assert!(
            report.ok,
            "le manifeste valide ne doit rien bloquer : {}",
            report.summary()
        );
        assert!(
            report
                .issues
                .iter()
                .all(|issue| !issue.message.starts_with("manifeste : ")),
            "aucun diagnostic de manifeste attendu : {}",
            report.summary()
        );
    }

    #[test]
    fn test_preflight_manifeste_licence_manquante_avertit_sans_bloquer() {
        let mut jeu = jeu_reference();
        let rel = "assets/scenes/ref_niveau_1.scene.json";
        let (bytes, hash) = crate::assets_manifest::verify_file(&chemin_asset_reference(rel))
            .expect("fichier de référence lisible");

        let mut manifeste = crate::assets_manifest::AssetsManifest::default();
        manifeste
            .register(crate::assets_manifest::AssetEntry {
                id: "sans_licence".to_string(),
                kind: crate::assets_manifest::AssetKind::Sprite,
                path: rel.to_string(),
                bytes,
                hash,
                provenance: crate::assets_manifest::Provenance {
                    source: crate::assets_manifest::ProvenanceSource::Studio,
                    license: None,
                    created_at: "2026-11-11".to_string(),
                },
                dimensions: None,
                duration_ms: None,
            })
            .expect("entrée acceptée");
        jeu.assets_manifest = Some(manifeste);

        let report = StandaloneExporter::preflight(&jeu);
        assert!(report.ok, "la licence n'est pas bloquante : {}", report.summary());
        assert_eq!(report.blocking_count(), 0);
        let issue = report
            .warnings()
            .into_iter()
            .find(|issue| issue.code == "licence-manquante")
            .expect("l'avertissement de licence est attendu");
        assert!(issue.message.starts_with("manifeste : "));
        assert!(!issue.message.contains("manifeste-absent"));
    }

    #[test]
    fn test_preflight_script_manquant_bloque() {
        let mut jeu = jeu_reference();
        jeu.entities[0].script = Some(crate::entity::KVLScriptComponent::new(
            "assets/scripts/script_fantome_introuvable.kvl.json",
        ));

        let report = StandaloneExporter::preflight(&jeu);
        assert!(!report.ok);
        assert_eq!(report.blocking_count(), 1, "rapport : {}", report.summary());
        let issue = report
            .issues
            .iter()
            .find(|i| i.code == "script-manquant")
            .expect("un blocage script-manquant est attendu");
        assert_eq!(issue.severity, PreflightSeverity::Blocking);
        assert!(issue.message.contains("script_fantome_introuvable.kvl.json"));
        assert!(issue.message.contains("Joueur"));
        assert!(report.summary().contains("blocage"));
    }

    #[test]
    fn test_export_html_checked_refuse_un_script_manquant() {
        let sain = jeu_reference();
        let html = StandaloneExporter::export_html_checked(&sain, "Niveau 1", "Testeur")
            .expect("le niveau de référence doit s'exporter");
        assert!(html.contains("<!DOCTYPE html>"));

        let mut casse = jeu_reference();
        casse.entities[1].script = Some(crate::entity::KVLScriptComponent::new(
            "assets/scripts/script_fantome_introuvable.kvl.json",
        ));
        let err = StandaloneExporter::export_html_checked(&casse, "Niveau 1", "Testeur")
            .expect_err("un script manquant doit refuser l'export");
        assert!(!err.ok);
        assert!(err.issues.iter().any(|i| i.code == "script-manquant"));
    }

    #[test]
    fn test_preflight_budget_entites_avertit_sans_bloquer() {
        let mut jeu = jeu_reference();
        let modele = jeu.entities.clone();
        assert!(!modele.is_empty());
        let mut i = 0usize;
        while jeu.entities.len() <= 500 {
            let mut copie = modele[i % modele.len()].clone();
            copie.id = jeu.entities.len() + 1;
            jeu.entities.push(copie);
            i += 1;
        }

        let report = StandaloneExporter::preflight(&jeu);
        assert!(report.ok, "un budget dépassé n'est pas bloquant : {}", report.summary());
        assert!(report
            .warnings()
            .iter()
            .any(|a| a.code == "budget-entites"));
        assert!(report
            .issues
            .iter()
            .all(|i| i.severity == PreflightSeverity::Warning));
    }

    #[test]
    fn test_export_fingerprint_stable() {
        let jeu = jeu_reference();
        let a = StandaloneExporter::export_html(&jeu, "Empreinte", "Testeur");
        let b = StandaloneExporter::export_html(&jeu, "Empreinte", "Testeur");
        let fa = StandaloneExporter::export_fingerprint(&a);
        let fb = StandaloneExporter::export_fingerprint(&b);

        assert_eq!(fa, fb, "deux exports du même état doivent partager l'empreinte");
        assert_eq!(fa.len(), 16, "FNV-1a 64 bits encodé en 16 caractères hex");
        assert!(fa.chars().all(|c| c.is_ascii_hexdigit()));

        let autre = StandaloneExporter::export_html(&jeu, "Autre titre", "Testeur");
        assert_ne!(fa, StandaloneExporter::export_fingerprint(&autre));
    }

    /// Répertoire temporaire unique par test, **supprimé automatiquement**.
    ///
    /// Les tests s'exécutent en parallèle : le nom mêle pid et nanosecondes.
    /// Jamais les artefacts réels de 3,5 Mo, seulement de petites chaînes, pour
    /// rester rapide et indépendant de la machine.
    struct RepertoireTemporaire(PathBuf);

    impl RepertoireTemporaire {
        /// Chemin du répertoire, pour les API qui attendent un `Path`.
        fn as_path(&self) -> &Path {
            &self.0
        }
    }

    impl std::ops::Deref for RepertoireTemporaire {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.0
        }
    }

    /// Nettoyage en fin de test, y compris si le test panique (déroulement).
    impl Drop for RepertoireTemporaire {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn repertoire_temporaire(nom: &str) -> RepertoireTemporaire {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duree| duree.as_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "karacti_runtime_{nom}_{}_{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&base).expect("répertoire temporaire créé");
        RepertoireTemporaire(base)
    }

    /// Écrit un faux runtime (`glue` JS + binaire wasm factice) dans `dir`.
    fn ecrire_faux_runtime(dir: &Path, glue: &str, wasm: &[u8]) {
        std::fs::write(dir.join(RUNTIME_GLUE_FILE), glue).expect("glue écrite");
        std::fs::write(dir.join(RUNTIME_WASM_FILE), wasm).expect("wasm écrit");
    }

    /// Écrit un manifeste de build factice (aucun vrai wasm n'est nécessaire).
    fn ecrire_manifeste_runtime(dir: &Path, version: &str, wasm_sha: &str) {
        let manifeste = format!(
            "{{\n  \"engine_version\": \"{version}\",\n  \"wasm_sha256\": \"{wasm_sha}\",\n  \"glue_sha256\": \"0000000000000000000000000000000000000000000000000000000000000000\",\n  \"target\": \"wasm32-unknown-unknown\"\n}}\n"
        );
        std::fs::write(dir.join(RUNTIME_MANIFEST_FILE), manifeste).expect("manifeste écrit");
    }

    #[test]
    fn test_load_embedded_runtime_lit_les_artefacts() {
        let dir = repertoire_temporaire("load");
        let glue = "export function initSync(module) { return module; }\n";
        let wasm = b"\0asm\x01\0\0\0";
        ecrire_faux_runtime(&dir, glue, wasm);

        let runtime = load_embedded_runtime(&dir).expect("runtime factice chargeable");
        assert_eq!(runtime.glue, glue);
        assert_eq!(runtime.wasm_base64, STANDARD.encode(wasm));
        assert_eq!(runtime.bytes, glue.len() + runtime.wasm_base64.len());
        // La taille annoncée est exactement celle du HTML (base64 compris).
        assert_eq!(runtime.bytes, taille_runtime_embarque(&dir).unwrap());
        assert!(runtime.human_size().ends_with(" o"));
    }

    #[test]
    fn test_load_embedded_runtime_renvoie_none_si_absent_ou_vide() {
        let dir = repertoire_temporaire("absent");
        assert!(load_embedded_runtime(&dir).is_none(), "répertoire vide");

        // Glue vide : artefact inutilisable.
        ecrire_faux_runtime(&dir, "   \n", b"\0asm");
        assert!(load_embedded_runtime(&dir).is_none(), "glue vide");

        // Binaire wasm vide.
        ecrire_faux_runtime(&dir, "export const x = 1;", b"");
        assert!(load_embedded_runtime(&dir).is_none(), "wasm vide");

        // Fichier wasm manquant.
        std::fs::remove_file(dir.join(RUNTIME_WASM_FILE)).unwrap();
        assert!(load_embedded_runtime(&dir).is_none(), "wasm absent");
    }

    #[test]
    fn test_echapper_fin_de_script_insensible_a_la_casse() {
        assert_eq!(echapper_fin_de_script("a</script>b"), "a<\\/script>b");
        assert_eq!(echapper_fin_de_script("a</SCRIPT>b"), "a<\\/script>b");
        assert_eq!(echapper_fin_de_script("a</ScRiPt >b"), "a<\\/script >b");
        assert_eq!(echapper_fin_de_script(" rien à échapper "), " rien à échapper ");
    }

    #[test]
    fn test_export_html_with_runtime_injecte_avant_body() {
        let dir = repertoire_temporaire("injecte");
        let glue = "export function initSync() {}\n// piège : const s = \"</script>\";\n";
        ecrire_faux_runtime(&dir, glue, b"\0asm\x01\0\0\0");
        let jeu = jeu_reference();

        let html = StandaloneExporter::export_html_with_runtime(
            &jeu,
            "Runtime",
            "Testeur",
            Some(dir.as_path()),
        );

        assert!(html.contains("id=\"runtime-glue\""));
        assert!(html.contains("id=\"runtime-wasm\" type=\"application/base64\""));
        let injection = html.find("id=\"runtime-glue\"").unwrap();
        let fin_body = html.rfind("</body>").unwrap();
        assert!(injection < fin_body, "le runtime doit être injecté avant </body>");
        // Le paquet est toujours présent avant le runtime.
        assert!(html.find("id=\"embedded-package\"").unwrap() < injection);

        // La glue piégée est échappée : exactement 4 fermetures de script
        // (paquet, module, glue, wasm), aucune coupure prématurée.
        assert_eq!(html.matches("</script>").count(), 4);
        assert!(!html.contains("</SCRIPT>"));
        assert!(html.contains("<\\/script>"), "la glue doit être échappée");

        // Le runtime n'est pas haché : l'empreinte reste celle du paquet.
        assert_eq!(
            StandaloneExporter::export_fingerprint(&html),
            StandaloneExporter::export_fingerprint(
                &StandaloneExporter::export_html_with_runtime(
                    &jeu,
                    "Runtime",
                    "Testeur",
                    None
                )
            )
        );
    }

    #[test]
    fn test_export_html_with_runtime_absent_reste_jouable_en_donnees() {
        let dir = repertoire_temporaire("vide");
        let jeu = jeu_reference();

        for html in [
            StandaloneExporter::export_html_with_runtime(&jeu, "Sans", "Testeur", None),
            StandaloneExporter::export_html_with_runtime(
                &jeu,
                "Sans",
                "Testeur",
                Some(dir.as_path()),
            ),
        ] {
            assert!(!html.contains("id=\"runtime-glue\""));
            assert!(!html.contains("id=\"runtime-wasm\""));
            assert!(html.contains("id=\"embedded-package\""));
            assert!(html.contains("id=\"game-canvas\""));
            // L'import sibling (source de l'erreur CORS) a disparu du template :
            // seul l'import de la Blob URL subsiste.
            assert!(!html.contains("await import('./pkg"));
            assert!(html.contains("URL.createObjectURL(new Blob"));
            assert!(html.contains("putImageData"));
            assert!(html.contains("wasm_last_error"));
        }
    }

    #[test]
    fn test_export_html_with_runtime_est_deterministe() {
        let dir = repertoire_temporaire("deterministe");
        ecrire_faux_runtime(&dir, "export function initSync() {}\n", b"\0asm\x01\0\0\0");
        let jeu = jeu_reference();

        let a = StandaloneExporter::export_html_with_runtime(
            &jeu,
            "Déterminisme",
            "Testeur",
            Some(dir.as_path()),
        );
        let b = StandaloneExporter::export_html_with_runtime(
            &jeu,
            "Déterminisme",
            "Testeur",
            Some(dir.as_path()),
        );
        assert_eq!(a, b, "même projet + mêmes artefacts = même fichier");
        assert_eq!(
            StandaloneExporter::export_fingerprint(&a),
            StandaloneExporter::export_fingerprint(&b)
        );
    }

    #[test]
    fn test_serialisation_json_deterministe_trie_les_cles() {
        // `HashMap` n'a pas d'ordre stable entre processus : le tri des clés
        // est ce qui rend le HTML reproductible.
        let mut table = HashMap::new();
        table.insert("zeta".to_string(), 1u32);
        table.insert("alpha".to_string(), 2u32);
        table.insert("mike".to_string(), 3u32);
        assert_eq!(
            serialiser_json_deterministe(&table).unwrap(),
            r#"{"alpha":2,"mike":3,"zeta":1}"#
        );

        // Les objets imbriqués dans des tableaux sont triés eux aussi.
        let imbrique = serde_json::json!({"z": {"b": 1, "a": 2}, "a": [{"y": 1, "x": 2}]});
        assert_eq!(
            serialiser_json_deterministe(&imbrique).unwrap(),
            r#"{"a":[{"x":2,"y":1}],"z":{"a":2,"b":1}}"#
        );
    }

    #[test]
    fn test_preflight_avertit_runtime_embarque_sans_bloquer() {
        // 4 882 616 octets = glue + base64 d'environ 3,5 Mo de wasm.
        let issue = avertissement_runtime(4_882_616);
        assert_eq!(issue.severity, PreflightSeverity::Warning);
        assert_eq!(issue.code, "runtime-embarque");
        assert!(
            issue.message.starts_with("runtime embarqué : 4,7 Mo"),
            "message attendu avec la taille : {}",
            issue.message
        );

        let dir = repertoire_temporaire("avertissement");
        ecrire_faux_runtime(&dir, "export function initSync() {}\n", b"\0asm\x01\0\0\0");
        assert_eq!(
            taille_runtime_embarque(&dir),
            Some(load_embedded_runtime(&dir).unwrap().bytes)
        );
        assert!(taille_runtime_embarque(&repertoire_temporaire("avertissement_vide")).is_none());
    }

    /// Empreinte factice stable : 64 caractères hexadécimaux, comme un SHA-256.
    const SHA_WASM_FACTICE: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn test_runtime_manifeste_coherent_remonte_version_et_aucun_avertissement() {
        let dir = repertoire_temporaire("manifeste_coherent");
        ecrire_faux_runtime(&dir, "export function initSync() {}\n", b"\0asm\x01\0\0\0");
        ecrire_manifeste_runtime(&dir, env!("CARGO_PKG_VERSION"), SHA_WASM_FACTICE);
        let jeu = jeu_reference();

        let runtime = load_embedded_runtime(&dir).expect("runtime factice chargeable");
        assert_eq!(
            runtime.engine_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(runtime.wasm_sha256.as_deref(), Some(SHA_WASM_FACTICE));

        let report = StandaloneExporter::preflight_with_runtime_dir(&jeu, Some(dir.as_path()));
        assert!(report.ok, "pré-vol : {}", report.summary());
        assert!(
            !report.issues.iter().any(|i| i.code == "runtime-perime"),
            "aucun runtime-perime quand les versions concordent : {}",
            report.summary()
        );
        assert!(
            report.issues.iter().any(|i| i.code == "runtime-embarque"),
            "l'avertissement de taille reste présent : {}",
            report.summary()
        );
    }

    #[test]
    fn test_runtime_manifeste_version_differente_avertit_sans_bloquer() {
        let dir = repertoire_temporaire("manifeste_perime");
        ecrire_faux_runtime(&dir, "export function initSync() {}\n", b"\0asm\x01\0\0\0");
        let version_obsolete = format!("{}-perime", env!("CARGO_PKG_VERSION"));
        assert_ne!(
            version_obsolete,
            env!("CARGO_PKG_VERSION"),
            "la version factice doit différer de celle du moteur"
        );
        ecrire_manifeste_runtime(&dir, &version_obsolete, SHA_WASM_FACTICE);
        let jeu = jeu_reference();

        let report = StandaloneExporter::preflight_with_runtime_dir(&jeu, Some(dir.as_path()));
        assert!(report.ok, "un runtime périmé n'empêche pas l'export : {}", report.summary());
        assert_eq!(report.blocking_count(), 0, "rapport : {}", report.summary());

        let issue = report
            .issues
            .iter()
            .find(|i| i.code == "runtime-perime")
            .expect("runtime-perime attendu");
        assert_eq!(issue.severity, PreflightSeverity::Warning);
        assert!(
            issue
                .message
                .contains(&format!("construit pour la version {version_obsolete}")),
            "message : {}",
            issue.message
        );
        assert!(
            issue
                .message
                .contains(&format!("le moteur est en {}", env!("CARGO_PKG_VERSION"))),
            "message : {}",
            issue.message
        );
        assert!(
            issue.message.contains("scripts/build_wasm.sh"),
            "message : {}",
            issue.message
        );
    }

    #[test]
    fn test_runtime_manifeste_absent_reste_compatible() {
        let dir = repertoire_temporaire("manifeste_absent");
        ecrire_faux_runtime(&dir, "export function initSync() {}\n", b"\0asm\x01\0\0\0");
        let jeu = jeu_reference();

        let runtime = load_embedded_runtime(&dir).expect("runtime factice chargeable");
        assert_eq!(runtime.engine_version, None, "aucun manifeste : version inconnue");
        assert_eq!(runtime.wasm_sha256, None, "aucun manifeste : sha inconnu");

        let html = StandaloneExporter::export_html_with_runtime(
            &jeu,
            "Sans manifeste",
            "Testeur",
            Some(dir.as_path()),
        );
        assert!(
            !html.contains("<meta name=\"karacti-runtime\""),
            "aucune méta sans manifeste (le template lit simplement une balise absente)"
        );
        assert!(html.contains("id=\"runtime-glue\""), "le runtime reste embarqué");
        assert!(html.contains("id=\"embedded-package\""), "le paquet reste présent");

        let report = StandaloneExporter::preflight_with_runtime_dir(&jeu, Some(dir.as_path()));
        assert!(report.ok, "rapport : {}", report.summary());
        assert!(
            !report.issues.iter().any(|i| i.code == "runtime-perime"),
            "pas d'avertissement sans manifeste : {}",
            report.summary()
        );
    }

    #[test]
    fn test_export_runtime_html_contient_meta_quand_manifeste_existe() {
        let dir = repertoire_temporaire("meta_runtime");
        ecrire_faux_runtime(&dir, "export function initSync() {}\n", b"\0asm\x01\0\0\0");
        ecrire_manifeste_runtime(&dir, env!("CARGO_PKG_VERSION"), SHA_WASM_FACTICE);
        let jeu = jeu_reference();

        let html = StandaloneExporter::export_html_with_runtime(
            &jeu,
            "Meta runtime",
            "Testeur",
            Some(dir.as_path()),
        );
        let meta = format!(
            "  <meta name=\"karacti-runtime\" content=\"version={};wasm={SHA_WASM_FACTICE}\">\n",
            env!("CARGO_PKG_VERSION")
        );
        assert!(html.contains(&meta), "balise meta attendue, absente du HTML");

        let position = html.find("karacti-runtime").unwrap();
        let fin_head = html.find("</head>").unwrap();
        assert!(position < fin_head, "la méta doit être dans le <head>");
        assert!(
            position < html.find("id=\"embedded-package\"").unwrap(),
            "la méta précède les données du jeu"
        );

        // Même projet + mêmes artefacts + même manifeste = fichier identique.
        let second = StandaloneExporter::export_html_with_runtime(
            &jeu,
            "Meta runtime",
            "Testeur",
            Some(dir.as_path()),
        );
        assert_eq!(html, second, "la méta doit rester déterministe");
    }

    #[test]
    fn reexport_conserve_les_images_embarquees() {
        // Un jeu exporté puis ré-importé n'a plus ses images sur disque : un ré-export
        // doit les reprendre depuis la mémoire du paquet, sinon elles disparaissent.
        let mut game = jeu_reference();
        game.embedded_sprites
            .insert("sprite_importe".to_string(), vec![0x89, 0x50, 0x4E, 0x47]);
        game.embedded_sheets.insert(
            "atlas_importe".to_string(),
            (vec![0x89, 0x50, 0x4E, 0x47], "{}".to_string()),
        );

        let package = StandaloneExporter::create_package(&game, "Test", "Auteur");
        assert!(
            package.sprite_assets.contains_key("sprite_importe"),
            "le sprite embarqué doit être ré-exporté"
        );
        assert!(
            package.sheet_assets.contains_key("atlas_importe"),
            "l'atlas embarqué doit être ré-exporté"
        );
    }

    // -----------------------------------------------------------------------
    // Sources SVG embarquées (option A) : le runtime wasm n'a pas de disque.
    // -----------------------------------------------------------------------

    /// Identifiant volontairement absent du disque : seule la source embarquée
    /// du paquet peut le résoudre. C'est exactement la situation du wasm.
    const SVG_PREUVE: &str = "svg_preuve_embarquee_absente_du_disque";
    const SVG_PREUVE_SOURCE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect width="16" height="16" fill="#ff00ff"/></svg>"##;
    /// Couleur du rectangle de repli : elle ne doit pas apparaître quand le SVG
    /// est résolu (le test de rendu la distingue du magenta du sprite).
    const COULEUR_REPLI: u32 = 0x00123456;

    /// Charge une scène du dépôt indépendamment du répertoire courant des tests.
    fn scene_depuis_depot(rel: &str) -> SceneData {
        let chemin = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
        SceneData::load_from_file(&chemin)
            .unwrap_or_else(|e| panic!("scène {rel} illisible ({chemin}) : {e}"))
    }

    /// Projet de preuve SVG (hors dépôt à l'export) : niveau 1 = `level_01`
    /// (torches + caisse en `SvgRef`), niveau 2 = démo SVG (héros `hero`).
    fn jeu_preuve_svg() -> GameState {
        let niveau_1 = scene_depuis_depot("assets/scenes/level_01.scene.json");
        let niveau_2 = scene_depuis_depot("assets/scenes/svg_demo_01.scene.json");
        let mut jeu = GameState::from_scene_data(niveau_1.clone(), "preuve:niveau_1")
            .expect("état de preuve SVG");
        jeu.levels = vec![niveau_1, niveau_2];
        jeu
    }

    /// Scène minimale ne référençant qu'un SVG fourni par le paquet.
    fn scene_svg_preuve(nom: &str) -> SceneData {
        use crate::entity::{Entity, Renderable2D, Transform2D};

        let mut rendu = Renderable2D::svg_ref(SVG_PREUVE, 10);
        rendu.color = COULEUR_REPLI;
        SceneData {
            name: nom.to_string(),
            description: String::new(),
            objective: None,
            entities: vec![
                Entity::new(1, "Joueur SVG", "player", Transform2D::new(100.0, 100.0, 32.0, 32.0))
                    .with_renderable(rendu),
            ],
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        }
    }

    /// Paquet multi-niveaux dont le SVG n'existe que dans `svg_assets`.
    fn paquet_svg_embarque() -> KaractiGamePackage {
        let scenes = vec![
            scene_svg_preuve("NIVEAU SVG 1"),
            scene_svg_preuve("NIVEAU SVG 2"),
        ];
        let mut svg_assets = HashMap::new();
        svg_assets.insert(SVG_PREUVE.to_string(), SVG_PREUVE_SOURCE.to_string());
        KaractiGamePackage {
            metadata: PackageMetadata {
                title: "Preuve SVG".to_string(),
                ..PackageMetadata::default()
            },
            scene_name: scenes[0].name.clone(),
            scene_objective: None,
            entities: scenes[0].to_entities(),
            scenes,
            ui_screens: Vec::new(),
            kvl_graphs: HashMap::new(),
            initial_variables: HashMap::new(),
            sprite_assets: HashMap::new(),
            sheet_assets: HashMap::new(),
            svg_assets,
            svg_anim_assets: HashMap::new(),
        }
    }

    /// (a) Le paquet embarque les sources SVG de **toutes** les scènes.
    #[test]
    fn test_paquet_preuve_svg_embarque_les_sources_de_tous_les_niveaux() {
        let jeu = jeu_preuve_svg();
        let package = StandaloneExporter::create_package(&jeu, "Preuve SVG", "Testeur");

        assert_eq!(package.scenes.len(), 2, "les deux niveaux voyagent");
        for attendu in ["decor_torch", "decor_crate", "hero"] {
            let source = package
                .svg_assets
                .get(attendu)
                .unwrap_or_else(|| panic!("source SVG « {attendu} » absente du paquet"));
            assert!(
                source.contains("<svg"),
                "la source de « {attendu} » doit être du SVG brut"
            );
        }
        assert!(
            package.svg_assets.len() >= 3,
            "au moins les 3 assets SVG référencés, trouvés : {:?}",
            package.svg_assets.keys().collect::<Vec<_>>()
        );
    }

    /// (b) Import dans une instance neuve : `vector_sprite` résolu, y compris
    /// après `start_game` et `next_level` (reconstructions depuis la scène nue).
    #[test]
    fn test_import_paquet_svg_resout_et_survit_aux_reconstructions() {
        let package = StandaloneExporter::create_package(&jeu_preuve_svg(), "Preuve SVG", "Testeur");

        // Instance neuve : aucun SVG résolu au départ.
        let mut neuf = GameState::from_scene_data(
            scene_depuis_depot("assets/scenes/level_01.scene.json"),
            "test:neuf",
        )
        .expect("état neuf");

        StandaloneExporter::import_package(&mut neuf, &package);
        assert_eq!(neuf.levels.len(), 2);
        assert!(
            !neuf.embedded_svg_assets.is_empty(),
            "les sources SVG sont mémorisées pour les reconstructions"
        );
        assert!(
            neuf.entities.iter().any(|e| e.vector_sprite.is_some()),
            "le niveau 1 importé doit résoudre ses décors SVG"
        );

        neuf.start_game();
        assert!(
            neuf.entities
                .iter()
                .filter(|e| matches!(e.renderable.as_ref().map(|r| &r.shape), Some(ShapeKind::SvgRef { .. })))
                .all(|e| e.vector_sprite.is_some()),
            "start_game ne doit pas détruire les sprites SVG du niveau 1"
        );

        assert!(neuf.next_level(), "le niveau 2 est disponible");
        let heros = neuf
            .entities
            .iter()
            .find(|e| e.tag == "player")
            .expect("héros du niveau 2");
        let sprite = heros
            .vector_sprite
            .as_ref()
            .expect("next_level doit conserver le sprite SVG du héros");
        assert_eq!(sprite.asset, "hero");

        // `reload` repart aussi d'une scène nue : même exigence.
        neuf.reload();
        assert!(
            neuf.entities.iter().any(|e| e.vector_sprite.is_some()),
            "reload doit réinstaller les sources SVG du paquet"
        );
    }

    /// (b bis + d) Un SVG **absent du disque** mais embarqué est rastérisé :
    /// c'est la preuve du chemin wasm. Le rendu contient alors la couleur du
    /// sprite, jamais celle du rectangle de repli.
    #[test]
    fn test_svg_embarque_absent_du_disque_est_rendu_et_non_le_repli() {
        assert!(
            crate::vector::read_svg_source(SVG_PREUVE).is_none(),
            "le test n'a de sens que si l'asset est réellement absent du disque"
        );
        let package = paquet_svg_embarque();
        let mut jeu = GameState::from_scene_data(scene_svg_preuve("NEUF"), "test:neuf")
            .expect("état neuf");

        StandaloneExporter::import_package(&mut jeu, &package);
        let sprite = jeu.entities[0]
            .vector_sprite
            .as_ref()
            .expect("la source embarquée doit résoudre le sprite");
        assert_eq!(sprite.asset, SVG_PREUVE);
        assert!(sprite.image.width > 0 && sprite.image.height > 0);

        let compter = |jeu: &mut GameState, couleur: u32| {
            let mut canvas = crate::renderer::Canvas::new(1200, 800);
            jeu.render(&mut canvas);
            canvas.buffer.iter().filter(|&&p| p == couleur).count()
        };

        let magenta = 0x00FF00FF;
        assert!(
            compter(&mut jeu, magenta) > 100,
            "le sprite SVG (magenta) doit produire des pixels"
        );

        // Témoin : sans `vector_sprite`, le rendu retombe sur le rectangle teinté.
        jeu.entities[0].vector_sprite = None;
        assert_eq!(
            compter(&mut jeu, magenta),
            0,
            "sans sprite, aucune couleur SVG ne doit apparaître"
        );
    }

    /// (c) Ancien paquet sans `svg_assets` : désérialisable, inoffensif.
    #[test]
    fn test_ancien_paquet_sans_svg_assets_reste_importable() {
        let package = paquet_svg_embarque();
        let mut valeur = serde_json::to_value(&package).expect("paquet JSON");
        valeur
            .as_object_mut()
            .expect("objet JSON")
            .remove("svg_assets")
            .expect("champ svg_assets présent avant retrait");

        let ancien = KaractiGamePackage::from_json(&serde_json::to_string(&valeur).expect("JSON"))
            .expect("un ancien paquet sans svg_assets reste lisible");
        assert!(ancien.svg_assets.is_empty());

        let mut neuf = jeu_reference();
        StandaloneExporter::import_package(&mut neuf, &ancien);
        assert_eq!(neuf.levels.len(), 2, "les niveaux restent importés");
        assert!(
            neuf.embedded_svg_assets.is_empty(),
            "aucune source inventée pour un ancien paquet"
        );
        // Import valide : le rendu ne panique pas.
        neuf.start_game();
        let mut canvas = crate::renderer::Canvas::new(1200, 800);
        neuf.render(&mut canvas);
    }

    /// Le non-écrasement vaut aussi pour une scène qui porte déjà son sprite.
    #[test]
    fn test_scene_deja_resolue_conserve_son_vector_sprite() {
        use crate::entity::{Entity, Renderable2D, Transform2D};

        let mut entite = Entity::new(
            1,
            "Torche déjà résolue",
            "decor",
            Transform2D::new(100.0, 100.0, 32.0, 64.0),
        )
        .with_renderable(Renderable2D::svg_ref(SVG_PREUVE, 5));
        let image = std::sync::Arc::new(crate::vector::RasterImage {
            width: 1,
            height: 1,
            pixels: vec![0x00FF00FF],
        });
        entite.vector_sprite = Some(crate::entity::VectorSprite {
            asset: SVG_PREUVE.to_string(),
            image,
            rig: None,
            clip: "idle".to_string(),
            time: 0.0,
            speed: 1.0,
            playing: true,
        });
        let scene = SceneData {
            name: "Scène déjà résolue".to_string(),
            description: String::new(),
            objective: None,
            entities: vec![entite],
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        };

        // La source est introuvable (ni disque ni embarqué) : le sprite existant
        // doit survivre au lieu d'être remis à `None`.
        let etat = GameState::from_scene_data(scene, "test:deja_resolue")
            .expect("état sur scène déjà résolue");
        let sprite = etat.entities[0]
            .vector_sprite
            .as_ref()
            .expect("le vector_sprite de la scène ne doit pas être détruit");
        assert_eq!(sprite.asset, SVG_PREUVE);
    }

    // -----------------------------------------------------------------------
    // Clips `.anim.json` embarqués : l'auteur retrouve ses animations dans le
    // jeu exporté, y compris sous wasm où le disque est illisible.
    // -----------------------------------------------------------------------

    /// Sprite riggé minimal : quatre groupes nommés comme les membres attendus
    /// par `default_clips`, pour que l'absence de `.anim.json` produise bien
    /// `idle`/`walk` (et pas un rig sans clip).
    const SVG_RIG_ANIM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">
  <g id="arm_l"><rect x="0" y="0" width="8" height="8" fill="#ff0000"/></g>
  <g id="arm_r"><rect x="16" y="0" width="8" height="8" fill="#00ff00"/></g>
  <g id="leg_l"><rect x="32" y="0" width="8" height="8" fill="#0000ff"/></g>
  <g id="leg_r"><rect x="48" y="0" width="8" height="8" fill="#ffff00"/></g>
</svg>"##;

    /// Clips dont les noms et les durées diffèrent volontairement des clips
    /// générés : `idle` (1,0 s) et `walk` (0,8 s).
    const ANIM_RIG_ANIM: &str = r##"{
  "version": 1,
  "clips": {
    "danse": { "loop": true, "tracks": { "arm_l": [ { "t": 0.0, "rot": 0.0 }, { "t": 2.0, "rot": 90.0 } ] } },
    "saut": { "loop": false, "tracks": { "leg_l": [ { "t": 0.0, "rot": 0.0 }, { "t": 0.5, "rot": -40.0 } ] } }
  }
}"##;

    /// Écrit un SVG riggé (et, si demandé, son `.anim.json` voisin) dans un
    /// répertoire temporaire ; renvoie le chemin **absolu** du SVG.
    ///
    /// Aucun fichier n'est créé dans `assets/` : un identifiant absolu est
    /// résolu tel quel par `read_svg_source`/`read_anim_source`, ce qui permet
    /// de tester « fichier présent » puis « fichier absent » sur le même projet.
    fn ecrire_sprite_rigge(dir: &Path, nom: &str, anim: Option<&str>) -> String {
        let svg = dir.join(format!("{nom}.svg"));
        std::fs::write(&svg, SVG_RIG_ANIM).expect("SVG temporaire écrit");
        if let Some(anim) = anim {
            std::fs::write(dir.join(format!("{nom}.anim.json")), anim)
                .expect("anim.json temporaire écrit");
        }
        svg.to_string_lossy().into_owned()
    }

    /// Scène minimale à une entité `SvgRef` pointant un chemin absolu.
    fn scene_svg_absolu(nom: &str, asset: &str) -> SceneData {
        use crate::entity::{Entity, Renderable2D, Transform2D};

        let mut rendu = Renderable2D::svg_ref(asset, 10);
        rendu.color = COULEUR_REPLI;
        SceneData {
            name: nom.to_string(),
            description: String::new(),
            objective: None,
            entities: vec![
                Entity::new(
                    1,
                    "Joueur animé",
                    "player",
                    Transform2D::new(100.0, 100.0, 64.0, 64.0),
                )
                .with_renderable(rendu),
            ],
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        }
    }

    /// Projet à deux niveaux pointant tous deux le même sprite temporaire.
    fn jeu_clips_temp(asset: &str) -> GameState {
        let niveau_1 = scene_svg_absolu("NIVEAU ANIM 1", asset);
        let niveau_2 = scene_svg_absolu("NIVEAU ANIM 2", asset);
        let mut jeu = GameState::from_scene_data(niveau_1.clone(), "test:anim:niveau_1")
            .expect("état de preuve clips");
        jeu.levels = vec![niveau_1, niveau_2];
        jeu
    }

    /// Premier clip de ce nom visible sur un sprite vectoriel résolu.
    fn clip_du_sprite<'a>(
        jeu: &'a GameState,
        nom: &str,
    ) -> Option<&'a crate::vector::AnimClip> {
        jeu.entities
            .iter()
            .filter_map(|e| e.vector_sprite.as_ref())
            .filter_map(|s| s.rig.as_ref())
            .find_map(|rig| rig.clip(nom))
    }

    /// Vérifie que seuls les clips générés sont présents.
    fn assert_clips_par_defaut(jeu: &GameState) {
        assert!(
            clip_du_sprite(jeu, "idle").is_some(),
            "clip « idle » généré attendu"
        );
        assert!(
            clip_du_sprite(jeu, "walk").is_some(),
            "clip « walk » généré attendu"
        );
        assert!(
            clip_du_sprite(jeu, "danse").is_none(),
            "aucun clip du fichier ne doit apparaître"
        );
    }

    /// (a) Le `.anim.json` voisin d'un SVG du projet part dans le paquet, à
    /// l'octet près.
    #[test]
    fn test_paquet_embarque_le_anim_json_present_sur_disque() {
        let dossier = repertoire_temporaire("anim_present");
        let asset = ecrire_sprite_rigge(&dossier, "heros_rigge", Some(ANIM_RIG_ANIM));
        let jeu = jeu_clips_temp(&asset);

        let package = StandaloneExporter::create_package(&jeu, "Preuve clips", "Testeur");
        assert_eq!(
            package.svg_anim_assets.get(&asset).map(String::as_str),
            Some(ANIM_RIG_ANIM),
            "le contenu exact du `.anim.json` doit voyager"
        );
        assert!(
            package.svg_assets.contains_key(&asset),
            "la source SVG voyage aussi"
        );
    }

    /// Le projet de démonstration SVG embarque aussi le `.anim.json` réel de
    /// « hero », à l'octet près (fichier présent dans le dépôt, pas en temp).
    #[test]
    fn test_paquet_embarque_le_anim_json_reel_de_hero() {
        let package =
            StandaloneExporter::create_package(&jeu_preuve_svg(), "Preuve SVG", "Testeur");
        let attendu = crate::vector::read_anim_source("hero")
            .expect("assets/sprites/hero.anim.json doit exister dans le dépôt");
        assert!(attendu.contains("\"clips\""));
        assert_eq!(
            package.svg_anim_assets.get("hero").map(String::as_str),
            Some(attendu.as_str()),
            "le contenu du fichier réel doit être embarqué tel quel"
        );
    }

    /// (b) Import : `load_clips` renvoie les clips du fichier, pas les clips
    /// générés. Le SVG et son `.anim.json` sont supprimés après l'export :
    /// c'est le cas wasm, où seul le paquet peut encore fournir le sprite et
    /// son animation.
    #[test]
    fn test_import_utilise_les_clips_du_fichier_pas_les_defauts() {
        let dossier = repertoire_temporaire("anim_import");
        let asset = ecrire_sprite_rigge(&dossier, "heros_rigge", Some(ANIM_RIG_ANIM));
        let package =
            StandaloneExporter::create_package(&jeu_clips_temp(&asset), "Preuve clips", "Testeur");
        assert!(package.svg_anim_assets.contains_key(&asset));

        // Le `.anim.json` disparaît après l'export : l'état neuf ne peut plus
        // résoudre que les clips générés.
        std::fs::remove_file(dossier.join("heros_rigge.anim.json"))
            .expect("anim.json retiré après l'export");
        let mut neuf = GameState::from_scene_data(scene_svg_absolu("NEUF", &asset), "test:neuf")
            .expect("état neuf");
        assert_clips_par_defaut(&neuf);

        // Le SVG disparaît à son tour : c'est la situation wasm. Ni le SVG ni
        // son `.anim.json` ne sont lisibles sur disque ; seuls `svg_assets` et
        // `svg_anim_assets` peuvent encore les fournir.
        std::fs::remove_file(dossier.join("heros_rigge.svg"))
            .expect("SVG retiré après l'export");

        StandaloneExporter::import_package(&mut neuf, &package);
        let danse = clip_du_sprite(&neuf, "danse").expect("clip « danse » du fichier importé");
        assert_eq!(danse.duration, 2.0, "durée du fichier, pas celle d'un clip généré");
        assert!(danse.looped);
        let saut = clip_du_sprite(&neuf, "saut").expect("clip « saut » du fichier importé");
        assert_eq!(saut.duration, 0.5);
        assert!(!saut.looped);
        assert!(
            clip_du_sprite(&neuf, "walk").is_none(),
            "les clips générés ne doivent jamais gagner"
        );
        assert!(
            !neuf.embedded_svg_anims.is_empty(),
            "les clips sont mémorisés pour les reconstructions"
        );
    }

    /// (c) `start_game`, `next_level` et `reload` repartent d'une scène nue :
    /// les clips embarqués doivent être réinstallés à chaque fois.
    #[test]
    fn test_clips_importes_survivent_aux_reconstructions() {
        let dossier = repertoire_temporaire("anim_reconstructions");
        let asset = ecrire_sprite_rigge(&dossier, "heros_rigge", Some(ANIM_RIG_ANIM));
        let package =
            StandaloneExporter::create_package(&jeu_clips_temp(&asset), "Preuve clips", "Testeur");
        std::fs::remove_file(dossier.join("heros_rigge.anim.json"))
            .expect("anim.json retiré après l'export");
        std::fs::remove_file(dossier.join("heros_rigge.svg"))
            .expect("SVG retiré après l'export");

        let mut neuf = GameState::from_scene_data(scene_svg_absolu("NEUF", &asset), "test:neuf")
            .expect("état neuf");
        StandaloneExporter::import_package(&mut neuf, &package);

        neuf.start_game();
        assert_eq!(
            clip_du_sprite(&neuf, "danse").map(|c| c.duration),
            Some(2.0),
            "start_game réinstalle les clips"
        );

        assert!(neuf.next_level(), "le niveau 2 est disponible");
        assert_eq!(
            clip_du_sprite(&neuf, "danse").map(|c| c.duration),
            Some(2.0),
            "next_level réinstalle les clips"
        );

        neuf.reload();
        assert_eq!(
            clip_du_sprite(&neuf, "danse").map(|c| c.duration),
            Some(2.0),
            "reload réinstalle les clips"
        );
        assert!(clip_du_sprite(&neuf, "walk").is_none());
    }

    /// (d) Ancien paquet sans `svg_anim_assets` : lisible, inoffensif, clips
    /// par défaut, aucun panic au rendu.
    #[test]
    fn test_ancien_paquet_sans_svg_anim_assets_donne_les_clips_par_defaut() {
        let dossier = repertoire_temporaire("anim_ancien");
        let asset = ecrire_sprite_rigge(&dossier, "heros_rigge", Some(ANIM_RIG_ANIM));
        let package =
            StandaloneExporter::create_package(&jeu_clips_temp(&asset), "Preuve clips", "Testeur");
        std::fs::remove_file(dossier.join("heros_rigge.anim.json"))
            .expect("anim.json retiré après l'export");

        let mut valeur = serde_json::to_value(&package).expect("paquet JSON");
        valeur
            .as_object_mut()
            .expect("objet JSON")
            .remove("svg_anim_assets")
            .expect("champ svg_anim_assets présent avant retrait");
        let ancien = KaractiGamePackage::from_json(&serde_json::to_string(&valeur).expect("JSON"))
            .expect("un ancien paquet sans svg_anim_assets reste lisible");
        assert!(ancien.svg_anim_assets.is_empty());

        let mut neuf = GameState::from_scene_data(scene_svg_absolu("NEUF", &asset), "test:neuf")
            .expect("état neuf");
        StandaloneExporter::import_package(&mut neuf, &ancien);
        assert_clips_par_defaut(&neuf);
        assert!(
            neuf.embedded_svg_anims.is_empty(),
            "aucun clip inventé pour un ancien paquet"
        );

        neuf.start_game();
        let mut canvas = crate::renderer::Canvas::new(1200, 800);
        neuf.render(&mut canvas);
    }

    /// (e) `.anim.json` absent du disque **et** du paquet : clips par défaut.
    #[test]
    fn test_anim_absent_du_disque_et_du_paquet_donne_les_clips_par_defaut() {
        let dossier = repertoire_temporaire("anim_absent");
        let asset = ecrire_sprite_rigge(&dossier, "heros_sans_anim", None);
        assert!(
            crate::vector::read_anim_source(&asset).is_none(),
            "le fichier ne doit pas exister sur disque"
        );

        let jeu = jeu_clips_temp(&asset);
        assert_clips_par_defaut(&jeu);
        let package = StandaloneExporter::create_package(&jeu, "Sans clips", "Testeur");
        assert!(
            !package.svg_anim_assets.contains_key(&asset),
            "aucun clip ne doit être inventé à l'export"
        );

        let mut neuf = GameState::from_scene_data(scene_svg_absolu("NEUF", &asset), "test:neuf")
            .expect("état neuf");
        StandaloneExporter::import_package(&mut neuf, &package);
        assert_clips_par_defaut(&neuf);
    }

    /// Export → import → ré-export : les clips mémorisés par l'import sont
    /// ré-embarqués même quand leur fichier n'existe plus sur disque.
    #[test]
    fn reexport_conserve_les_clips_embarques() {
        let dossier = repertoire_temporaire("anim_reexport");
        let asset = ecrire_sprite_rigge(&dossier, "heros_rigge", Some(ANIM_RIG_ANIM));
        let package =
            StandaloneExporter::create_package(&jeu_clips_temp(&asset), "Preuve clips", "Testeur");
        std::fs::remove_file(dossier.join("heros_rigge.anim.json"))
            .expect("anim.json retiré après l'export");

        let mut neuf = GameState::from_scene_data(scene_svg_absolu("NEUF", &asset), "test:neuf")
            .expect("état neuf");
        StandaloneExporter::import_package(&mut neuf, &package);

        let reexport = StandaloneExporter::create_package(&neuf, "Preuve clips", "Testeur");
        assert_eq!(
            reexport.svg_anim_assets.get(&asset).map(String::as_str),
            Some(ANIM_RIG_ANIM),
            "le ré-export doit reprendre les clips de la mémoire du paquet"
        );
    }
}
