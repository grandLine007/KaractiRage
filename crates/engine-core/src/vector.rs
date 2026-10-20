//! Intégration SVG : rastérisation à l'import via `usvg` + `resvg`/`tiny-skia`.
//!
//! Les images produites sont au format **RGBA prémultiplié** packé en `u32`
//! (`0xAARRGGBB`), directement compatible avec `Canvas::draw_pixel_matrix_alpha`.
//! Le pixel `0` est transparent. Aucune rasterisation n'a lieu dans le hot path :
//! le rendu ne fait que blitter des bitmaps mis en cache à l'import (zéro
//! allocation par frame).

use std::collections::HashMap;
use std::sync::Arc;

use resvg::{tiny_skia, usvg};

/// Image RGBA prémultiplié `0xAARRGGBB` (0 = transparent).
#[derive(Debug, Clone, PartialEq)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

impl RasterImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width as usize) * (height as usize)],
        }
    }

    /// Recadre la région `[x0, y0, x1, y1)` (bornes rognées aux limites de l'image).
    pub fn crop(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> RasterImage {
        let x0 = x0.max(0).min(self.width as i32);
        let y0 = y0.max(0).min(self.height as i32);
        let x1 = x1.max(x0).min(self.width as i32);
        let y1 = y1.max(y0).min(self.height as i32);
        let w = (x1 - x0) as u32;
        let h = (y1 - y0) as u32;
        let mut out = RasterImage::new(w, h);
        for sy in 0..h {
            for sx in 0..w {
                let src = ((y0 + sy as i32) as usize) * self.width as usize
                    + (x0 + sx as i32) as usize;
                out.pixels[(sy as usize) * (w as usize) + (sx as usize)] = self.pixels[src];
            }
        }
        out
    }
}

/// Erreur de rastérisation SVG (parse ou allocation).
#[derive(Debug, Clone)]
pub struct SvgError(pub String);

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SVG: {}", self.0)
    }
}

/// SVG résolu : bitmap prémultiplié complet (décor/fallback) et dimensions.
#[derive(Debug)]
pub struct LoadedSvg {
    pub width: u32,
    pub height: u32,
    pub image: Arc<RasterImage>,
    /// Rig squelettique extrait des groupes nommés (si ≥ 2 groupes).
    pub rig: Option<Arc<RigAsset>>,
}

/// Keyframe d'animation d'un membre du rig. Valeurs par défaut = identité.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct Keyframe {
    #[serde(default)]
    pub t: f32,
    #[serde(default)]
    pub rot: f32, // degrés autour du pivot
    #[serde(default)]
    pub tx: f32, // translation (unités SVG)
    #[serde(default)]
    pub ty: f32,
    #[serde(default = "default_one")]
    pub sx: f32, // échelle (1 = neutre)
    #[serde(default = "default_one")]
    pub sy: f32,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self {
            t: 0.0,
            rot: 0.0,
            tx: 0.0,
            ty: 0.0,
            sx: 1.0,
            sy: 1.0,
        }
    }
}

fn default_one() -> f32 {
    1.0
}

/// Un membre du rig : bitmap recadré + géométrie (dans l'espace SVG absolu).
#[derive(Debug, Clone, PartialEq)]
pub struct RigPart {
    pub id: String,
    /// Rectangle `(x0, y0, x1, y1)` en coordonnées SVG absolues.
    pub rect: (i32, i32, i32, i32),
    /// Pivot de rotation en coordonnées locales au bitmap (centre par défaut).
    pub pivot: (f32, f32),
    pub image: Arc<RasterImage>,
}

/// Piste d'animation d'un membre (keyframes triées par `t`).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimTrack {
    pub part: usize,
    pub keys: Vec<Keyframe>,
}

/// Clip d'animation (ex: "idle", "walk").
#[derive(Debug, Clone, PartialEq)]
pub struct AnimClip {
    pub name: String,
    pub looped: bool,
    pub duration: f32,
    pub tracks: Vec<AnimTrack>,
}

/// Rig squelettique complet : membres + clips, partagé entre entités.
#[derive(Debug, Clone, PartialEq)]
pub struct RigAsset {
    pub width: u32,
    pub height: u32,
    pub parts: Vec<RigPart>,
    pub clips: Vec<AnimClip>,
}

