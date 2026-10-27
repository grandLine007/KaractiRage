//! Catalogue de types de nœuds KVL.
//!
//! Un [`NodeSchema`] est la **description unique** d'un type de nœud : il est consommé
//! par l'interpréteur (exécution), la palette native, l'éditeur Studio et le prompt IA.
//! Les instances de nœuds ([`crate::Node`]) portent l'état (position, pins, identifiant) ;
//! le schéma porte le contrat.
//!
//! Le registre résout aussi les types **préfixés** : `DamagePlayer:2` est résolu par le
//! schéma `DamagePlayer` avec l'argument `"2"`. Cela permet de décrire un paramètre
//! optionnel sans multiplier les identifiants.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::category::Category;
use crate::graph::Graph;
use crate::node::NodeKind;
use crate::types::Value;

/// Famille d'un nœud, alignée sur [`NodeKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeFlavor {
    Event,
    Condition,
    Action,
    Composite,
}

impl NodeFlavor {
    pub fn label(self) -> &'static str {
        match self {
            Self::Event => "Événement",
            Self::Condition => "Condition",
            Self::Action => "Action",
            Self::Composite => "Bloc composé",
        }
    }
}

/// Type d'un paramètre exposé par un nœud.
///
/// `Enum` porte les valeurs autorisées (libellés ou identifiants) pour que le Studio
/// puisse générer un sélecteur sans connaître le nœud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "values")]
pub enum ParamType {
    Int,
    Float,
    Bool,
    String,
    EntityRef,
    Enum(Vec<String>),
}

/// Description d'un paramètre de nœud (nom technique, libellé, type, défaut).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    pub label: String,
    pub param_type: ParamType,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub description: String,
}

impl ParamSpec {
    pub fn int(name: impl Into<String>, label: impl Into<String>, default: i64) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            param_type: ParamType::Int,
            default: Some(Value::Int(default)),
            description: String::new(),
        }
    }

    pub fn text(name: impl Into<String>, label: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            param_type: ParamType::String,
            default: Some(Value::String(default.into())),
            description: String::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

fn default_true() -> bool {
    true
}

fn default_flow_outputs() -> Vec<String> {
    vec!["out_trigger".to_string()]
}

/// Contrat d'un type de nœud.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeSchema {
    /// Identifiant du type, sans suffixe d'argument (`AddScore`, `DamagePlayer`).
    pub id: String,
    pub flavor: NodeFlavor,
    pub label: String,
    pub category: Category,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub params: Vec<ParamSpec>,
    /// Alias exacts résolus vers ce schéma (`ChasePlayer` → `NavChasePlayer`).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Préfixes acceptés en plus de l'identifiant exact (`DamagePlayer:`).
    #[serde(default)]
    pub prefixes: Vec<String>,
    /// Arguments proposés par défaut pour un type préfixé (`score:0` pour `SetVar:`).
    #[serde(default)]
    pub default_argument: String,
    /// Le nœud reçoit-il un flux d'entrée ?
    #[serde(default = "default_true")]
    pub flow_input: bool,
    /// Pins de sortie de flux (`out_trigger`, `out_true`, `out_false`).
    #[serde(default = "default_flow_outputs")]
    pub flow_outputs: Vec<String>,
    /// Plugin propriétaire, `None` pour le cœur.
    #[serde(default)]
    pub plugin: Option<String>,
}

