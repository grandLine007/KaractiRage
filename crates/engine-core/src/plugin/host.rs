//! Hôte de plugins : chargement, validation et catalogue.
//!
//! L'hôte ne charge **aucun code** : il lit des manifestes, vérifie les chemins et les
//! graphes, agrège les schémas de nœuds et les handlers déjà compilés. C'est la partie
//! « données » du système ; les extensions natives s'enregistrent par le même chemin
//! (`NodePack` / `Native`) au démarrage.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use kvl_core::{Graph, NodeRegistry, NodeSchema};
use kvl_runtime::{
    builtin_action_handlers, builtin_node_schemas, HandlerRegistry, CORE_NODE_PACK,
};
use serde::Serialize;
use thiserror::Error;

use super::manifest::{
    Capability, ManifestError, PluginKind, PluginManifest, PLUGIN_API_VERSION,
};

/// Cause d'un diagnostic de chargement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginIssueKind {
    InvalidManifest,
    DuplicateId,
    MissingDependency,
    DependencyCycle,
    MissingContent,
    InvalidGraph,
    UnknownNodeType,
    Disabled,
}

/// Diagnostic non bloquant (ou bloquant pour un pack isolé) émis pendant le chargement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginIssue {
    pub plugin: String,
    pub kind: PluginIssueKind,
    pub message: String,
}

impl PluginIssue {
    fn new(plugin: impl Into<String>, kind: PluginIssueKind, message: impl Into<String>) -> Self {
        Self {
            plugin: plugin.into(),
            kind,
            message: message.into(),
        }
    }
}

/// Résultat d'un scan de dossier.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LoadReport {
    pub loaded: Vec<String>,
    pub issues: Vec<PluginIssue>,
}

