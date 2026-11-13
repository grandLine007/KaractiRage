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
///
/// Le layout est calculé sur le contenu (`title` + éléments) : la carte s'élargit et
/// grandit avec lui, avec des marges internes généreuses. Le HUD emprunte un autre
/// chemin (`render_hud_panel`) et reste inchangé.
pub fn render_overlay(
    canvas: &mut Canvas,
    screen: &UiScreen,
    ctx: &ExecutionContext,
    vx: i32,
    vy: i32,
    vw: i32,
    vh: i32,
) {
    // Voile semi-transparent : le décor du jeu reste perceptible derrière la carte.
    draw_overlay_veil(canvas, vx, vy, vw, vh);
    let layout = overlay_layout(screen, ctx, vx, vy, vw, vh);
    draw_overlay_card(canvas, screen, ctx, &layout);
}

// ---------------------------------------------------------------------------
// Écrans plein écran — charte Dark Atelier du Studio.
//
// Tokens alignés sur `apps/studio/src/index.css` : fond panneau `#0d0819`,
// carte `#150d27`, accent `#a855f7`, texte principal `#f8fafc`, secondaire
// `#94a3b8`. Toutes les tailles sont calculées à partir du contenu mesuré.
// ---------------------------------------------------------------------------

/// Fond de la carte de menu (token `--bg-panel`).
const MENU_BG: u32 = 0x000D0819;
/// Fond d'une ligne d'entrée (token `--bg-card`).
const MENU_ROW_BG: u32 = 0x00150D27;
/// Fond de la touche mise en valeur (token `--bg-card-hover`).
const MENU_CHIP_BG: u32 = 0x001E1338;
/// Bordure discrète (token `--border-subtle`).
const MENU_BORDER: u32 = 0x002D1B4E;
/// Accent principal (token `--accent-primary`).
const MENU_ACCENT: u32 = 0x00A855F7;
/// Accent clair, lisible sur les fonds de carte (contraste ≥ 4,5:1).
const MENU_ACCENT_SOFT: u32 = 0x00C084FC;
/// Accent profond pour les traits décoratifs.
const MENU_ACCENT_DEEP: u32 = 0x007E22CE;
/// Voile sombre posé sur le décor du jeu. Teinte calibrée pour que le fond
/// dominant du niveau (token `#0d1525`) retombe exactement sur `#060911`.
const MENU_VEIL: u32 = 0x0005070D;
/// Opacité du voile (0–255) : le décor reste visible à ~20 %.
const MENU_VEIL_ALPHA: u32 = 205;

/// Marge entre la carte et le bord du viewport.
const OVERLAY_MARGIN: i32 = 24;
/// Largeur minimale de la carte (avant calcul sur le contenu).
const OVERLAY_MIN_W: i32 = 460;
/// Largeur maximale de la carte (confort de lecture).
const OVERLAY_MAX_W: i32 = 760;
/// Marge interne horizontale de la carte.
const OVERLAY_PAD_X: i32 = 44;
/// Marge interne verticale de la carte.
const OVERLAY_PAD_Y: i32 = 34;
/// Taille du titre (police vectorielle Space Grotesk Bold, tailles libres).
const OVERLAY_TITLE_PT: f32 = 30.0;
/// Interlettrage du titre (px) : donne de la présence sans glyphe inventé.
const OVERLAY_TITLE_TRACK: i32 = 2;
/// Taille du texte de la touche.
const OVERLAY_KEY_PT: f32 = 14.0;
/// Taille du texte descriptif d'une entrée.
const OVERLAY_DESC_PT: f32 = 15.0;
/// Hauteur d'une ligne d'entrée.
const OVERLAY_ROW_H: i32 = 34;
/// Hauteur d'une ligne de jauge.
const OVERLAY_BAR_H: i32 = 48;
/// Espace vertical entre deux lignes.
const OVERLAY_ROW_GAP: i32 = 10;
/// Marge interne horizontale d'une ligne.
const OVERLAY_ROW_PAD: i32 = 16;
/// Hauteur de la pastille de touche.
const OVERLAY_CHIP_H: i32 = 24;
/// Marge interne horizontale de la pastille.
const OVERLAY_CHIP_PAD: i32 = 8;
/// Taille d'un glyphe d'icône.
const OVERLAY_ICON_PT: f32 = 18.0;
/// Espace réservé au chevron de commande à droite d'une ligne.
const OVERLAY_CHEVRON_GAP: i32 = 22;