impl RigAsset {
    pub fn clip(&self, name: &str) -> Option<&AnimClip> {
        self.clips.iter().find(|c| c.name == name)
    }
}

impl AnimClip {
    /// Interpole la valeur d'un membre à l'instant `time` (bouclé sur `duration`).
    pub fn sample(&self, part: usize, time: f32) -> Keyframe {
        let Some(track) = self.tracks.iter().find(|t| t.part == part) else {
            return Keyframe::default();
        };
        if track.keys.is_empty() {
            return Keyframe::default();
        }
        if track.keys.len() == 1 {
            return track.keys[0];
        }
        let t = if self.duration > 0.0 {
            time % self.duration
        } else {
            time
        };
        let mut lo = 0usize;
        let mut hi = track.keys.len() - 1;
        for (i, k) in track.keys.iter().enumerate() {
            if k.t <= t {
                lo = i;
            } else {
                hi = i;
                break;
            }
        }
        if lo == hi {
            return track.keys[lo];
        }
        let k0 = track.keys[lo];
        let k1 = track.keys[hi];
        let span = k1.t - k0.t;
        let f = if span > 0.0 {
            ((t - k0.t) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Keyframe {
            t,
            rot: k0.rot + (k1.rot - k0.rot) * f,
            tx: k0.tx + (k1.tx - k0.tx) * f,
            ty: k0.ty + (k1.ty - k0.ty) * f,
            sx: k0.sx + (k1.sx - k0.sx) * f,
            sy: k0.sy + (k1.sy - k0.sy) * f,
        }
    }
}

/// Cache d'assets SVG rastérisés, indexé par identifiant (dédoublonnage entre entités).
#[derive(Default)]
pub struct SvgCache {
    assets: HashMap<String, Arc<LoadedSvg>>,
}

impl SvgCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Récupère un asset déjà rastérisé, ou le charge/rasterise à la demande.
    pub fn get_or_load(&mut self, id: &str) -> Result<Arc<LoadedSvg>, SvgError> {
        if let Some(loaded) = self.assets.get(id) {
            return Ok(loaded.clone());
        }
        let src = load_svg_source(id)?;
        let tree = parse_svg(&src)?;
        let image = rasterize_tree(&tree, None)?;
        let rig = extract_rig(&tree).map(|mut r| {
            r.clips = load_clips(id, &r.parts);
            Arc::new(r)
        });
        let loaded = LoadedSvg {
            width: image.width,
            height: image.height,
            image: Arc::new(image),
            rig,
        };
        let arc = Arc::new(loaded);
        self.assets.insert(id.to_string(), arc.clone());
        Ok(arc)
    }
}

/// Lit la source SVG d'un asset : disque (cwd racine ou `apps/studio`), puis repli embarqué.
fn load_svg_source(id: &str) -> Result<String, SvgError> {
    let rel = if id.ends_with(".svg") {
        id.to_string()
    } else {
        format!("assets/sprites/{id}.svg")
    };
    if let Ok(s) = std::fs::read_to_string(&rel) {
        return Ok(s);
    }
    if let Ok(s) = std::fs::read_to_string(format!("../../{rel}")) {
        return Ok(s);
    }
    // Repli embarqué (assets de démonstration compilés dans le binaire).
    let embedded = match id {
        "decor_torch" => Some(include_str!("../../../assets/sprites/decor_torch.svg")),
        "decor_crate" => Some(include_str!("../../../assets/sprites/decor_crate.svg")),
        "hero" => Some(include_str!("../../../assets/sprites/hero.svg")),
        _ => None,
    };
    embedded
        .map(str::to_string)
        .ok_or_else(|| SvgError(format!("asset introuvable: {id}")))
}

/// Rastérise un SVG en image prémultipliée. `target` = dimensions optionnelles,
/// sinon la taille native de l'image SVG.
pub fn rasterize_svg(src: &str, target: Option<(u32, u32)>) -> Result<RasterImage, SvgError> {
    let tree = parse_svg(src)?;
    rasterize_tree(&tree, target)
}

fn parse_svg(src: &str) -> Result<usvg::Tree, SvgError> {
    usvg::Tree::from_str(src, &usvg::Options::default()).map_err(|e| SvgError(e.to_string()))
}

fn rasterize_tree(tree: &usvg::Tree, target: Option<(u32, u32)>) -> Result<RasterImage, SvgError> {
    let size = tree.size();
    let native_w = size.width().ceil().max(1.0) as u32;
    let native_h = size.height().ceil().max(1.0) as u32;
    let (w, h) = match target {
        Some((tw, th)) if tw > 0 && th > 0 => (tw, th),
        _ => (native_w, native_h),
    };
    let (w, h) = clamp_dims(w, h);

    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| SvgError(format!("dimensions invalides {w}x{h}")))?;