impl NodeSchema {
    pub fn action(
        id: impl Into<String>,
        label: impl Into<String>,
        category: Category,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            flavor: NodeFlavor::Action,
            label: label.into(),
            category,
            description: description.into(),
            params: Vec::new(),
            aliases: Vec::new(),
            prefixes: Vec::new(),
            default_argument: String::new(),
            flow_input: true,
            flow_outputs: vec!["out_trigger".to_string()],
            plugin: None,
        }
    }

    pub fn event(
        id: impl Into<String>,
        label: impl Into<String>,
        category: Category,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            flavor: NodeFlavor::Event,
            label: label.into(),
            category,
            description: description.into(),
            params: Vec::new(),
            aliases: Vec::new(),
            prefixes: Vec::new(),
            default_argument: String::new(),
            flow_input: false,
            flow_outputs: vec!["out_trigger".to_string()],
            plugin: None,
        }
    }

    pub fn condition(
        id: impl Into<String>,
        label: impl Into<String>,
        category: Category,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            flavor: NodeFlavor::Condition,
            label: label.into(),
            category,
            description: description.into(),
            params: Vec::new(),
            aliases: Vec::new(),
            prefixes: Vec::new(),
            default_argument: String::new(),
            flow_input: true,
            flow_outputs: vec!["out_true".to_string(), "out_false".to_string()],
            plugin: None,
        }
    }

    pub fn with_param(mut self, param: ParamSpec) -> Self {
        self.params.push(param);
        self
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefixes.push(prefix.into());
        self
    }

    pub fn with_default_argument(mut self, argument: impl Into<String>) -> Self {
        self.default_argument = argument.into();
        self
    }

    pub fn with_plugin(mut self, plugin: impl Into<String>) -> Self {
        self.plugin = Some(plugin.into());
        self
    }

    /// Sorties de flux effectives (même si le JSON n'en déclare aucune).
    pub fn effective_flow_outputs(&self) -> Vec<String> {
        if self.flow_outputs.is_empty() {
            default_flow_outputs()
        } else {
            self.flow_outputs.clone()
        }
    }

    /// Vérifie la cohérence interne du schéma.
    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_id(&self.id)?;

        for alias in &self.aliases {
            validate_id(alias)?;
            if alias == &self.id {
                return Err(SchemaError::DuplicateAlias(alias.clone()));
            }
        }
        let mut seen_aliases = std::collections::BTreeSet::new();
        for alias in &self.aliases {
            if !seen_aliases.insert(alias.as_str()) {
                return Err(SchemaError::DuplicateAlias(alias.clone()));
            }
        }

        for prefix in &self.prefixes {
            let matches_base = std::iter::once(&self.id)
                .chain(self.aliases.iter())
                .any(|base| prefix.starts_with(&format!("{}:", base)));
            if !matches_base {
                return Err(SchemaError::InvalidPrefix(prefix.clone()));
            }
        }

        let mut seen_outputs = std::collections::BTreeSet::new();
        for output in &self.flow_outputs {
            if output.is_empty() {
                return Err(SchemaError::InvalidId(format!("{} (sortie vide)", self.id)));
            }
            if !seen_outputs.insert(output.as_str()) {
                return Err(SchemaError::DuplicateOutput(output.clone()));
            }
        }

        if !self.default_argument.is_empty() && self.prefixes.is_empty() {
            return Err(SchemaError::InvalidPrefix(format!(
                "{} (argument par défaut sans préfixe)",
                self.id
            )));
        }

        if self.flavor == NodeFlavor::Condition && self.flow_outputs.len() < 2 {
            return Err(SchemaError::FlavorMismatch(self.flavor));
        }

        Ok(())
    }
}

/// Nœud résolu par le registre, avec l'argument éventuel du type préfixé.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedNode<'schema, 'input> {
    pub schema: &'schema NodeSchema,
    /// Suffixe après le préfixe (`"2"` pour `DamagePlayer:2`), vide sinon.
    pub argument: &'input str,
}

/// Erreurs d'enregistrement d'un schéma.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchemaError {
    #[error("identifiant de nœud vide")]
    EmptyId,
    #[error("identifiant de nœud invalide : {0}")]
    InvalidId(String),
    #[error("nœud déjà enregistré : {0}")]
    DuplicateId(String),
    #[error("préfixe invalide : {0} (attendu : '<id>:...')")]
    InvalidPrefix(String),
    #[error("préfixe déjà enregistré : {0}")]
    DuplicatePrefix(String),
    #[error("alias déjà enregistré : {0}")]
    DuplicateAlias(String),
    #[error("sortie de flux dupliquée : {0}")]
    DuplicateOutput(String),
    #[error("flavor {0:?} incohérent avec le schéma")]
    FlavorMismatch(NodeFlavor),
}

/// Diagnostic de validation d'un graphe contre le registre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSchemaIssue {
    pub node_id: String,
    pub type_id: String,
    pub message: String,
}

