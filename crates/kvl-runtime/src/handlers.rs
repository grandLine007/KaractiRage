//! Registre d'exécution des nœuds KVL.
//!
//! Le registre associe un type de nœud (`AddScore`, `DamagePlayer`) à un pointeur de
//! fonction. Il est consulté **avant** le `match` historique de l'interpréteur : une
//! action migrée est exécutée par le registre, une action non migrée continue de passer
//! par le repli. Cette stratification permet de migrer le catalogue progressivement,
//! sans rupture de comportement.
//!
//! Aucune allocation n'est faite par le registre au moment du dispatch : les tables sont
//! construites une fois au chargement.

use std::collections::HashMap;

use thiserror::Error;

use crate::context::ExecutionContext;
use crate::interpreter::RuntimeError;

/// Contexte d'un nœud en cours d'exécution, passé au handler.
#[derive(Debug, Clone, Copy)]
pub struct NodeCall<'a> {
    pub node_id: &'a str,
    pub node_name: &'a str,
    /// Type complet tel qu'il apparaît dans le graphe (`DamagePlayer:2`).
    pub type_id: &'a str,
    /// Suffixe après le préfixe (`2`), vide pour un type exact.
    pub argument: &'a str,
}

impl<'a> NodeCall<'a> {
    pub fn new(
        node_id: &'a str,
        node_name: &'a str,
        type_id: &'a str,
        argument: &'a str,
    ) -> Self {
        Self {
            node_id,
            node_name,
            type_id,
            argument,
        }
    }
}

/// Signature d'un handler d'action.
pub type ActionHandler = fn(&mut ExecutionContext, &NodeCall<'_>) -> Result<(), RuntimeError>;

/// Erreurs d'enregistrement dans le registre.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HandlerError {
    #[error("action déjà enregistrée : {0}")]
    DuplicateAction(String),
    #[error("préfixe invalide : {0} (doit finir par ':')")]
    InvalidPrefix(String),
    #[error("préfixe déjà enregistré : {0}")]
    DuplicatePrefix(String),
}

/// Table de handlers d'actions.
#[derive(Default, Clone)]
pub struct HandlerRegistry {
    exact: HashMap<String, ActionHandler>,
    /// (préfixe, handler), triés du plus long au plus court.
    prefixes: Vec<(String, ActionHandler)>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre un handler pour un type exact.
    pub fn register(
        &mut self,
        type_id: impl Into<String>,
        handler: ActionHandler,
    ) -> Result<(), HandlerError> {
        let type_id = type_id.into();
        if self.exact.contains_key(&type_id) {
            return Err(HandlerError::DuplicateAction(type_id));
        }
        self.exact.insert(type_id, handler);
        Ok(())
    }

    /// Remplace un handler exact existant (surcharge explicite par un plugin).
    ///
    /// À utiliser avec parcimonie : une surcharge change le comportement de tous les
    /// graphes existants. Les préfixes ne sont pas touchés.
    pub fn replace(&mut self, type_id: impl Into<String>, handler: ActionHandler) {
        self.exact.insert(type_id.into(), handler);
    }

    /// Enregistre un handler pour tous les types commençant par `prefix` (`DamagePlayer:`).
    pub fn register_prefix(
        &mut self,
        prefix: impl Into<String>,
        handler: ActionHandler,
    ) -> Result<(), HandlerError> {
        let prefix = prefix.into();
        if !prefix.ends_with(':') {
            return Err(HandlerError::InvalidPrefix(prefix));
        }
        if self.prefixes.iter().any(|(p, _)| p == &prefix) {
            return Err(HandlerError::DuplicatePrefix(prefix));
        }
        self.prefixes.push((prefix, handler));
        self.prefixes
            .sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        Ok(())
    }

    /// Exécute le handler correspondant, s'il existe.
    ///
    /// `None` signifie « non géré par le registre » : l'appelant applique son repli.
    pub fn dispatch(
        &self,
        ctx: &mut ExecutionContext,
        node_id: &str,
        node_name: &str,
        type_id: &str,
    ) -> Option<Result<(), RuntimeError>> {
        if let Some(handler) = self.exact.get(type_id) {
            let call = NodeCall::new(node_id, node_name, type_id, "");
            return Some(handler(ctx, &call));
        }

        for (prefix, handler) in &self.prefixes {
            if let Some(argument) = type_id.strip_prefix(prefix) {
                let call = NodeCall::new(node_id, node_name, type_id, argument);
                return Some(handler(ctx, &call));
            }
        }

        None
    }

    pub fn is_registered(&self, type_id: &str) -> bool {
        if self.exact.contains_key(type_id) {
            return true;
        }
        self.prefixes
            .iter()
            .any(|(prefix, _)| type_id.starts_with(prefix))
    }

    /// Types exacts enregistrés, triés (diagnostic et tests).
    pub fn action_types(&self) -> Vec<&str> {
        let mut types: Vec<&str> = self.exact.keys().map(String::as_str).collect();
        types.sort_unstable();
        types
    }

    pub fn prefix_count(&self) -> usize {
        self.prefixes.len()
    }

    pub fn len(&self) -> usize {
        self.exact.len() + self.prefixes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.exact.is_empty() && self.prefixes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvl_core::Value;

    fn set_flag(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
        ctx.set_var("flag", Value::String(format!("{}:{}", call.type_id, call.argument)));
        Ok(())
    }

    #[test]
    fn dispatch_exact_then_prefix() {
        let mut registry = HandlerRegistry::new();
        registry.register("Exact", set_flag).unwrap();
        registry.register_prefix("Prefix:", set_flag).unwrap();

        let mut ctx = ExecutionContext::new();
        assert!(registry.dispatch(&mut ctx, "n", "N", "Exact").is_some());
        assert_eq!(
            ctx.get_var("flag"),
            Some(&Value::String("Exact:".into()))
        );

        assert!(registry.dispatch(&mut ctx, "n", "N", "Prefix:42").is_some());
        assert_eq!(
            ctx.get_var("flag"),
            Some(&Value::String("Prefix:42:42".into()))
        );

        assert!(registry.dispatch(&mut ctx, "n", "N", "Unknown").is_none());
    }

    #[test]
    fn duplicate_and_invalid_prefix_are_rejected() {
        let mut registry = HandlerRegistry::new();
        registry.register("Exact", set_flag).unwrap();
        assert_eq!(
            registry.register("Exact", set_flag).unwrap_err(),
            HandlerError::DuplicateAction("Exact".into())
        );
        assert_eq!(
            registry.register_prefix("Prefix", set_flag).unwrap_err(),
            HandlerError::InvalidPrefix("Prefix".into())
        );
    }

    #[test]
    fn longest_prefix_wins() {
        let mut registry = HandlerRegistry::new();
        let short = |ctx: &mut ExecutionContext, _call: &NodeCall<'_>| {
            ctx.set_var("winner", Value::String("short".into()));
            Ok(())
        };
        let long = |ctx: &mut ExecutionContext, _call: &NodeCall<'_>| {
            ctx.set_var("winner", Value::String("long".into()));
            Ok(())
        };
        registry.register_prefix("A:", short).unwrap();
        registry.register_prefix("A:B:", long).unwrap();

        let mut ctx = ExecutionContext::new();
        registry.dispatch(&mut ctx, "n", "N", "A:B:1");
        assert_eq!(ctx.get_var("winner"), Some(&Value::String("long".into())));
    }
}