    let sx = w as f32 / size.width();
    let sy = h as f32 / size.height();
    let mut pm = pixmap.as_mut();
    resvg::render(tree, tiny_skia::Transform::from_scale(sx, sy), &mut pm);

    Ok(pixmap_to_image(&pixmap))
}

/// Plafonne les dimensions pour respecter le budget mémoire Low-End (vision < 60 Mo).
fn clamp_dims(w: u32, h: u32) -> (u32, u32) {
    const MAX: u32 = 1024;
    if w <= MAX && h <= MAX {
        return (w, h);
    }
    let scale = (MAX as f32 / w.max(h) as f32).min(1.0);
    (
        (w as f32 * scale).max(1.0) as u32,
        (h as f32 * scale).max(1.0) as u32,
    )
}

/// Convertit un `Pixmap` tiny-skia (prémultiplié RGBA) en `RasterImage` 0xAARRGGBB.
fn pixmap_to_image(pixmap: &tiny_skia::Pixmap) -> RasterImage {
    let width = pixmap.width();
    let height = pixmap.height();
    let pixels = pixmap
        .pixels()
        .iter()
        .map(|c| {
            ((c.alpha() as u32) << 24)
                | ((c.red() as u32) << 16)
                | ((c.green() as u32) << 8)
                | (c.blue() as u32)
        })
        .collect();
    RasterImage {
        width,
        height,
        pixels,
    }
}

/// Extrait un rig squelettique depuis les groupes nommés **de premier niveau**.
///
/// Chaque `<g id="...">` enfant direct de la racine devient un membre rastérisé
/// isolément. Les groupes imbriqués ne sont pas encore supportés (limite v1).
fn extract_rig(tree: &usvg::Tree) -> Option<RigAsset> {
    let size = tree.size();
    let w = size.width().ceil().max(1.0) as u32;
    let h = size.height().ceil().max(1.0) as u32;
    let mut parts = Vec::new();
    for node in tree.root().children() {
        if let usvg::Node::Group(g) = node {
            if g.id().is_empty() {
                continue;
            }
            if let Some(part) = rasterize_part(node) {
                parts.push(part);
            }
        }
    }
    if parts.len() < 2 {
        return None;
    }
    Some(RigAsset {
        width: w,
        height: h,
        parts,
        clips: Vec::new(),
    })
}

fn rasterize_part(node: &usvg::Node) -> Option<RigPart> {
    let bbox = node.abs_layer_bounding_box()?;
    let bw = bbox.width().ceil().max(1.0) as u32;
    let bh = bbox.height().ceil().max(1.0) as u32;
    let x0 = bbox.x().floor() as i32;
    let y0 = bbox.y().floor() as i32;

    // `render_node` applique `pre_translate(-bbox.x, -bbox.y)` : le contenu est
    // donc calé en (0,0) dans un pixmap de la taille exacte du bbox.
    let mut pm = tiny_skia::Pixmap::new(bw, bh)?;
    let mut pmm = pm.as_mut();
    resvg::render_node(node, tiny_skia::Transform::identity(), &mut pmm)?;

    let img = pixmap_to_image(&pm);
    let pivot = (bw as f32 * 0.5, bh as f32 * 0.5);
    Some(RigPart {
        id: node.id().to_string(),
        rect: (x0, y0, x0 + bw as i32, y0 + bh as i32),
        pivot,
        image: Arc::new(img),
    })
}