/// Rectangle de la carte calculé pour un écran et un viewport donnés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OverlayLayout {
    px: i32,
    py: i32,
    w: i32,
    h: i32,
}

/// Division entière exacte par 255 (même identité que `renderer::div255`).
#[inline]
fn div255_local(v: u32) -> u32 {
    (v + 1 + (v >> 8)) >> 8
}

/// Applique le voile sombre du menu sans écraser le décor (alpha blending direct).
///
/// Le buffer est parcouru par lignes contiguës : aucun appel `set_pixel` par pixel,
/// donc un coût linéaire borné même en wasm.
fn draw_overlay_veil(canvas: &mut Canvas, vx: i32, vy: i32, vw: i32, vh: i32) {
    let mut x0 = vx.max(0).min(canvas.width as i32);
    let mut y0 = vy.max(0).min(canvas.height as i32);
    let mut x1 = (vx + vw).max(0).min(canvas.width as i32);
    let mut y1 = (vy + vh).max(0).min(canvas.height as i32);
    if let Some((cx, cy, cw, ch)) = canvas.clip_rect {
        x0 = x0.max(cx);
        y0 = y0.max(cy);
        x1 = x1.min(cx.saturating_add(cw as i32));
        y1 = y1.min(cy.saturating_add(ch as i32));
    }
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let inv = 255 - MENU_VEIL_ALPHA;
    let vr = (MENU_VEIL >> 16) & 0xFF;
    let vg = (MENU_VEIL >> 8) & 0xFF;
    let vb = MENU_VEIL & 0xFF;
    for y in y0..y1 {
        let row = y as usize * canvas.width;
        for x in x0..x1 {
            let bg = canvas.buffer[row + x as usize];
            let r = div255_local(vr * MENU_VEIL_ALPHA + ((bg >> 16) & 0xFF) * inv);
            let g = div255_local(vg * MENU_VEIL_ALPHA + ((bg >> 8) & 0xFF) * inv);
            let b = div255_local(vb * MENU_VEIL_ALPHA + (bg & 0xFF) * inv);
            canvas.buffer[row + x as usize] = (r << 16) | (g << 8) | b;
        }
    }
}

/// Coupe une étiquette de commande en « touche — action ».
///
/// Le séparateur et son espacement sont conservés à l'affichage : seule la mise
/// en forme change, jamais le contenu textuel déclaré par le projet.
fn split_command(label: &str) -> Option<(&str, &str)> {
    for sep in ['—', '–'] {
        if let Some(i) = label.find(sep) {
            let key = label[..i].trim_end();
            let rest = label[i + sep.len_utf8()..].trim_start();
            if !key.is_empty() && !rest.is_empty() {
                return Some((key, rest));
            }
        }
    }
    None
}

/// Largeur d'un texte avec interlettrage, pour une police donnée.
fn text_width(font: &fontdue::Font, text: &str, size: f32, track: i32) -> i32 {
    let (w, _) = Canvas::measure_text_styled(font, text, size);
    let n = text.chars().count() as i32;
    w + track * (n - 1).max(0)
}

/// Largeur d'un titre avec interlettrage (police d'affichage).
fn tracked_width(text: &str, size: f32, track: i32) -> i32 {
    text_width(crate::renderer::get_title_font(), text, size, track)
}

