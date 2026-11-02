//! Sauvegarde runtime data-driven : consomme `subsystems::SaveConfig`.
//!
//! `SaveManager` expose des emplacements (slots) de progression de jeu. Les règles
//! `enabled` / `slots` / `auto_save` sont appliquées ici, à distance du hot path de
//! simulation : aucune écriture disque n'est déclenchée par `update()`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::project::KaractiProject;
use crate::project_registry::ProjectRegistry;
use crate::subsystems::SaveConfig;

/// Intervalle d'auto-save (secondes simulées).
pub const AUTO_SAVE_INTERVAL: f32 = 30.0;

/// Gère les emplacements de sauvegarde (slots) d'un projet, pilotés par `SaveConfig`.
#[derive(Debug, Clone)]
pub struct SaveManager {
    pub config: SaveConfig,
    root: PathBuf,
    /// Emplacement actif (0 = par défaut).
    pub active_slot: usize,
    /// Accumulateur d'auto-save (secondes).
    auto_save_accum: f32,
}

impl SaveManager {
    /// Crée un gestionnaire avec le répertoire de sauvegarde par défaut.
    pub fn new(config: SaveConfig) -> Self {
        Self::with_root(config, ProjectRegistry::home_dir().join("saves"))
    }

    /// Crée un gestionnaire avec un répertoire précis (tests, `KARACTI_HOME`…).
    pub fn with_root(config: SaveConfig, root: PathBuf) -> Self {
        Self {
            config,
            root,
            active_slot: 0,
            auto_save_accum: 0.0,
        }
    }