/// Charge les clips depuis `assets/sprites/{id}.anim.json`, sinon génère des clips par défaut.
fn load_clips(id: &str, parts: &[RigPart]) -> Vec<AnimClip> {
    let rel = id.trim_end_matches(".svg");
    let json_path = format!("assets/sprites/{rel}.anim.json");
    for path in [json_path.as_str(), &format!("../../{json_path}")] {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(clips) = clips_from_json(&content, parts) {
                if !clips.is_empty() {
                    return clips;
                }
            }
        }
    }
    default_clips(parts)
}

fn clips_from_json(content: &str, parts: &[RigPart]) -> Result<Vec<AnimClip>, SvgError> {
    #[derive(serde::Deserialize)]
    struct ClipFile {
        #[serde(default = "default_true")]
        #[serde(rename = "loop")]
        looped: bool,
        #[serde(default)]
        tracks: HashMap<String, Vec<Keyframe>>,
    }
    #[derive(serde::Deserialize)]
    struct AnimFile {
        #[serde(default)]
        clips: HashMap<String, ClipFile>,
    }

    let file: AnimFile = serde_json::from_str(content).map_err(|e| SvgError(e.to_string()))?;
    let mut out = Vec::new();
    for (name, cf) in file.clips {
        let mut tracks = Vec::new();
        for (part_id, mut keys) in cf.tracks {
            if let Some(idx) = parts.iter().position(|p| p.id == part_id) {
                keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
                tracks.push(AnimTrack { part: idx, keys });
            }
        }
        let duration = tracks
            .iter()
            .flat_map(|t| t.keys.iter())
            .map(|k| k.t)
            .fold(0.0f32, f32::max);
        out.push(AnimClip {
            name,
            looped: cf.looped,
            duration,
            tracks,
        });
    }
    Ok(out)
}

fn default_true() -> bool {
    true
}