/// Tronque un texte à `max_w` en conservant une ellipse lisible.
fn truncate_text(font: &fontdue::Font, text: &str, size: f32, max_w: i32, track: i32) -> String {
    if max_w <= 0 {
        return String::new();
    }
    if text_width(font, text, size, track) <= max_w {
        return text.to_string();
    }
    let ell = "...";
    let ell_w = text_width(font, ell, size, track) + track;
    if max_w < ell_w {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in text.chars() {
        let mut buf = [0u8; 4];
        let cw = text_width(font, c.encode_utf8(&mut buf), size, track) + track;
        if w + cw + ell_w > max_w {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push_str(ell);
    out
}

/// Dessine un texte centré horizontalement avec interlettrage, et renvoie sa largeur.
fn draw_centered_tracked(
    canvas: &mut Canvas,
    cx: i32,
    y: i32,
    text: &str,
    color: u32,
    size: f32,
    track: i32,
) -> (i32, i32) {
    let width = tracked_width(text, size, track);
    let mut cur = cx - width / 2;
    let font = crate::renderer::get_title_font();
    let mut h = 0;
    for c in text.chars() {
        let mut buf = [0u8; 4];
        let (cw, ch) = canvas.draw_text_styled(font, cur, y, c.encode_utf8(&mut buf), color, size);
        cur += cw + track;
        h = h.max(ch);
    }
    (width, h)
}

/// Dessine un texte tronqué à partir de `x`, et renvoie sa largeur.
fn draw_truncated(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    text: &str,
    color: u32,
    size: f32,
    max_w: i32,
) -> i32 {
    let font = crate::renderer::get_body_font();
    let shown = truncate_text(font, text, size, max_w, 0);
    let width = text_width(font, &shown, size, 0);
    canvas.draw_text_styled(font, x, y, &shown, color, size);
    width
}

/// Hauteur d'une ligne, 0 si l'élément n'a rien à afficher.
fn overlay_row_height(el: &UiElement, ctx: &ExecutionContext) -> i32 {
    match el.kind.as_str() {
        "bar" => OVERLAY_BAR_H,
        "icon" => OVERLAY_ROW_H,
        _ => {
            if element_text(el, ctx).is_some() {
                OVERLAY_ROW_H
            } else {
                0
            }
        }
    }
}

/// Largeur naturelle du contenu d'une ligne (sans les marges de la carte).
fn overlay_row_width(el: &UiElement, ctx: &ExecutionContext) -> i32 {
    match el.kind.as_str() {
        "bar" => {
            let (cur, max) = bound_value_and_max(ctx, el.bind.as_deref());
            let value = format!("{cur:.0}/{max:.0}");
            let (vw, _) = Canvas::measure_text_styled(crate::renderer::get_title_font(), &value, 12.0);
            let label_w = el
                .label
                .as_deref()
                .map(|l| Canvas::measure_text_styled(crate::renderer::get_body_font(), l, 13.0).0)
                .unwrap_or(0);
            (BAR_W as i32 + 8 + vw).max(label_w)
        }
        "icon" => {
            let glyph = el.label.as_deref().unwrap_or("•");
            Canvas::measure_text_styled(crate::renderer::get_title_font(), glyph, OVERLAY_ICON_PT).0
        }
        _ => {
            let Some(line) = element_text(el, ctx) else {
                return 0;
            };
            if let Some((key, rest)) = split_command(&line) {
                let key_w = Canvas::measure_text_styled(
                    crate::renderer::get_title_font(),
                    key,
                    OVERLAY_KEY_PT,
                )
                .0;
                let sep_w =
                    Canvas::measure_text_styled(crate::renderer::get_body_font(), "—", OVERLAY_DESC_PT).0;
                let rest_w =
                    Canvas::measure_text_styled(crate::renderer::get_body_font(), rest, OVERLAY_DESC_PT).0;
                OVERLAY_CHIP_PAD * 2 + key_w + 12 + sep_w + 8 + rest_w
            } else {
                12
                    + Canvas::measure_text_styled(
                        crate::renderer::get_body_font(),
                        &line,
                        OVERLAY_DESC_PT,
                    )
                    .0
            }
        }
    }
}

/// Calcule la carte : largeur/hauteur sur le contenu, centrage dans le viewport.
fn overlay_layout(
    screen: &UiScreen,
    ctx: &ExecutionContext,
    vx: i32,
    vy: i32,
    vw: i32,
    vh: i32,
) -> OverlayLayout {
    let max_allowed = (vw - 2 * OVERLAY_MARGIN)
        .min(OVERLAY_MAX_W)
        .max(240);
    let min_allowed = OVERLAY_MIN_W.min(max_allowed);

    // Largeur naturelle : titre + chaque ligne (marges de ligne comprises).
    let mut content_w = 0;
    if let Some(title) = screen.title.as_deref().filter(|t| !t.is_empty()) {
        content_w = content_w.max(tracked_width(title, OVERLAY_TITLE_PT, OVERLAY_TITLE_TRACK));
    }
    for el in &screen.elements {
        let row_w = overlay_row_width(el, ctx);
        if row_w > 0 {
            content_w = content_w.max(row_w + 2 * OVERLAY_ROW_PAD);
        }
    }
    let w = (content_w + 2 * OVERLAY_PAD_X).clamp(min_allowed, max_allowed);
    let mut h: i32 = OVERLAY_PAD_Y;
    if screen.title.as_deref().map_or(false, |t| !t.is_empty()) {
        let title_h = Canvas::measure_text_styled(
            crate::renderer::get_title_font(),
            screen.title.as_deref().unwrap_or(""),
            OVERLAY_TITLE_PT,
        )
        .1;
        h += 2 + 14 + title_h.max(24) + 22 + 1 + 26;
    }
    let mut visible = 0;
    for el in &screen.elements {
        let rh = overlay_row_height(el, ctx);
        if rh > 0 {
            visible += 1;
            h += rh;
        }
    }
    if visible > 1 {
        h += (visible - 1) * OVERLAY_ROW_GAP;
    }
    h += OVERLAY_PAD_Y;

    let px = vx + (vw - w) / 2;
    let py = vy + (vh - h) / 2;
    OverlayLayout { px, py, w, h }
}

/// Ornement discret au-dessus du titre : trait fin + losange central.
fn draw_overlay_ornament(canvas: &mut Canvas, cx: i32, y: i32) {
    canvas.draw_rect(cx - 34, y + 2, 26, 1, MENU_ACCENT_DEEP);
    canvas.draw_rect(cx + 9, y + 2, 26, 1, MENU_ACCENT_DEEP);
    canvas.draw_line(cx, y, cx + 4, y + 3, MENU_ACCENT);
    canvas.draw_line(cx + 4, y + 3, cx, y + 6, MENU_ACCENT);
    canvas.draw_line(cx, y + 6, cx - 4, y + 3, MENU_ACCENT);
    canvas.draw_line(cx - 4, y + 3, cx, y, MENU_ACCENT);
}

/// Petit chevron décoratif à droite d'une ligne (affordance de commande).
fn draw_overlay_chevron(canvas: &mut Canvas, x: i32, y: i32) {
    canvas.draw_line(x, y, x + 4, y + 4, MENU_ACCENT_DEEP);
    canvas.draw_line(x + 4, y + 4, x, y + 8, MENU_ACCENT_DEEP);
}

/// Dessine la carte : cadre, en-tête (titre + filet) et lignes d'entrées.
fn draw_overlay_card(
    canvas: &mut Canvas,
    screen: &UiScreen,
    ctx: &ExecutionContext,
    layout: &OverlayLayout,
) {
    let OverlayLayout { px, py, w, h } = *layout;
    let cx = px + w / 2;
    let inner_w = w - 2 * OVERLAY_PAD_X;

    // Cadre « Dark Atelier » : fond panneau, filet d'accent supérieur, bordure,
    // cornières technologiques.
    canvas.draw_rect(px, py, w as u32, h as u32, MENU_BG);
    canvas.draw_rect(px, py, w as u32, 2, MENU_ACCENT);
    canvas.draw_rect_outline(px, py, w as u32, h as u32, MENU_BORDER, 1);
    canvas.draw_rect_outline(px + 1, py + 1, (w - 2) as u32, (h - 2) as u32, MENU_ACCENT_DEEP, 1);
    canvas.draw_corner_brackets(px, py, w as u32, h as u32, 16, MENU_ACCENT);

    let mut y = py + OVERLAY_PAD_Y;

    if let Some(title) = screen.title.as_deref().filter(|t| !t.is_empty()) {
        draw_overlay_ornament(canvas, cx, y);
        y += 2 + 14;

        let title_pt = {
            let natural = tracked_width(title, OVERLAY_TITLE_PT, OVERLAY_TITLE_TRACK);
            if natural > inner_w && natural > 0 {
                (OVERLAY_TITLE_PT * inner_w as f32 / natural as f32)
                    .clamp(18.0, OVERLAY_TITLE_PT)
            } else {
                OVERLAY_TITLE_PT
            }
        };
        let title = truncate_text(
            crate::renderer::get_title_font(),
            title,
            title_pt,
            inner_w,
            OVERLAY_TITLE_TRACK,
        );
        let title_h = Canvas::measure_text_styled(
            crate::renderer::get_title_font(),
            &title,
            title_pt,
        )
        .1;
        // Ombre portée d'un pixel : profondeur sans bavure.
        draw_centered_tracked(canvas, cx, y + 1, &title, MENU_ACCENT_DEEP, title_pt, OVERLAY_TITLE_TRACK);
        draw_centered_tracked(canvas, cx, y, &title, TEXT_MAIN, title_pt, OVERLAY_TITLE_TRACK);
        y += title_h.max(24) + 22;

        // Filet de séparation : ligne discrète + segment accent centré.
        canvas.draw_rect(px + OVERLAY_PAD_X, y, inner_w as u32, 1, MENU_BORDER);
        canvas.draw_rect(cx - 40, y, 80, 1, MENU_ACCENT_DEEP);
        y += 1 + 26;
    }

    let row_x = px + OVERLAY_PAD_X;
    let row_w = inner_w;
    let mut first = true;
    for el in &screen.elements {
        let rh = overlay_row_height(el, ctx);
        if rh <= 0 {
            continue;
        }
        if !first {
            y += OVERLAY_ROW_GAP;
        }
        first = false;
        draw_overlay_row(canvas, el, ctx, row_x, y, row_w, rh);
        y += rh;
    }
}

/// Dessine une ligne (texte/commande, bouton, icône ou jauge).
fn draw_overlay_row(
    canvas: &mut Canvas,
    el: &UiElement,
    ctx: &ExecutionContext,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let is_button = el.kind == "button";
    let row_bg = if is_button { 0x002E1065 } else { MENU_ROW_BG };
    let border = if is_button { MENU_ACCENT } else { MENU_BORDER };
    canvas.draw_rect(x, y, w as u32, h as u32, row_bg);
    canvas.draw_rect_outline(x, y, w as u32, h as u32, border, 1);
    let rail_h = (h - 18).max(4) as u32;
    canvas.draw_rect(x, y + 9, 3, rail_h, if is_button { MENU_ACCENT } else { MENU_ACCENT_DEEP });

    match el.kind.as_str() {
        "bar" => draw_overlay_bar(canvas, el, ctx, x, y, w, h),
        "icon" => {
            let glyph = el.label.as_deref().unwrap_or("•");
            let font = crate::renderer::get_title_font();
            let (gw, gh) = Canvas::measure_text_styled(font, glyph, OVERLAY_ICON_PT);
            canvas.draw_text_styled(
                font,
                x + OVERLAY_ROW_PAD,
                y + (h - gh) / 2,
                glyph,
                MENU_ACCENT_SOFT,
                OVERLAY_ICON_PT,
            );
            if w - OVERLAY_ROW_PAD * 2 > gw + OVERLAY_CHEVRON_GAP {
                draw_overlay_chevron(canvas, x + w - OVERLAY_ROW_PAD - 4, y + h / 2 - 4);
            }
        }
        _ => draw_overlay_command_row(canvas, el, ctx, x, y, w, h),
    }
}

/// Ligne de commande : pastille de touche en accent, action en texte secondaire.
fn draw_overlay_command_row(
    canvas: &mut Canvas,
    el: &UiElement,
    ctx: &ExecutionContext,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let Some(line) = element_text(el, ctx) else {
        return;
    };
    let ty = y + (h - 20) / 2;

    if let Some((key, rest)) = split_command(&line) {
        let key_font = crate::renderer::get_title_font();
        let max_key_w = (w - 2 * OVERLAY_ROW_PAD - 120).max(40);
        let key = truncate_text(key_font, key, OVERLAY_KEY_PT, max_key_w, 0);
        let (key_w, key_h) =
            Canvas::measure_text_styled(key_font, &key, OVERLAY_KEY_PT);
        let chip_w = key_w + OVERLAY_CHIP_PAD * 2;
        let chip_y = y + (h - OVERLAY_CHIP_H) / 2;
        canvas.draw_rect(x + OVERLAY_ROW_PAD, chip_y, chip_w as u32, OVERLAY_CHIP_H as u32, MENU_CHIP_BG);
        canvas.draw_rect_outline(
            x + OVERLAY_ROW_PAD,
            chip_y,
            chip_w as u32,
            OVERLAY_CHIP_H as u32,
            MENU_ACCENT_DEEP,
            1,
        );
        canvas.draw_text_styled(
            key_font,
            x + OVERLAY_ROW_PAD + OVERLAY_CHIP_PAD,
            chip_y + (OVERLAY_CHIP_H - key_h) / 2,
            &key,
            MENU_ACCENT_SOFT,
            OVERLAY_KEY_PT,
        );

        let body_font = crate::renderer::get_body_font();
        let sep_x = x + OVERLAY_ROW_PAD + chip_w + 12;
        let (sep_w, _sep_h) = Canvas::measure_text_styled(body_font, "—", OVERLAY_DESC_PT);
        canvas.draw_text_styled(body_font, sep_x, ty, "—", TEXT_MUTED, OVERLAY_DESC_PT);
        let rest_x = sep_x + sep_w + 8;
        let free = x + w - OVERLAY_ROW_PAD - OVERLAY_CHEVRON_GAP - rest_x;
        draw_truncated(canvas, rest_x, ty, rest, TEXT_MUTED, OVERLAY_DESC_PT, free);
        if free > 0 {
            draw_overlay_chevron(canvas, x + w - OVERLAY_ROW_PAD - 4, y + h / 2 - 4);
        }
    } else {
        // Texte simple : puce accent + libellé principal (lisible sur la carte).
        canvas.draw_rect(x + OVERLAY_ROW_PAD, y + h / 2 - 2, 4, 4, MENU_ACCENT);
        let free = x + w - OVERLAY_ROW_PAD - OVERLAY_CHEVRON_GAP - (x + OVERLAY_ROW_PAD + 12);
        draw_truncated(
            canvas,
            x + OVERLAY_ROW_PAD + 12,
            ty,
            &line,
            TEXT_MAIN,
            OVERLAY_DESC_PT,
            free,
        );
        if free > 0 {
            draw_overlay_chevron(canvas, x + w - OVERLAY_ROW_PAD - 4, y + h / 2 - 4);
        }
    }
}

/// Jauge d'écran plein : étiquette au-dessus, barre + valeur.
fn draw_overlay_bar(
    canvas: &mut Canvas,
    el: &UiElement,
    ctx: &ExecutionContext,
    x: i32,
    y: i32,
    _w: i32,
    _h: i32,
) {
    let body = crate::renderer::get_body_font();
    let title = crate::renderer::get_title_font();
    if let Some(label) = el.label.as_deref().filter(|l| !l.is_empty()) {
        canvas.draw_text_styled(body, x + OVERLAY_ROW_PAD, y + 7, label, TEXT_MUTED, 13.0);
    }
    let bar_x = x + OVERLAY_ROW_PAD;
    let bar_y = y + 27;
    canvas.draw_rect(bar_x, bar_y, BAR_W, 8, BAR_BG);
    let (cur, max) = bound_value_and_max(ctx, el.bind.as_deref());
    let ratio = if max > 0.0 { (cur / max).clamp(0.0, 1.0) } else { 0.0 };
    let fill_w = (BAR_W as f32 * ratio) as u32;
    if fill_w > 0 {
        let fill = if el.bind.as_deref().map_or(false, |b| b.contains("hp")) {
            BAR_FILL_HP
        } else {
            BAR_FILL
        };
        canvas.draw_rect(bar_x, bar_y, fill_w, 8, fill);
    }
    canvas.draw_rect_outline(bar_x, bar_y, BAR_W, 8, MENU_BORDER, 1);
    let value = format!("{cur:.0}/{max:.0}");
    canvas.draw_text_styled(title, bar_x + BAR_W as i32 + 10, bar_y - 4, &value, TEXT_MAIN, 12.0);
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

    fn fnv1a(bytes: &[u32]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for v in bytes {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }

    /// Luminance relative WCAG d'une couleur `0x00RRGGBB`.
    fn relative_luminance(color: u32) -> f64 {
        fn channel(c: u32) -> f64 {
            let s = c as f64 / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel((color >> 16) & 0xFF)
            + 0.7152 * channel((color >> 8) & 0xFF)
            + 0.0722 * channel(color & 0xFF)
    }

    /// Ratio de contraste WCAG entre deux couleurs opaques.
    fn contrast_ratio(a: u32, b: u32) -> f64 {
        let (la, lb) = (relative_luminance(a), relative_luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Bornes des pixels d'une couleur exacte (les glyphes pleins sont opaques).
    fn color_bounds(canvas: &Canvas, color: u32) -> Option<(i32, i32, i32, i32)> {
        let mut bounds: Option<(i32, i32, i32, i32)> = None;
        for (i, &p) in canvas.buffer.iter().enumerate() {
            if p != color {
                continue;
            }
            let x = (i % canvas.width) as i32;
            let y = (i / canvas.width) as i32;
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
        bounds
    }

    /// Couleurs de texte utilisées par la carte de menu (hors décors/bordure).
    fn overlay_text_colors() -> [u32; 3] {
        [TEXT_MAIN, TEXT_MUTED, MENU_ACCENT_SOFT]
    }

    fn assert_text_inside_card(canvas: &Canvas, layout: &OverlayLayout) {
        let (x0, y0) = (layout.px + 4, layout.py + 4);
        let (x1, y1) = (layout.px + layout.w - 5, layout.py + layout.h - 5);
        let mut seen = 0;
        for color in overlay_text_colors() {
            if let Some((bx0, by0, bx1, by1)) = color_bounds(canvas, color) {
                seen += 1;
                assert!(
                    bx0 >= x0 && by0 >= y0 && bx1 <= x1 && by1 <= y1,
                    "pixel de texte {color:08x} hors carte (marge 4 px) : \
                     ({bx0},{by0})-({bx1},{by1}) vs carte ({x0},{y0})-({x1},{y1})"
                );
            }
        }
        assert!(seen >= 2, "la carte doit contenir du texte principal et secondaire");
    }

    fn menu_test_screen() -> UiScreen {
        UiScreen {
            id: "menu_test".into(),
            kind: UiScreenKind::Menu,
            title: Some("TITRE DE TEST SUFFISAMMENT LONG".into()),
            elements: vec![
                UiElement {
                    id: "e0".into(),
                    kind: "text".into(),
                    label: Some("ENTRÉE — JOUER".into()),
                    bind: None,
                },
                UiElement {
                    id: "e1".into(),
                    kind: "text".into(),
                    label: Some("ÉCHAP — PAUSE EN JEU".into()),
                    bind: None,
                },
            ],
        }
    }

    #[test]
    fn overlay_menu_card_layout_and_contrast() {
        let screen = menu_test_screen();
        let ctx = ctx_with(&[]);
        let mut canvas = Canvas::new(1200, 800);
        canvas.clear(0x00080B14);
        let layout = overlay_layout(&screen, &ctx, 0, 0, 1200, 800);
        render_overlay(&mut canvas, &screen, &ctx, 0, 0, 1200, 800);

        // Carte centrée (tolérance 2 px) sur les deux axes, mesurée sur les pixels réels.
        let (bx0, by0, bx1, by1) =
            color_bounds(&canvas, MENU_BG).expect("le fond de carte doit être peint");
        let left_gap = bx0;
        let right_gap = 1199 - bx1;
        let top_gap = by0;
        let bottom_gap = 799 - by1;
        println!(
            "carte={}x{} px=({}, {}) gaps L/R={}/{} T/B={}/{}",
            layout.w, layout.h, layout.px, layout.py, left_gap, right_gap, top_gap, bottom_gap
        );
        assert!((left_gap - right_gap).abs() < 2, "carte non centrée horizontalement");
        assert!((top_gap - bottom_gap).abs() < 2, "carte non centrée verticalement");

        // La largeur suit le contenu : le titre de test dépasse la largeur minimale.
        assert!(layout.w > OVERLAY_MIN_W, "carte trop étroite : {}", layout.w);
        assert!(layout.h > 150, "carte trop plate : {}", layout.h);

        // Texte principal présent dans la zone du titre, plus grand qu'avant (30 pt).
        let title_zone_end = layout.py + layout.h / 3;
        let title_pixels = canvas
            .buffer
            .iter()
            .enumerate()
            .filter(|(i, &p)| p == TEXT_MAIN && ((i / canvas.width) as i32) < title_zone_end)
            .count();
        assert!(title_pixels > 100, "titre absent ou trop petit ({title_pixels} px)");

        // Contraste mesuré (WCAG) : jamais sous 4,5:1.
        let title_ratio = contrast_ratio(TEXT_MAIN, MENU_BG);
        let muted_ratio = contrast_ratio(TEXT_MUTED, MENU_ROW_BG);
        let key_ratio = contrast_ratio(MENU_ACCENT_SOFT, MENU_CHIP_BG);
        println!(
            "contrastes titre={title_ratio:.2} secondaire={muted_ratio:.2} touche={key_ratio:.2}"
        );
        assert!(title_ratio >= 4.5, "contraste du titre {title_ratio:.2} < 4,5");
        assert!(muted_ratio >= 4.5, "contraste du texte secondaire {muted_ratio:.2} < 4,5");
        assert!(key_ratio >= 4.5, "contraste de la touche {key_ratio:.2} < 4,5");

        // Aucun pixel de texte ne touche ni ne dépasse la bordure (marge ≥ 4 px).
        assert_text_inside_card(&canvas, &layout);

        // La carte tient dans le viewport.
        assert!(layout.px >= 0 && layout.py >= 0);
        assert!(layout.px + layout.w <= 1200 && layout.py + layout.h <= 800);
    }

    #[test]
    fn overlay_without_title_and_six_entries_fits() {
        let screen = UiScreen {
            id: "menu_six".into(),
            kind: UiScreenKind::Menu,
            title: None,
            elements: (0..6)
                .map(|i| UiElement {
                    id: format!("e{i}"),
                    kind: "text".into(),
                    label: Some(format!("TOUCHE {i} — ACTION {i}")),
                    bind: None,
                })
                .collect(),
        };
        let ctx = ctx_with(&[]);
        let mut canvas = Canvas::new(1024, 720);
        canvas.clear(0x00080B14);
        let layout = overlay_layout(&screen, &ctx, 0, 0, 1024, 720);
        render_overlay(&mut canvas, &screen, &ctx, 0, 0, 1024, 720);

        assert!(layout.px >= 0 && layout.py >= 0);
        assert!(
            layout.px + layout.w <= 1024 && layout.py + layout.h <= 720,
            "carte 6 entrées hors viewport : {}x{} @ ({}, {})",
            layout.w,
            layout.h,
            layout.px,
            layout.py
        );
        assert_text_inside_card(&canvas, &layout);
    }

    #[test]
    fn overlay_pause_defeat_victory_share_the_same_path() {
        let ctx = ctx_with(&[]);
        for (kind, title) in [
            (UiScreenKind::Pause, "PAUSE"),
            (UiScreenKind::GameOver, "DÉFAITE"),
            (UiScreenKind::Victory, "VICTOIRE"),
        ] {
            let screen = UiScreen {
                id: format!("{kind:?}"),
                kind,
                title: Some(title.into()),
                elements: vec![
                    UiElement {
                        id: "e0".into(),
                        kind: "text".into(),
                        label: Some("ENTRÉE — CONTINUER".into()),
                        bind: None,
                    },
                    UiElement {
                        id: "e1".into(),
                        kind: "text".into(),
                        label: Some("ÉCHAP — MENU".into()),
                        bind: None,
                    },
                ],
            };
            let mut canvas = Canvas::new(1024, 720);
            canvas.clear(0x00080B14);
            let layout = overlay_layout(&screen, &ctx, 0, 0, 1024, 720);
            render_overlay(&mut canvas, &screen, &ctx, 0, 0, 1024, 720);
            assert!(
                layout.px >= 0
                    && layout.py >= 0
                    && layout.px + layout.w <= 1024
                    && layout.py + layout.h <= 720,
                "{title} déborde du viewport"
            );
            assert_text_inside_card(&canvas, &layout);
        }
    }

    #[test]
    fn render_hud_panel_is_pixel_identical() {
        // Empreintes FNV-1a capturées AVANT le relookage des menus : le HUD emprunte
        // toujours `render_hud_panel` → `render_screen` sans passer par les helpers
        // plein écran. Toute modification du HUD casserait ces valeurs.
        let screen = hud_screen();
        let ctx = ctx_with(&[
            ("player_hp", Value::Int(2)),
            ("player_max_hp", Value::Int(3)),
            ("score", Value::Int(42)),
        ]);
        let mut small = Canvas::new(320, 200);
        small.clear(0x00080B14);
        render_hud_panel(&mut small, &screen, &ctx, 18, 12);
        assert_eq!(fnv1a(&small.buffer), 0x8aa7_82c8_2757_8a56);

        let mut big = Canvas::new(1200, 800);
        big.clear(0x00080B14);
        render_hud_panel(&mut big, &screen, &ctx, 18, 12);
        assert_eq!(fnv1a(&big.buffer), 0x6eda_033d_b52c_f0d6);
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
        assert!(
            canvas.buffer.iter().all(|&p| p != 0x00080B14),
            "le voile semi-transparent doit recouvrir tout l'écran"
        );
        assert!(canvas.buffer.iter().any(|&p| p == MENU_BG), "la carte doit être peinte");
        assert!(
            canvas.buffer.iter().any(|&p| p == TEXT_MAIN),
            "le texte de la carte doit être peint"
        );
    }

    #[test]
    fn overlay_veil_keeps_the_background_visible() {
        // Deux moitiés de décor distinctes : après le voile, la variation subsiste.
        let mut canvas = Canvas::new(200, 120);
        canvas.draw_rect(0, 0, 100, 120, 0x00FF0000);
        canvas.draw_rect(100, 0, 100, 120, 0x000000FF);
        draw_overlay_veil(&mut canvas, 0, 0, 200, 120);
        let left = canvas.buffer[60 * 200 + 50];
        let right = canvas.buffer[60 * 200 + 150];
        assert_ne!(left, right, "le voile ne doit pas opacifier le décor");
        assert_eq!(left >> 16 & 0xFF, 54, "voile trop clair ou trop sombre");
        assert_eq!(right & 0xFF, 60, "voile trop clair ou trop sombre");
    }
}