impl LoadReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Erreurs bloquantes d'installation d'un pack.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("plugin.json absent dans {0}")]
    MissingManifest(PathBuf),
    #[error("lecture impossible : {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON invalide : {0}")]
    Json(#[from] serde_json::Error),
    #[error("manifeste invalide : {0}")]
    Manifest(#[from] ManifestError),
    #[error("identifiant déjà chargé : {0}")]
    DuplicateId(String),
    #[error("graphe invalide {path} : {message}")]
    InvalidGraph { path: PathBuf, message: String },
}

/// Manifeste du pack de nœuds cœur.
pub fn builtin_manifest() -> PluginManifest {
    let mut manifest = PluginManifest::new(
        CORE_NODE_PACK,
        "Nœuds cœur KaractiRage",
        "0.1.0",
        PluginKind::NodePack,
    )
    .with_description(
        "Pack de référence : actions AddScore, DamagePlayer et GiveItem, migrées du \
         dispatch historique vers le registre de handlers.",
    )
    .with_capability(Capability::ContentRead);
    manifest.deterministic = true;
    manifest
}

/// Hôte de plugins d'une instance de moteur.
pub struct PluginHost {
    manifests: BTreeMap<String, PluginManifest>,
    roots: BTreeMap<String, PathBuf>,
    content_files: BTreeMap<String, Vec<PathBuf>>,
    nodes: NodeRegistry,
    handlers: HandlerRegistry,
    issues: Vec<PluginIssue>,
    load_order: Vec<String>,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    /// Hôte vide (aucun nœud, aucun handler) : utile aux tests et aux outils.
    pub fn new() -> Self {
        Self {
            manifests: BTreeMap::new(),
            roots: BTreeMap::new(),
            content_files: BTreeMap::new(),
            nodes: NodeRegistry::new(),
            handlers: HandlerRegistry::new(),
            issues: Vec::new(),
            load_order: Vec::new(),
        }
    }

    /// Hôte prêt à l'emploi : pack cœur (schémas + handlers) installé.
    pub fn with_builtins() -> Self {
        let mut host = Self {
            handlers: builtin_action_handlers(),
            ..Self::new()
        };
        host.install_builtin_core();
        host
    }

    fn install_builtin_core(&mut self) {
        let manifest = builtin_manifest();
        let id = manifest.id.clone();

        for schema in builtin_node_schemas() {
            if let Err(error) = self.nodes.register(schema) {
                self.issues.push(PluginIssue::new(
                    &id,
                    PluginIssueKind::InvalidManifest,
                    format!("schéma cœur refusé : {}", error),
                ));
            }
        }

        self.manifests.insert(id.clone(), manifest);
        self.roots.insert(id.clone(), PathBuf::from("<builtin>"));
        self.content_files.insert(id.clone(), Vec::new());
        self.load_order.push(id);
    }

    /// Charge un pack isolé (strict : la première erreur est bloquante).
    ///
    /// Retourne l'identifiant du plugin installé.
    pub fn install_pack(&mut self, dir: impl AsRef<Path>) -> Result<String, PluginError> {
        let dir = dir.as_ref();
        let manifest_path = dir.join("plugin.json");
        if !manifest_path.is_file() {
            return Err(PluginError::MissingManifest(dir.to_path_buf()));
        }

        let raw = fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = serde_json::from_str(&raw)?;
        manifest.validate()?;

        if self.manifests.contains_key(&manifest.id) {
            return Err(PluginError::DuplicateId(manifest.id));
        }

        if !manifest.enabled {
            self.issues.push(PluginIssue::new(
                &manifest.id,
                PluginIssueKind::Disabled,
                "plugin désactivé dans son manifeste",
            ));
            return Ok(manifest.id);
        }

        let mut installed_files = Vec::new();
        for relative in manifest.content.all_paths() {
            let full = dir.join(relative);
            if !full.is_file() {
                self.push_issue_unique(PluginIssue::new(
                    &manifest.id,
                    PluginIssueKind::MissingContent,
                    format!("fichier de contenu introuvable : {}", relative),
                ));
                continue;
            }
            installed_files.push(full);
        }

        // Les graphes sont la seule partie du contenu interprétable : ils sont parsés
        // et validés contre le catalogue de nœuds dès le chargement.
        for relative in &manifest.content.graphs {
            let full = dir.join(relative);
            if !full.is_file() {
                continue;
            }
            let raw = fs::read_to_string(&full)?;
            let graph = Graph::from_json(&raw).map_err(|error| PluginError::InvalidGraph {
                path: full.clone(),
                message: error.to_string(),
            })?;

            for issue in self.nodes.validate_graph(&graph) {
                self.push_issue_unique(PluginIssue::new(
                    &manifest.id,
                    PluginIssueKind::UnknownNodeType,
                    format!(
                        "{} (nœud '{}' du graphe '{}')",
                        issue.message, issue.node_id, graph.id
                    ),
                ));
            }
        }

        let id = manifest.id.clone();
        self.manifests.insert(id.clone(), manifest);
        self.roots.insert(id.clone(), dir.to_path_buf());
        self.content_files.insert(id.clone(), installed_files);
        Ok(id)
    }

    /// Scanne un dossier de packs (`plugin.json` à la racine ou un sous-dossier par pack).
    ///
    /// Un pack invalide n'interrompt pas les autres : il produit un diagnostic.
    pub fn load_dir(&mut self, root: impl AsRef<Path>) -> LoadReport {
        let root = root.as_ref();
        let mut report = LoadReport::default();
        if !root.is_dir() {
            return report;
        }

        let mut candidates: Vec<PathBuf> = Vec::new();
        if root.join("plugin.json").is_file() {
            candidates.push(root.to_path_buf());
        } else if let Ok(entries) = fs::read_dir(root) {
            let mut dirs: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir() && path.join("plugin.json").is_file())
                .collect();
            dirs.sort();
            candidates = dirs;
        }

        for dir in candidates {
            match self.install_pack(&dir) {
                Ok(id) => report.loaded.push(id),
                Err(error) => {
                    let issue = PluginIssue::new(
                        dir.file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| dir.display().to_string()),
                        issue_kind_for(&error),
                        error.to_string(),
                    );
                    self.push_issue_unique(issue.clone());
                    report.issues.push(issue);
                }
            }
        }

        self.finalize();
        report.issues.extend(self.issues.iter().cloned());
        report
    }

    /// Cherche `plugins/` puis `../../plugins/` depuis le répertoire courant.
    ///
    /// Utilisé par le moteur : le binaire tourne à la racine du dépôt, les tests de
    /// `engine-core` depuis `crates/engine-core`.
    pub fn load_workspace(&mut self) -> LoadReport {
        for candidate in ["plugins", "../../plugins"] {
            let path = Path::new(candidate);
            if path.is_dir() {
                let report = self.load_dir(path);
                if !report.loaded.is_empty() {
                    return report;
                }
            }
        }
        LoadReport::default()
    }

    /// Vérifie les dépendances, détecte les cycles et fixe l'ordre d'application.
    fn finalize(&mut self) {
        let ids: Vec<String> = self.manifests.keys().cloned().collect();

        for id in &ids {
            let dependencies = match self.manifests.get(id) {
                Some(manifest) => manifest.dependencies.clone(),
                None => continue,
            };
            for dependency in dependencies {
                if !self.manifests.contains_key(&dependency) {
                    self.push_issue_unique(PluginIssue::new(
                        id,
                        PluginIssueKind::MissingDependency,
                        format!("dépendance introuvable : {}", dependency),
                    ));
                }
            }
        }

        let manifests = self.manifests.clone();
        let mut marks: BTreeMap<String, u8> = BTreeMap::new();
        let mut order = Vec::new();
        let mut cycles = Vec::new();
        for id in manifests.keys() {
            visit(id, &manifests, &mut marks, &mut order, &mut cycles);
        }
        for cycle in cycles {
            self.push_issue_unique(PluginIssue::new(
                &cycle,
                PluginIssueKind::DependencyCycle,
                "cycle de dépendances détecté",
            ));
        }
        self.load_order = order;
    }

    fn push_issue_unique(&mut self, issue: PluginIssue) {
        if !self.issues.iter().any(|existing| existing == &issue) {
            self.issues.push(issue);
        }
    }

    pub fn manifests(&self) -> impl Iterator<Item = &PluginManifest> {
        self.manifests.values()
    }

    pub fn manifest(&self, id: &str) -> Option<&PluginManifest> {
        self.manifests.get(id)
    }

    pub fn root(&self, id: &str) -> Option<&Path> {
        self.roots.get(id).map(PathBuf::as_path)
    }

    pub fn content_files(&self, id: &str) -> Option<&[PathBuf]> {
        self.content_files.get(id).map(Vec::as_slice)
    }

    pub fn nodes(&self) -> &NodeRegistry {
        &self.nodes
    }

    pub fn handlers(&self) -> &HandlerRegistry {
        &self.handlers
    }

    pub fn issues(&self) -> &[PluginIssue] {
        &self.issues
    }

    pub fn load_order(&self) -> &[String] {
        &self.load_order
    }

    pub fn is_loaded(&self, id: &str) -> bool {
        self.manifests.contains_key(id)
    }

    /// Prépare l'insertion d'un nœud : renvoie le schéma et le **type concret** à
    /// écrire dans le graphe.
    ///
    /// Un schéma préfixé sans handler exact (`SetVar`) est matérialisé avec ses
    /// paramètres par défaut (`SetVar:flag:true`) pour que le nœud soit exécutable
    /// immédiatement après un clic dans la palette.
    pub fn instantiate(&self, type_id: &str) -> Option<(NodeSchema, String)> {
        let resolved = self.nodes.resolve(type_id)?;
        let schema = resolved.schema.clone();
        let prefix = schema.prefixes.first().cloned();

        // Un schéma paramétré est matérialisé avec ses valeurs par défaut, même si un
        // handler exact existe : c'est la forme la plus explicite pour l'éditeur.
        if let Some(prefix) = prefix.clone() {
            if !schema.default_argument.is_empty() {
                return Some((
                    schema.clone(),
                    format!("{}{}", prefix, schema.default_argument),
                ));
            }
            if !schema.params.is_empty() {
                let arguments: Vec<String> = schema
                    .params
                    .iter()
                    .map(|param| param_argument(&param.default))
                    .collect();
                return Some((schema, format!("{}{}", prefix, arguments.join(":"))));
            }
        }

        if self.handlers.is_registered(type_id) {
            return Some((schema, type_id.to_string()));
        }

        if let Some(prefix) = prefix {
            return Some((schema, prefix));
        }

        Some((schema, type_id.to_string()))
    }

    /// Catalogue consommable par le Studio (plugins, nœuds, ordre, diagnostics).
    pub fn catalog_json(&self) -> Result<String, serde_json::Error> {
        #[derive(Serialize)]
        struct Catalog<'a> {
            api_version: u32,
            plugin_count: usize,
            node_count: usize,
            load_order: &'a [String],
            plugins: Vec<&'a PluginManifest>,
            nodes: Vec<&'a NodeSchema>,
            issues: &'a [PluginIssue],
        }

        let catalog = Catalog {
            api_version: PLUGIN_API_VERSION,
            plugin_count: self.manifests.len(),
            node_count: self.nodes.len(),
            load_order: &self.load_order,
            plugins: self.manifests.values().collect(),
            nodes: self.nodes.iter().collect(),
            issues: &self.issues,
        };
        serde_json::to_string_pretty(&catalog)
    }
}

