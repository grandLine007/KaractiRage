//! Rendu UI data-driven : consomme `subsystems::UiScreen`/`UiElement` pour afficher
//! HUD et écrans (menu, pause, victoire, game over) pilotés par le contexte KVL.
//!
//! Complément du rendu procédural historique dans `game.rs` : quand un écran `Hud`
//! déclaratif contient des éléments, il remplace la pilule codée en dur. Les écrans
//! plein écran (menu/pause/victoire/game over) sont superposés selon l'état runtime.

use kvl_core::Value;
use kvl_runtime::ExecutionContext;

use crate::renderer::Canvas;
use crate::subsystems::{UiElement, UiScreen, UiScreenKind};

/// Charte Dark Atelier (partagée avec le HUD procédural de `game.rs`).
const GLASS_BG: u32 = 0x000F172A;
const GLASS_BORDER: u32 = 0x00334155;
const ACCENT_CYAN: u32 = 0x0038BDF8;
const TEXT_MAIN: u32 = 0x00F8FAFC;
const TEXT_MUTED: u32 = 0x0094A3B8;
const BAR_BG: u32 = 0x001E293B;
const BAR_FILL: u32 = 0x0010B981;
const BAR_FILL_HP: u32 = 0x00EF4444;
const OVERLAY_VEIL: u32 = 0x00060911;

/// Hauteur verticale consommée par un élément UI (curseur d'empilement).
const ROW_H: i32 = 22;
/// Largeur d'une barre de progression.
const BAR_W: u32 = 160;

/// Formatte une valeur KVL pour l'affichage HUD (sans unité, concis).
pub fn format_bound_value(value: &Value) -> String {
    match value {
        Value::Bool(b) => {
            if *b {
                "OUI".to_string()
            } else {
                "NON".to_string()
            }
        }
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format!("{f:.0}"),
        Value::String(s) => s.clone(),
        Value::Vector2(v) => format!("{}, {}", v[0] as i64, v[1] as i64),
        Value::Vector3(v) => format!("{}, {}, {}", v[0] as i64, v[1] as i64, v[2] as i64),
        Value::EntityRef(e) => e.clone(),
    }
}

/// Résout la valeur liée (`bind`) d'un élément depuis le contexte KVL, en texte.
pub fn resolve_bind(ctx: &ExecutionContext, bind: Option<&str>) -> Option<String> {
    let name = bind?;
    ctx.get_var(name).map(format_bound_value)
}

/// Convertit une valeur KVL en `f32` (pour les jauges).
fn value_as_f32(value: &Value) -> Option<f32> {
    match value {
        Value::Int(i) => Some(*i as f32),
        Value::Float(f) => Some(*f as f32),
        _ => None,
    }
}

/// Lit la valeur courante et son maximum pour une jauge liée à `bind`.
///
/// Convention : le maximum est lu dans `{bind}_max` (ex. `player_hp` → `player_max_hp`),
/// comme synchronisé par le moteur de vitalité. À défaut, le maximum vaut au moins la
/// valeur courante (ou 1) pour éviter une division par zéro.
fn bound_value_and_max(ctx: &ExecutionContext, bind: Option<&str>) -> (f32, f32) {
    let Some(name) = bind else {
        return (0.0, 0.0);
    };
    let cur = ctx
        .get_var(name)
        .and_then(value_as_f32)
        .unwrap_or(0.0);
    let max_name = format!("{name}_max");
    let max = ctx
        .get_var(&max_name)
        .and_then(value_as_f32)
        .unwrap_or_else(|| cur.max(1.0));
    (cur, max)
}

/// Texte affichable d'un élément de type `text` (label + valeur liée).
fn element_text(el: &UiElement, ctx: &ExecutionContext) -> Option<String> {
    let label = el.label.as_deref().unwrap_or("");
    let value = resolve_bind(ctx, el.bind.as_deref());
    match (label.is_empty(), value) {
        (true, Some(v)) => Some(v),
        (false, Some(v)) => Some(format!("{label}: {v}")),
        (false, None) => Some(label.to_string()),
        (true, None) => None,
    }
}