/// Génère des clips par défaut (idle + walk) basés sur les identifiants des membres.
fn default_clips(parts: &[RigPart]) -> Vec<AnimClip> {
    let find = |id: &str| parts.iter().position(|p| p.id == id);
    let mut clips = Vec::new();

    if let (Some(al), Some(ar), Some(ll), Some(lr)) = (
        find("arm_l"),
        find("arm_r"),
        find("leg_l"),
        find("leg_r"),
    ) {
        clips.push(AnimClip {
            name: "idle".into(),
            looped: true,
            duration: 1.0,
            tracks: vec![
                AnimTrack {
                    part: al,
                    keys: vec![
                        Keyframe { t: 0.0, rot: -6.0, ..Default::default() },
                        Keyframe { t: 0.5, rot: 6.0, ..Default::default() },
                        Keyframe { t: 1.0, rot: -6.0, ..Default::default() },
                    ],
                },
                AnimTrack {
                    part: ar,
                    keys: vec![
                        Keyframe { t: 0.0, rot: 6.0, ..Default::default() },
                        Keyframe { t: 0.5, rot: -6.0, ..Default::default() },
                        Keyframe { t: 1.0, rot: 6.0, ..Default::default() },
                    ],
                },
                AnimTrack {
                    part: ll,
                    keys: vec![
                        Keyframe { t: 0.0, rot: 4.0, ..Default::default() },
                        Keyframe { t: 0.5, rot: -4.0, ..Default::default() },
                        Keyframe { t: 1.0, rot: 4.0, ..Default::default() },
                    ],
                },
                AnimTrack {
                    part: lr,
                    keys: vec![
                        Keyframe { t: 0.0, rot: -4.0, ..Default::default() },
                        Keyframe { t: 0.5, rot: 4.0, ..Default::default() },
                        Keyframe { t: 1.0, rot: -4.0, ..Default::default() },
                    ],
                },
            ],
        });
    }

    if let (Some(ll), Some(lr)) = (find("leg_l"), find("leg_r")) {
        clips.push(AnimClip {
            name: "walk".into(),
            looped: true,
            duration: 0.8,
            tracks: vec![
                AnimTrack {
                    part: ll,
                    keys: vec![
                        Keyframe { t: 0.0, rot: 20.0, ..Default::default() },
                        Keyframe { t: 0.4, rot: -20.0, ..Default::default() },
                        Keyframe { t: 0.8, rot: 20.0, ..Default::default() },
                    ],
                },
                AnimTrack {
                    part: lr,
                    keys: vec![
                        Keyframe { t: 0.0, rot: -20.0, ..Default::default() },
                        Keyframe { t: 0.4, rot: 20.0, ..Default::default() },
                        Keyframe { t: 0.8, rot: -20.0, ..Default::default() },
                    ],
                },
            ],
        });
    }

    clips
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="4" y="4" width="8" height="8" fill="#ff0000"/></svg>"##;

    #[test]
    fn rasterize_produces_opaque_pixels() {
        let img = rasterize_svg(SQUARE, Some((16, 16))).unwrap();
        assert_eq!((img.width, img.height), (16, 16));
        // Centre : rouge opaque
        let center = img.pixels[(8 * 16 + 8) as usize];
        assert_ne!(center, 0);
        assert_eq!(center >> 24, 0xFF, "alpha plein attendu");
        assert_eq!((center >> 16) & 0xFF, 0xFF, "canal rouge attendu");
        // Coin (hors du rect) : transparent
        assert_eq!(img.pixels[0], 0);
    }

    #[test]
    fn rasterize_is_deterministic() {
        let a = rasterize_svg(SQUARE, None).unwrap();
        let b = rasterize_svg(SQUARE, None).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    #[test]
    fn invalid_svg_is_an_error() {
        assert!(rasterize_svg("<svg><", None).is_err());
    }

    #[test]
    fn crop_extracts_region() {
        let img = rasterize_svg(SQUARE, Some((16, 16))).unwrap();
        let cropped = img.crop(4, 4, 12, 12);
        assert_eq!((cropped.width, cropped.height), (8, 8));
        assert_ne!(cropped.pixels[(4 * 8 + 4) as usize], 0);
    }

    #[test]
    fn cache_dedups_by_id() {
        let mut cache = SvgCache::new();
        let a = cache.get_or_load("decor_torch").unwrap();
        let b = cache.get_or_load("decor_torch").unwrap();
        assert!(Arc::ptr_eq(&a, &b), "même Arc pour le même id");
        assert!(a.width > 0 && a.height > 0);
    }

    #[test]
    fn extract_rig_finds_top_level_groups() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">
  <g id="a"><rect x="0" y="0" width="8" height="8" fill="#ff0000"/></g>
  <g id="b"><rect x="16" y="0" width="8" height="8" fill="#00ff00"/></g>
</svg>"##;
        let tree = parse_svg(svg).unwrap();
        let rig = extract_rig(&tree).expect("rig extrait");
        assert_eq!(rig.parts.len(), 2);
        let ids: Vec<&str> = rig.parts.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"a") && ids.contains(&"b"));
        // Les bitmaps de membres ne doivent pas être vides
        for p in &rig.parts {
            assert!(p.image.pixels.iter().any(|&px| px != 0));
        }
    }

    #[test]
    fn extract_rig_ignores_unnamed_or_single_group() {
        // Un seul groupe nommé → pas de rig
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
  <g id="a"><rect x="0" y="0" width="8" height="8" fill="#ff0000"/></g>
</svg>"##;
        let tree = parse_svg(svg).unwrap();
        assert!(extract_rig(&tree).is_none());
    }

    #[test]
    fn anim_clip_sample_interpolates_and_loops() {
        let clip = AnimClip {
            name: "idle".into(),
            looped: true,
            duration: 1.0,
            tracks: vec![AnimTrack {
                part: 0,
                keys: vec![
                    Keyframe { t: 0.0, rot: 0.0, ..Default::default() },
                    Keyframe { t: 1.0, rot: 90.0, ..Default::default() },
                ],
            }],
        };
        assert_eq!(clip.sample(0, 0.5).rot, 45.0);
        assert_eq!(clip.sample(0, 1.5).rot, 45.0, "bouclage sur la durée");
        assert_eq!(clip.sample(99, 0.5).rot, 0.0, "part sans piste = identité");
        assert_eq!(clip.sample(0, 0.0).sx, 1.0, "échelle par défaut neutre");
    }
}