fn param_argument(value: &Option<kvl_core::Value>) -> String {
    match value {
        Some(kvl_core::Value::Bool(flag)) => flag.to_string(),
        Some(kvl_core::Value::Int(number)) => number.to_string(),
        Some(kvl_core::Value::Float(number)) => number.to_string(),
        Some(kvl_core::Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

fn issue_kind_for(error: &PluginError) -> PluginIssueKind {
    match error {
        PluginError::MissingManifest(_) => PluginIssueKind::InvalidManifest,
        PluginError::Io(_) | PluginError::Json(_) => PluginIssueKind::InvalidManifest,
        PluginError::Manifest(_) => PluginIssueKind::InvalidManifest,
        PluginError::DuplicateId(_) => PluginIssueKind::DuplicateId,
        PluginError::InvalidGraph { .. } => PluginIssueKind::InvalidGraph,
    }
}

fn visit(
    id: &str,
    manifests: &BTreeMap<String, PluginManifest>,
    marks: &mut BTreeMap<String, u8>,
    order: &mut Vec<String>,
    cycles: &mut Vec<String>,
) {
    match marks.get(id).copied().unwrap_or(0) {
        2 => return,
        1 => {
            cycles.push(id.to_string());
            return;
        }
        _ => {}
    }

    marks.insert(id.to_string(), 1);
    if let Some(manifest) = manifests.get(id) {
        for dependency in &manifest.dependencies {
            if manifests.contains_key(dependency) {
                visit(dependency, manifests, marks, order, cycles);
            }
        }
    }
    marks.insert(id.to_string(), 2);
    order.push(id.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvl_core::{Category, Node, NodeKind};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempPack {
        root: PathBuf,
    }

    impl TempPack {
        fn new(name: &str) -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "karacti-plugin-{}-{}-{}",
                std::process::id(),
                name,
                unique
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("création du dossier temporaire");
            Self { root }
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("dossier parent");
            }
            fs::write(path, content).expect("écriture du fichier de test");
        }

        fn write_manifest(&self, manifest: &PluginManifest) {
            self.write(
                "plugin.json",
                &serde_json::to_string_pretty(manifest).expect("manifeste JSON"),
            );
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TempPack {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn content_pack(id: &str, graphs: &[&str]) -> PluginManifest {
        let mut manifest =
            PluginManifest::new(id, "Pack de test", "1.0.0", PluginKind::Content)
                .with_capability(Capability::ContentRead);
        manifest.content.graphs = graphs.iter().map(|g| g.to_string()).collect();
        manifest
    }

    fn sample_graph() -> String {
        let mut graph = Graph::new("g_test", "Test", "graphe de test");
        graph.add_variable("score", kvl_core::Value::Int(0));
        graph.add_node(Node::new(
            "a1",
            "Score",
            Category::Progression,
            "",
            NodeKind::Action {
                action_type: "AddScore".into(),
            },
        ));
        graph.to_json().expect("graphe JSON")
    }

    #[test]
    fn builtin_host_exposes_core_nodes_and_handlers() {
        let host = PluginHost::with_builtins();
        assert!(host.is_loaded(CORE_NODE_PACK));
        assert!(host.nodes().get("AddScore").is_some());
        assert!(host.nodes().get("OnPlayerOverlap").is_some());
        assert!(host.nodes().len() >= 3);
        let resolved = host.nodes().resolve("DamagePlayer:2").expect("nœud résolu");
        assert_eq!(resolved.schema.id, "DamagePlayer");
        assert_eq!(resolved.argument, "2");
        assert!(host.handlers().is_registered("AddScore"));
        assert!(host.load_order().contains(&CORE_NODE_PACK.to_string()));
    }

    #[test]
    fn installs_content_pack_and_validates_its_graph() {
        let pack = TempPack::new("valid");
        pack.write_manifest(&content_pack("karacti.test.valid", &["graphs/a.kvl.json"]));
        pack.write("graphs/a.kvl.json", &sample_graph());

        let mut host = PluginHost::with_builtins();
        let id = host.install_pack(pack.path()).expect("pack installé");
        assert_eq!(id, "karacti.test.valid");
        assert_eq!(host.content_files(&id).expect("fichiers").len(), 1);
        assert!(host.issues().is_empty(), "diagnostics : {:?}", host.issues());
    }

    #[test]
    fn unknown_node_type_is_reported_but_pack_loads() {
        let pack = TempPack::new("unknown-node");
        pack.write_manifest(&content_pack("karacti.test.unknown", &["graphs/a.kvl.json"]));
        let mut graph = Graph::new("g", "G", "");
        graph.add_node(Node::new(
            "a1",
            "Fusée",
            Category::World,
            "",
            NodeKind::Action {
                action_type: "LaunchRocket".into(),
            },
        ));
        pack.write("graphs/a.kvl.json", &graph.to_json().unwrap());

        let mut host = PluginHost::with_builtins();
        host.install_pack(pack.path()).expect("pack installé");
        assert!(host
            .issues()
            .iter()
            .any(|issue| issue.kind == PluginIssueKind::UnknownNodeType));
    }

    #[test]
    fn missing_content_file_is_a_warning() {
        let pack = TempPack::new("missing");
        pack.write_manifest(&content_pack("karacti.test.missing", &["graphs/absent.json"]));

        let mut host = PluginHost::with_builtins();
        host.install_pack(pack.path()).expect("pack installé");
        assert!(host
            .issues()
            .iter()
            .any(|issue| issue.kind == PluginIssueKind::MissingContent));
    }

    #[test]
    fn duplicate_pack_id_is_rejected() {
        let pack = TempPack::new("duplicate");
        pack.write_manifest(&content_pack("karacti.test.duplicate", &[]));

        let mut host = PluginHost::with_builtins();
        host.install_pack(pack.path()).expect("première installation");
        let error = host.install_pack(pack.path()).expect_err("doublon refusé");
        assert!(matches!(error, PluginError::DuplicateId(_)));
    }

    #[test]
    fn unsafe_content_path_is_rejected_at_manifest_validation() {
        let pack = TempPack::new("unsafe");
        let mut manifest = content_pack("karacti.test.unsafe", &[]);
        manifest.content.scenes.push("../secret.json".into());
        pack.write_manifest(&manifest);

        let mut host = PluginHost::with_builtins();
        let error = host.install_pack(pack.path()).expect_err("chemin refusé");
        assert!(matches!(error, PluginError::Manifest(_)));
    }

    #[test]
    fn disabled_pack_is_skipped() {
        let pack = TempPack::new("disabled");
        let mut manifest = content_pack("karacti.test.disabled", &[]);
        manifest.enabled = false;
        pack.write_manifest(&manifest);

        let mut host = PluginHost::with_builtins();
        let id = host.install_pack(pack.path()).expect("pack ignoré proprement");
        assert_eq!(id, "karacti.test.disabled");
        assert!(!host.is_loaded(&id));
        assert!(host
            .issues()
            .iter()
            .any(|issue| issue.kind == PluginIssueKind::Disabled));
    }

    #[test]
    fn load_dir_reports_dependency_and_cycle() {
        let root = TempPack::new("deps");
        let mut a = content_pack("karacti.test.a", &[]);
        a.dependencies.push("karacti.test.b".into());
        let mut b = content_pack("karacti.test.b", &[]);
        b.dependencies.push("karacti.test.a".into());

        fs::create_dir_all(root.path().join("a")).unwrap();
        fs::create_dir_all(root.path().join("b")).unwrap();
        fs::write(
            root.path().join("a/plugin.json"),
            serde_json::to_string(&a).unwrap(),
        )
        .unwrap();
        fs::write(
            root.path().join("b/plugin.json"),
            serde_json::to_string(&b).unwrap(),
        )
        .unwrap();

        let mut host = PluginHost::with_builtins();
        let report = host.load_dir(root.path());
        assert_eq!(report.loaded.len(), 2);
        assert!(host
            .issues()
            .iter()
            .any(|issue| issue.kind == PluginIssueKind::DependencyCycle));

        let mut missing = content_pack("karacti.test.c", &[]);
        missing.dependencies.push("karacti.test.absent".into());
        fs::create_dir_all(root.path().join("c")).unwrap();
        fs::write(
            root.path().join("c/plugin.json"),
            serde_json::to_string(&missing).unwrap(),
        )
        .unwrap();
        host.load_dir(root.path());
        assert!(host
            .issues()
            .iter()
            .any(|issue| issue.kind == PluginIssueKind::MissingDependency));
    }

    #[test]
    fn reference_pack_loads_from_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        if !root.is_dir() {
            eprintln!("pack de référence absent, test ignoré : {}", root.display());
            return;
        }

        let mut host = PluginHost::with_builtins();
        let report = host.load_dir(&root);
        assert!(
            report.loaded.iter().any(|id| id == "karacti.pack.platformer"),
            "plugins chargés : {:?}, diagnostics : {:?}",
            report.loaded,
            report.issues
        );
        assert!(host.issues().is_empty(), "diagnostics : {:?}", host.issues());
        assert!(host
            .manifests()
            .any(|manifest| manifest.id == "karacti.pack.platformer"));
    }

    #[test]
    fn instantiate_materialises_prefixed_schemas() {
        let host = PluginHost::with_builtins();

        let (schema, concrete) = host.instantiate("DamagePlayer").expect("exact");
        assert_eq!(schema.id, "DamagePlayer");
        assert_eq!(concrete, "DamagePlayer:1");

        let (schema, concrete) = host.instantiate("SetVar").expect("préfixé");
        assert_eq!(schema.id, "SetVar");
        assert_eq!(concrete, "SetVar:score:0");

        let (schema, concrete) = host.instantiate("PlaySound").expect("paramétré");
        assert_eq!(schema.id, "PlaySound");
        assert_eq!(concrete, "PlaySound:coin");

        let (_, concrete) = host.instantiate("OnPlayerOverlap").expect("événement");
        assert_eq!(concrete, "OnPlayerOverlap");

        assert!(host.instantiate("TypeInconnu").is_none());
    }

    #[test]
    fn catalog_json_is_stable_and_complete() {
        let host = PluginHost::with_builtins();
        let json = host.catalog_json().expect("catalogue");
        assert!(json.contains("\"api_version\": 1"));
        assert!(json.contains(CORE_NODE_PACK));
        assert!(json.contains("AddScore"));
        assert!(json.contains("\"load_order\""));
    }
}