/// Rend un élément UI data-driven à `(x, y)` ; renvoie le Y du curseur suivant.
pub fn render_element(
    canvas: &mut Canvas,
    el: &UiElement,
    ctx: &ExecutionContext,
    x: i32,
    y: i32,
) -> i32 {
    match el.kind.as_str() {
        "bar" => render_bar(canvas, el, ctx, x, y),
        "button" => render_button(canvas, el, x, y),
        "icon" => render_icon(canvas, el, x, y),
        _ => render_text(canvas, el, ctx, x, y),
    }
}

fn render_text(canvas: &mut Canvas, el: &UiElement, ctx: &ExecutionContext, x: i32, y: i32) -> i32 {
    match element_text(el, ctx) {
        Some(line) => {
            canvas.draw_text(x, y, &line, TEXT_MAIN, 1);
            y + ROW_H
        }
        None => y,
    }
}

fn render_bar(canvas: &mut Canvas, el: &UiElement, ctx: &ExecutionContext, x: i32, y: i32) -> i32 {
    let mut cursor = y;

    // Étiquette optionnelle au-dessus de la jauge.
    if let Some(label) = el.label.as_deref() {
        if !label.is_empty() {
            canvas.draw_text(x, cursor, label, TEXT_MUTED, 1);
            cursor += 14;
        }
    }

    let (cur, max) = bound_value_and_max(ctx, el.bind.as_deref());
    let bar_h: u32 = 8;
    canvas.draw_rect(x, cursor, BAR_W, bar_h, BAR_BG);

    let ratio = if max > 0.0 {
        (cur / max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fill_w = (BAR_W as f32 * ratio) as u32;
    if fill_w > 0 {
        let fill = if el.bind.as_deref().map_or(false, |b| b.contains("hp")) {
            BAR_FILL_HP
        } else {
            BAR_FILL
        };
        canvas.draw_rect(x, cursor, fill_w, bar_h, fill);
        canvas.draw_line(x, cursor, x + fill_w as i32 - 1, cursor, 0x00FFFFFF);
    }
    canvas.draw_rect_outline(x, cursor, BAR_W, bar_h, GLASS_BORDER, 1);

    // Valeur numérique à droite de la jauge.
    let value = format!("{cur:.0}/{max:.0}");
    canvas.draw_text(x + BAR_W as i32 + 8, cursor - 1, &value, TEXT_MAIN, 1);

    cursor + (bar_h as i32) + 4
}

fn render_button(canvas: &mut Canvas, el: &UiElement, x: i32, y: i32) -> i32 {
    let label = el.label.as_deref().unwrap_or("OK");
    canvas.draw_badge(x, y, 128, 24, GLASS_BG, ACCENT_CYAN, label, TEXT_MAIN, 1);
    y + 30
}

fn render_icon(canvas: &mut Canvas, el: &UiElement, x: i32, y: i32) -> i32 {
    // Icône = glyphe textuel (ex. cœur « ♥ ») rendu à plus grande échelle.
    let glyph = el.label.as_deref().unwrap_or("•");
    canvas.draw_text(x, y, glyph, ACCENT_CYAN, 2);
    y + 20
}

/// Rend le contenu d'un écran (titre + éléments) à `(x, y)` ; renvoie le Y suivant.
pub fn render_screen(
    canvas: &mut Canvas,
    screen: &UiScreen,
    ctx: &ExecutionContext,
    x: i32,
    y: i32,
) -> i32 {
    let mut cursor = y;
    if let Some(title) = screen.title.as_deref() {
        if !title.is_empty() {
            canvas.draw_text(x, cursor, title, TEXT_MAIN, 2);
            cursor += 26;
        }
    }
    for el in &screen.elements {
        cursor = render_element(canvas, el, ctx, x, cursor);
    }
    cursor
}

/// Largeur du panneau HUD calculée sur la plus longue ligne de contenu.
fn panel_width(canvas: &Canvas, screen: &UiScreen, ctx: &ExecutionContext) -> u32 {
    let mut max_w: u32 = 80;
    if let Some(title) = screen.title.as_deref() {
        if !title.is_empty() {
            max_w = max_w.max(canvas.measure_text(title, 2).0 as u32 + 24);
        }
    }
    for el in &screen.elements {
        if let Some(line) = element_text(el, ctx) {
            max_w = max_w.max(canvas.measure_text(&line, 1).0 as u32 + 24);
        }
    }
    max_w
}

/// Hauteur du panneau en fonction du titre et du nombre d'éléments.
fn panel_height(screen: &UiScreen) -> u32 {
    let title = if screen.title.as_deref().map_or(false, |t| !t.is_empty()) {
        26
    } else {
        0
    };
    (title + screen.elements.len() as i32 * ROW_H + 20).max(34) as u32
}

/// Cadre glassmorphism Dark Atelier (fond + liseré + cornières).
fn draw_panel(canvas: &mut Canvas, x: i32, y: i32, w: u32, h: u32) {
    canvas.draw_rect(x, y, w, h, GLASS_BG);
    canvas.draw_rect_outline(x, y, w, h, GLASS_BORDER, 1);
    canvas.draw_corner_brackets(x, y, w, h, 8, ACCENT_CYAN);
}

/// Rend un HUD data-driven compact (pilule glass) en haut à gauche du viewport.
/// Renvoie le Y sous le panneau (pour empiler d'autres widgets sans recouvrement).
pub fn render_hud_panel(
    canvas: &mut Canvas,
    screen: &UiScreen,
    ctx: &ExecutionContext,
    x: i32,
    y: i32,
) -> i32 {
    if screen.elements.is_empty() {
        return y;
    }
    let w = panel_width(canvas, screen, ctx);
    let h = panel_height(screen);
    draw_panel(canvas, x, y, w, h);
    render_screen(canvas, screen, ctx, x + 12, y + 10);
    y + h as i32 + 4
}

/// Rend un écran plein écran centré (menu, pause, victoire, game over) sur un voile.
pub fn render_overlay(
    canvas: &mut Canvas,
    screen: &UiScreen,
    ctx: &ExecutionContext,
    vx: i32,
    vy: i32,
    vw: i32,
    vh: i32,
) {
    // Voile opaque : isole l'écran du monde rendu en arrière-plan.
    canvas.draw_rect(vx, vy, vw as u32, vh as u32, OVERLAY_VEIL);

    let panel_w: u32 = 460;
    let panel_h = panel_height(screen);
    let px = vx + (vw - panel_w as i32) / 2;
    let py = vy + (vh - panel_h as i32) / 2;

    canvas.draw_rect(px, py, panel_w, panel_h, GLASS_BG);
    canvas.draw_rect_outline(px, py, panel_w, panel_h, ACCENT_CYAN, 1);
    canvas.draw_corner_brackets(px, py, panel_w, panel_h, 12, ACCENT_CYAN);

    render_screen(canvas, screen, ctx, px + 24, py + 24);
}

/// Déclencheur d'écran : état runtime qui pilote la bascule HUD/menu/pause/….
///
/// La résolution est faite par [`crate::screens::GameScreen::trigger`] : depuis P1.1,
/// l'état d'écran fait autorité (les variables KVL en sont le miroir, pas la source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenTrigger {
    Hud,
    Menu,
    Pause,
    Victory,
    GameOver,
}

/// Retourne l'écran déclaré correspondant au déclencheur, ou `None` si absent.
pub fn active_screen<'a>(screens: &'a [UiScreen], trigger: ScreenTrigger) -> Option<&'a UiScreen> {
    let kind = match trigger {
        ScreenTrigger::Hud => UiScreenKind::Hud,
        ScreenTrigger::Menu => UiScreenKind::Menu,
        ScreenTrigger::Pause => UiScreenKind::Pause,
        ScreenTrigger::Victory => UiScreenKind::Victory,
        ScreenTrigger::GameOver => UiScreenKind::GameOver,
    };
    screens.iter().find(|s| s.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(vars: &[(&str, Value)]) -> ExecutionContext {
        let mut ctx = ExecutionContext::new();
        for (k, v) in vars {
            ctx.set_var(*k, v.clone());
        }
        ctx
    }

    fn hud_screen() -> UiScreen {
        UiScreen {
            id: "hud_0".into(),
            kind: UiScreenKind::Hud,
            title: None,
            elements: vec![
                UiElement {
                    id: "hp".into(),
                    kind: "text".into(),
                    label: Some("VIE".into()),
                    bind: Some("player_hp".into()),
                },
                UiElement {
                    id: "hp_bar".into(),
                    kind: "bar".into(),
                    label: Some("VITALITÉ".into()),
                    bind: Some("player_hp".into()),
                },
            ],
        }
    }

    #[test]
    fn format_bound_value_covers_all_variants() {
        assert_eq!(format_bound_value(&Value::Bool(true)), "OUI");
        assert_eq!(format_bound_value(&Value::Int(42)), "42");
        assert_eq!(format_bound_value(&Value::Float(3.14)), "3");
        assert_eq!(format_bound_value(&Value::String("objectif".into())), "objectif");
        assert_eq!(format_bound_value(&Value::Vector2([1.0, 2.0])), "1, 2");
        assert_eq!(format_bound_value(&Value::Vector3([1.0, 2.0, 3.0])), "1, 2, 3");
        assert_eq!(format_bound_value(&Value::EntityRef("e_1".into())), "e_1");
    }

    #[test]
    fn resolve_bind_reads_context_and_handles_missing() {
        let ctx = ctx_with(&[("score", Value::Int(7))]);
        assert_eq!(resolve_bind(&ctx, Some("score")), Some("7".to_string()));
        assert_eq!(resolve_bind(&ctx, Some("absent")), None);
        assert_eq!(resolve_bind(&ctx, None), None);
    }

    #[test]
    fn active_screen_matches_declared_kind() {
        let screens = vec![hud_screen()];
        assert_eq!(
            active_screen(&screens, ScreenTrigger::Hud).unwrap().kind,
            UiScreenKind::Hud
        );
        assert!(active_screen(&screens, ScreenTrigger::Victory).is_none());
    }

    #[test]
    fn render_hud_panel_draws_without_panic() {
        let screen = hud_screen();
        let ctx = ctx_with(&[
            ("player_hp", Value::Int(2)),
            ("player_max_hp", Value::Int(3)),
        ]);
        let mut canvas = Canvas::new(320, 200);
        canvas.clear(0x00080B14);
        render_hud_panel(&mut canvas, &screen, &ctx, 18, 12);
        assert!(canvas.buffer.iter().any(|&p| p != 0x00080B14), "Le HUD doit écrire des pixels");
    }

    #[test]
    fn render_overlay_draws_centered_without_panic() {
        let screen = UiScreen {
            id: "victory".into(),
            kind: UiScreenKind::Victory,
            title: Some("VICTOIRE".into()),
            elements: vec![UiElement {
                id: "btn".into(),
                kind: "button".into(),
                label: Some("REJOUER".into()),
                bind: None,
            }],
        };
        let ctx = ctx_with(&[]);
        let mut canvas = Canvas::new(640, 480);
        canvas.clear(0x00080B14);
        render_overlay(&mut canvas, &screen, &ctx, 0, 0, 640, 480);
        assert!(canvas.buffer.iter().any(|&p| p == OVERLAY_VEIL), "Le voile doit recouvrir l'écran");
    }
}