/// Catalogue ordonné de schémas de nœuds.
///
/// `BTreeMap` garantit un ordre d'itération stable : le catalogue JSON du Studio et les
/// diagnostics sont reproductibles d'une exécution à l'autre.
#[derive(Debug, Clone, Default)]
pub struct NodeRegistry {
    schemas: BTreeMap<String, NodeSchema>,
    /// (alias exact, id du schéma canonique).
    aliases: BTreeMap<String, String>,
    /// (préfixe, id du schéma), triés du plus long au plus court.
    prefixes: Vec<(String, String)>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, schema: NodeSchema) -> Result<(), SchemaError> {
        schema.validate()?;

        if let Some(existing) = self.schemas.get(&schema.id) {
            return Err(SchemaError::DuplicateId(existing.id.clone()));
        }
        if self.aliases.contains_key(&schema.id) {
            return Err(SchemaError::DuplicateAlias(schema.id.clone()));
        }

        for alias in &schema.aliases {
            if self.schemas.contains_key(alias) || self.aliases.contains_key(alias) {
                return Err(SchemaError::DuplicateAlias(alias.clone()));
            }
        }
        for prefix in &schema.prefixes {
            if self.prefixes.iter().any(|(p, _)| p == prefix) {
                return Err(SchemaError::DuplicatePrefix(prefix.clone()));
            }
        }

        // Les préfixes sont insérés triés par longueur décroissante : le premier match
        // est le plus spécifique (`DamagePlayer:` avant `Damage:`).
        for alias in &schema.aliases {
            self.aliases.insert(alias.clone(), schema.id.clone());
        }
        for prefix in &schema.prefixes {
            self.prefixes.push((prefix.clone(), schema.id.clone()));
        }
        self.prefixes
            .sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));

        self.schemas.insert(schema.id.clone(), schema);
        Ok(())
    }

    pub fn register_all(&mut self, schemas: impl IntoIterator<Item = NodeSchema>) -> Result<(), SchemaError> {
        for schema in schemas {
            self.register(schema)?;
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&NodeSchema> {
        self.schemas.get(id)
    }

    /// Résout un type exact ou préfixé.
    pub fn resolve<'schema, 'input>(
        &'schema self,
        type_id: &'input str,
    ) -> Option<ResolvedNode<'schema, 'input>> {
        if let Some(schema) = self.schemas.get(type_id) {
            return Some(ResolvedNode {
                schema,
                argument: "",
            });
        }

        if let Some(schema_id) = self.aliases.get(type_id) {
            if let Some(schema) = self.schemas.get(schema_id) {
                return Some(ResolvedNode {
                    schema,
                    argument: "",
                });
            }
        }

        for (prefix, schema_id) in &self.prefixes {
            if let Some(argument) = type_id.strip_prefix(prefix) {
                if let Some(schema) = self.schemas.get(schema_id) {
                    return Some(ResolvedNode { schema, argument });
                }
            }
        }

        None
    }

    pub fn iter(&self) -> impl Iterator<Item = &NodeSchema> {
        self.schemas.values()
    }

    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    /// Valide tous les nœuds d'un graphe contre le registre.
    ///
    /// Ne modifie pas le graphe : l'appelant décide si une anomalie est bloquante.
    pub fn validate_graph(&self, graph: &Graph) -> Vec<GraphSchemaIssue> {
        let mut issues = Vec::new();

        for node in graph.nodes.values() {
            let (type_id, expected_flavor) = match &node.kind {
                NodeKind::Event { event_type } => (event_type.as_str(), NodeFlavor::Event),
                NodeKind::Condition { operator } => (operator.as_str(), NodeFlavor::Condition),
                NodeKind::Action { action_type } => (action_type.as_str(), NodeFlavor::Action),
                NodeKind::Composite { .. } => continue,
            };

            match self.resolve(type_id) {
                None => issues.push(GraphSchemaIssue {
                    node_id: node.id.clone(),
                    type_id: type_id.to_string(),
                    message: format!("Type de nœud inconnu du catalogue : '{}'", type_id),
                }),
                Some(resolved) if resolved.schema.flavor != expected_flavor => {
                    issues.push(GraphSchemaIssue {
                        node_id: node.id.clone(),
                        type_id: type_id.to_string(),
                        message: format!(
                            "'{}' est déclaré comme {} mais utilisé comme {}",
                            type_id,
                            resolved.schema.flavor.label(),
                            expected_flavor.label()
                        ),
                    });
                }
                Some(_) => {}
            }
        }

        issues
    }
}

fn validate_id(id: &str) -> Result<(), SchemaError> {
    if id.trim().is_empty() {
        return Err(SchemaError::EmptyId);
    }
    if id.contains(':') {
        return Err(SchemaError::InvalidId(format!(
            "{} (le ':' est réservé aux préfixes)",
            id
        )));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
    {
        return Err(SchemaError::InvalidId(id.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Node, Pin, PinDirection};

    fn damage_schema() -> NodeSchema {
        NodeSchema::action("DamagePlayer", "Infliger des dégâts", Category::Combat, "")
            .with_param(ParamSpec::int("amount", "Dégâts", 1))
            .with_prefix("DamagePlayer:")
    }

    #[test]
    fn resolve_exact_and_prefixed() {
        let mut registry = NodeRegistry::new();
        registry.register(damage_schema()).unwrap();

        let exact = registry.resolve("DamagePlayer").expect("exact");
        assert_eq!(exact.schema.id, "DamagePlayer");
        assert_eq!(exact.argument, "");

        let prefixed = registry.resolve("DamagePlayer:3").expect("prefixé");
        assert_eq!(prefixed.schema.id, "DamagePlayer");
        assert_eq!(prefixed.argument, "3");

        assert!(registry.resolve("DamagePlayer:3:4").is_some());
        assert!(registry.resolve("Damage").is_none());
    }

    #[test]
    fn aliases_resolve_to_the_canonical_schema() {
        let mut registry = NodeRegistry::new();
        let schema = NodeSchema::action("NavChasePlayer", "Traquer", Category::ArtificialIntelligence, "")
            .with_alias("ChasePlayer");
        registry.register(schema).unwrap();

        assert_eq!(registry.resolve("ChasePlayer").unwrap().schema.id, "NavChasePlayer");
        assert_eq!(registry.resolve("NavChasePlayer").unwrap().schema.id, "NavChasePlayer");
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let mut registry = NodeRegistry::new();
        registry.register(damage_schema()).unwrap();
        let err = registry.register(damage_schema()).unwrap_err();
        assert_eq!(err, SchemaError::DuplicateId("DamagePlayer".into()));
    }

    #[test]
    fn invalid_prefix_is_rejected() {
        let mut registry = NodeRegistry::new();
        let schema = NodeSchema::action("GiveItem", "Donner", Category::World, "")
            .with_prefix("Give");
        assert!(matches!(
            registry.register(schema),
            Err(SchemaError::InvalidPrefix(_))
        ));
    }

    #[test]
    fn condition_requires_two_outputs() {
        let mut schema = NodeSchema::condition("IsAlive", "Vivant", Category::Combat, "");
        schema.flow_outputs = vec!["out_true".into()];
        assert!(matches!(schema.validate(), Err(SchemaError::FlavorMismatch(_))));
    }

    #[test]
    fn graph_validation_reports_unknown_and_mismatched_types() {
        let mut registry = NodeRegistry::new();
        registry.register(damage_schema()).unwrap();
        registry
            .register(NodeSchema::condition(
                "IsPlayerAlive",
                "Vivant",
                Category::Combat,
                "",
            ))
            .unwrap();

        let mut graph = Graph::new("g", "g", "");
        graph.add_node(Node::new(
            "n1",
            "Dégâts",
            Category::Combat,
            "",
            NodeKind::Action {
                action_type: "DamagePlayer:2".into(),
            },
        ));
        graph.add_node(Node::new(
            "n2",
            "Inconnu",
            Category::World,
            "",
            NodeKind::Action {
                action_type: "ExplodeEverything".into(),
            },
        ));
        graph.add_node(Node::new(
            "n3",
            "Mauvais flavor",
            Category::Combat,
            "",
            NodeKind::Action {
                action_type: "IsPlayerAlive".into(),
            },
        ));

        let issues = registry.validate_graph(&graph);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|i| i.node_id == "n2"));
        assert!(issues.iter().any(|i| i.node_id == "n3"));
    }

    #[test]
    fn schema_serde_roundtrip_and_stable_order() {
        let schema = damage_schema().with_plugin("karacti.nodes.core");
        let json = serde_json::to_string(&schema).unwrap();
        let back: NodeSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema, back);

        let mut registry = NodeRegistry::new();
        registry
            .register(NodeSchema::action("Zeta", "Z", Category::World, ""))
            .unwrap();
        registry
            .register(NodeSchema::action("Alpha", "A", Category::World, ""))
            .unwrap();
        let ids: Vec<&str> = registry.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["Alpha", "Zeta"]);
    }

    #[test]
    fn prefixed_node_keeps_its_pins() {
        let mut registry = NodeRegistry::new();
        registry.register(damage_schema()).unwrap();

        let node = Node::new(
            "n",
            "Dégâts 3",
            Category::Combat,
            "",
            NodeKind::Action {
                action_type: "DamagePlayer:3".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let resolved = registry.resolve("DamagePlayer:3").unwrap();
        assert_eq!(resolved.argument, "3");
        assert_eq!(node.inputs.len(), 1);
    }
}
