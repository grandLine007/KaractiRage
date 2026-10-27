//! Système de plugins du moteur KaractiRage (socle v1).
//!
//! Trois notions :
//! - [`PluginManifest`] : le contrat déclaratif (`plugin.json`) ;
//! - [`PluginHost`] : chargement, validation, catalogue de nœuds et de handlers ;
//! - les packs de contenu et node packs décrits dans `docs/PLUGINS.md`.
//!
//! Aucun code tiers n'est chargé en v1 : les packs de contenu sont des données, les
//! node packs et extensions natives sont compilés dans le moteur.

pub mod host;
pub mod insert;
pub mod manifest;

pub use insert::{insert_schema_node, InsertError};
pub use host::{
    builtin_manifest, LoadReport, PluginError, PluginHost, PluginIssue, PluginIssueKind,
};
pub use manifest::{
    Capability, ContentManifest, ManifestError, PluginKind, PluginManifest, MAX_PLUGIN_BUDGET_US,
    PLUGIN_API_VERSION,
};