    /// Racine des sauvegardes.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Nom de dossier sûr pour un identifiant de projet.
    fn dir_name(project_id: &str) -> String {
        let mut out = String::with_capacity(project_id.len());
        for c in project_id.chars() {
            out.push(if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            });
        }
        if out.is_empty() {
            out.push_str("projet");
        }
        out
    }

    fn project_dir(&self, project_id: &str) -> PathBuf {
        self.root.join(Self::dir_name(project_id))
    }

    /// Chemin du fichier d'un slot.
    pub fn slot_path(&self, project_id: &str, slot: usize) -> PathBuf {
        self.project_dir(project_id).join(format!("slot_{slot}.karacti"))
    }

    /// Valide un index de slot (dans `0..slots`) en appliquant `enabled`.
    pub fn checked_slot(&self, slot: usize) -> Result<usize, String> {
        if !self.config.enabled {
            return Err("sauvegarde désactivée (save_config.enabled = false)".into());
        }
        let max = self.config.slots.max(1) as usize;
        if slot >= max {
            return Err(format!("slot {slot} hors limites (slots = {max})"));
        }
        Ok(slot)
    }

    /// Sauvegarde le projet dans un slot ; renvoie le chemin écrit.
    pub fn save(&self, project: &KaractiProject, slot: usize) -> Result<PathBuf, String> {
        let slot = self.checked_slot(slot)?;
        let path = self.slot_path(&project.metadata.id, slot);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        project.save_to_file(&path).map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// Charge un projet depuis un slot.
    pub fn load(&self, project_id: &str, slot: usize) -> Result<KaractiProject, String> {
        self.checked_slot(slot)?;
        let path = self.slot_path(project_id, slot);
        KaractiProject::load_from_file(&path).map_err(|e| format!("slot {slot} illisible : {e}"))
    }

    /// Liste les slots existants `(index, chemin)`, triés par index croissant.
    pub fn list_slots(&self, project_id: &str) -> Vec<(usize, PathBuf)> {
        let mut out = Vec::new();
        for slot in 0..self.config.slots.max(1) as usize {
            let path = self.slot_path(project_id, slot);
            if path.exists() {
                out.push((slot, path));
            }
        }
        out
    }

    /// Reprise en session : charge le slot le plus récemment modifié, ou `None`.
    pub fn resume(&self, project_id: &str) -> Option<KaractiProject> {
        if !self.config.enabled {
            return None;
        }
        let mut slots = self.list_slots(project_id);
        slots.sort_by_key(|(_, p)| fs::metadata(p).and_then(|m| m.modified()).ok());
        let (slot, _) = slots.last()?;
        self.load(project_id, *slot).ok()
    }

    /// Avance l'auto-save : sauvegarde dans le slot actif quand l'intervalle est atteint.
    /// Renvoie le chemin écrit le cas échéant, ou `None` si rien n'est écrit ce tick.
    pub fn auto_save_tick(
        &mut self,
        dt: f32,
        project: &KaractiProject,
    ) -> Result<Option<PathBuf>, String> {
        if !self.config.enabled || !self.config.auto_save {
            return Ok(None);
        }
        self.auto_save_accum += dt;
        if self.auto_save_accum >= AUTO_SAVE_INTERVAL {
            self.auto_save_accum = 0.0;
            self.save(project, self.active_slot).map(Some)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::KaractiProject;
    use crate::recipe::GameGenre;

    fn tmp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "karacti_save_{tag}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn project(title: &str) -> KaractiProject {
        KaractiProject::new_empty(title, "auteur", GameGenre::Platformer)
    }

    fn default_config() -> SaveConfig {
        SaveConfig {
            enabled: true,
            slots: 3,
            auto_save: true,
        }
    }

    #[test]
    fn save_and_load_roundtrip_preserves_title() {
        let root = tmp_root("roundtrip");
        let manager = SaveManager::with_root(default_config(), root.clone());
        let proj = project("Ma Partie");

        let path = manager.save(&proj, 1).expect("sauvegarde");
        assert!(path.exists());

        let loaded = manager.load(&proj.metadata.id, 1).expect("chargement");
        assert_eq!(loaded.metadata.title, "Ma Partie");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn disabled_save_and_out_of_range_slot_are_errors() {
        let root = tmp_root("gates");
        let mut disabled = default_config();
        disabled.enabled = false;
        let manager = SaveManager::with_root(disabled, root.clone());
        let proj = project("Gate");

        assert!(manager.save(&proj, 0).is_err());
        assert!(manager.load(&proj.metadata.id, 0).is_err());
        assert!(manager.resume(&proj.metadata.id).is_none());

        let manager = SaveManager::with_root(default_config(), root.clone());
        assert!(manager.save(&proj, 3).is_err(), "slot 3 hors limites (slots = 3)");
        assert!(manager.save(&proj, 2).is_ok());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn list_slots_and_resume_pick_most_recent() {
        let root = tmp_root("resume");
        let manager = SaveManager::with_root(default_config(), root.clone());
        let proj = project("Reprise");
        let id = proj.metadata.id.clone();

        manager.save(&proj, 0).expect("slot 0");
        manager.save(&proj, 2).expect("slot 2");

        let slots = manager.list_slots(&id);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].0, 0);
        assert_eq!(slots[1].0, 2);

        let resumed = manager.resume(&id).expect("reprise");
        assert_eq!(resumed.metadata.id, id);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_save_triggers_only_after_interval() {
        let root = tmp_root("autosave");
        let mut manager = SaveManager::with_root(default_config(), root.clone());
        let proj = project("Auto");
        let id = proj.metadata.id.clone();

        // Rien avant l'intervalle.
        for _ in 0..29 {
            assert!(manager.auto_save_tick(1.0, &proj).unwrap().is_none());
        }
        assert!(manager.list_slots(&id).is_empty());

        // À 30 s cumulés, le slot actif (0) est écrit.
        let written = manager.auto_save_tick(1.0, &proj).unwrap();
        assert!(written.is_some());
        assert!(manager.slot_path(&id, 0).exists());

        // auto_save = false → jamais d'écriture.
        let mut no_auto = default_config();
        no_auto.auto_save = false;
        let mut manager = SaveManager::with_root(no_auto, root.clone());
        for _ in 0..100 {
            assert!(manager.auto_save_tick(1.0, &proj).unwrap().is_none());
        }

        let _ = fs::remove_dir_all(&root);
    }
}

