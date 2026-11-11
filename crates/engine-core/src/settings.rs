//! Réglages utilisateur **hors partie** (audio pour l'instant).
//!
//! Ils sont volontairement séparés des sauvegardes de jeu : couper le son ne doit pas
//! dépendre du slot, et une sauvegarde ne doit pas écraser les préférences.
//! Stockés dans `$KARACTI_HOME/settings.json` (par défaut `~/.karactirage/settings.json`).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::project_registry::ProjectRegistry;

/// Réglages audio persistants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Volume maître dans `[0, 1]`.
    pub volume: f32,
    pub muted: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { volume: 0.5, muted: false }
    }
}

impl AudioSettings {
    /// Chemin du fichier de réglages utilisateur.
    pub fn path() -> PathBuf {
        ProjectRegistry::home_dir().join("settings.json")
    }

    /// Lit les réglages ; toute erreur (absent, illisible, corrompu) donne les défauts.
    pub fn load() -> Self {
        let Ok(content) = fs::read_to_string(Self::path()) else {
            return Self::default();
        };
        let settings: Self = serde_json::from_str(&content).unwrap_or_default();
        settings.sanitized()
    }

    /// Lit les réglages depuis un chemin précis (tests).
    pub fn load_from(path: &std::path::Path) -> Self {
        let Ok(content) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&content).unwrap_or_default().sanitized()
    }

    /// Écrit les réglages (crée le dossier si besoin).
    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let clean = AudioSettings { volume: self.volume, muted: self.muted }.sanitized();
        let json = serde_json::to_string_pretty(&clean).map_err(|error| error.to_string())?;
        fs::write(path, json).map_err(|error| error.to_string())
    }

    /// Borne le volume dans `[0, 1]` (une valeur aberrante ne doit pas assourdir le jeu).
    pub fn sanitized(mut self) -> Self {
        if !self.volume.is_finite() {
            self.volume = AudioSettings::default().volume;
        }
        self.volume = self.volume.clamp(0.0, 1.0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("karacti_settings_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("settings.json")
    }

    #[test]
    fn defaults_are_sane() {
        let settings = AudioSettings::default();
        assert_eq!(settings.volume, 0.5);
        assert!(!settings.muted);
    }

    #[test]
    fn roundtrip_through_disk() {
        let path = tmp_file("roundtrip");
        let settings = AudioSettings { volume: 0.25, muted: true };
        settings.save_to(&path).expect("écriture");
        let loaded = AudioSettings::load_from(&path);
        assert_eq!(loaded, settings);
        let _ = fs::remove_dir_all(path.parent().expect("dossier"));
    }

    #[test]
    fn missing_or_corrupt_file_falls_back_to_defaults() {
        let path = tmp_file("corrupt");
        assert_eq!(AudioSettings::load_from(&path), AudioSettings::default());

        fs::create_dir_all(path.parent().expect("dossier")).expect("dossier");
        fs::write(&path, "{ pas du json").expect("écriture");
        assert_eq!(AudioSettings::load_from(&path), AudioSettings::default());
        let _ = fs::remove_dir_all(path.parent().expect("dossier"));
    }

    #[test]
    fn volume_is_clamped_and_non_finite_is_rejected() {
        assert_eq!(AudioSettings { volume: 4.0, muted: false }.sanitized().volume, 1.0);
        assert_eq!(AudioSettings { volume: -2.0, muted: false }.sanitized().volume, 0.0);
        assert_eq!(
            AudioSettings { volume: f32::NAN, muted: false }.sanitized().volume,
            AudioSettings::default().volume
        );
    }
}
