//! Machine à états des écrans de jeu (P1.1).
//!
//! **Un seul état fait autorité** : [`GameScreen`]. Les variables KVL `menu_active`,
//! `paused` et `game_won` restent écrites en **sortie** (miroir) pour que la logique du
//! créateur puisse réagir, mais elles ne pilotent plus l'écran — sinon il y aurait deux
//! sources de vérité et des écrans fantômes.
//!
//! Règle de compatibilité : on ne bascule que vers un écran **déclaré** dans le projet.
//! Sans écran `menu`/`pause`/`game_over`/`victory`, le comportement d'avant P1.1 est
//! conservé (le jeu ne se fige jamais dans le vide).

use crate::ui_render::ScreenTrigger;

/// État d'écran courant du runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameScreen {
    /// Menu principal : le monde est figé, aucune simulation avancée.
    Menu,
    /// En jeu : HUD actif, simulation avancée.
    #[default]
    Playing,
    /// Pause : monde figé, reprise par `Entrée` ou `Échap`.
    Paused,
    /// Défaite du joueur : monde figé, retour au menu par `Entrée`.
    GameOver,
    /// Victoire (variable KVL `game_won`) : monde figé, retour au menu.
    Victory,
}

impl GameScreen {
    /// Écran de départ : menu uniquement s'il est déclaré **et** demandé.
    pub fn initial(has_menu_screen: bool, menu_requested: bool) -> Self {
        if has_menu_screen && menu_requested {
            Self::Menu
        } else {
            Self::Playing
        }
    }

    /// Vrai si la simulation doit avancer.
    pub fn is_playing(self) -> bool {
        matches!(self, Self::Playing)
    }

    /// `Entrée` / clic : démarrer, reprendre, ou revenir au menu.
    pub fn confirm(self) -> Self {
        match self {
            Self::Menu | Self::Paused => Self::Playing,
            Self::GameOver | Self::Victory => Self::Menu,
            Self::Playing => Self::Playing,
        }
    }

    /// `Échap` : bascule pause ↔ jeu. Sans effet sur les écrans terminaux.
    pub fn pause_toggle(self) -> Self {
        match self {
            Self::Playing => Self::Paused,
            Self::Paused => Self::Playing,
            other => other,
        }
    }

    /// Déclencheur d'écran correspondant (le HUD n'est qu'un fond en jeu).
    pub fn trigger(self) -> ScreenTrigger {
        match self {
            Self::Menu => ScreenTrigger::Menu,
            Self::Playing => ScreenTrigger::Hud,
            Self::Paused => ScreenTrigger::Pause,
            Self::GameOver => ScreenTrigger::GameOver,
            Self::Victory => ScreenTrigger::Victory,
        }
    }

    /// Libellé pour les messages de statut et les tests.
    pub fn label(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::Playing => "jeu",
            Self::Paused => "pause",
            Self::GameOver => "défaite",
            Self::Victory => "victoire",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_screen_requires_a_declared_menu() {
        assert_eq!(GameScreen::initial(false, true), GameScreen::Playing);
        assert_eq!(GameScreen::initial(true, false), GameScreen::Playing);
        assert_eq!(GameScreen::initial(true, true), GameScreen::Menu);
    }

    #[test]
    fn confirm_starts_resumes_and_returns_to_menu() {
        assert_eq!(GameScreen::Menu.confirm(), GameScreen::Playing);
        assert_eq!(GameScreen::Paused.confirm(), GameScreen::Playing);
        assert_eq!(GameScreen::GameOver.confirm(), GameScreen::Menu);
        assert_eq!(GameScreen::Victory.confirm(), GameScreen::Menu);
        // En jeu, Entrée n'a pas d'effet d'écran (le jeu garde la main).
        assert_eq!(GameScreen::Playing.confirm(), GameScreen::Playing);
    }

    #[test]
    fn escape_toggles_pause_only_from_play() {
        assert_eq!(GameScreen::Playing.pause_toggle(), GameScreen::Paused);
        assert_eq!(GameScreen::Paused.pause_toggle(), GameScreen::Playing);
        assert_eq!(GameScreen::Menu.pause_toggle(), GameScreen::Menu);
        assert_eq!(GameScreen::GameOver.pause_toggle(), GameScreen::GameOver);
        assert_eq!(GameScreen::Victory.pause_toggle(), GameScreen::Victory);
    }

    #[test]
    fn only_playing_advances_the_simulation() {
        assert!(GameScreen::Playing.is_playing());
        for screen in [GameScreen::Menu, GameScreen::Paused, GameScreen::GameOver, GameScreen::Victory] {
            assert!(!screen.is_playing(), "{} ne doit pas avancer", screen.label());
        }
    }

    #[test]
    fn triggers_map_to_the_declared_screen_kinds() {
        assert_eq!(GameScreen::Playing.trigger(), ScreenTrigger::Hud);
        assert_eq!(GameScreen::Menu.trigger(), ScreenTrigger::Menu);
        assert_eq!(GameScreen::Paused.trigger(), ScreenTrigger::Pause);
        assert_eq!(GameScreen::GameOver.trigger(), ScreenTrigger::GameOver);
        assert_eq!(GameScreen::Victory.trigger(), ScreenTrigger::Victory);
    }
}
