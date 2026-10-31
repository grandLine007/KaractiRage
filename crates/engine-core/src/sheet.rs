//! Spritesheets & animations de frames (pxl).
//!
//! pxl écrit un atlas PNG accompagné d'un JSON de type TexturePacker/Aseprite
//! (`frames.<nom>.frame { x, y, w, h }`, `duration` en ms pour les frames d'une
//! animation, `meta.frameTags`). Chaque frame est **recadrée une fois à l'import**
//! via `RasterImage::crop`, puis blittée par `Canvas::draw_pixel_matrix_alpha`
//! (aucune allocation ni recadrage dans le hot path).

use std::collections::HashMap;
use std::sync::Arc;

use crate::vector::{decode_png, RasterImage, SvgError};

/// Une frame nommée d'une spritesheet, déjà recadrée et prémultipliée.
pub struct SheetFrame {
    pub name: String,
    /// Frame recadrée (prémultipliée `0xAARRGGBB`).
    pub image: RasterImage,
    /// Durée d'affichage en millisecondes (0 = durée par défaut de 100 ms).
    pub duration_ms: u32,
    /// Position d'origine dans l'atlas (pour reconstituer l'ordre de packing).
    pub source_x: i32,
    pub source_y: i32,
}

/// Un tag Aseprite : plage de frames nommée (`from..=to`).
#[derive(Debug, Clone)]
pub struct FrameTag {
    pub name: String,
    pub from: usize,
    pub to: usize,
}

/// Spritesheet / bande d'animation chargée.
pub struct SheetAsset {
    /// Atlas complet (pour référence / debug).
    pub image: RasterImage,
    /// Frames dans l'ordre de packing (haut→bas, gauche→droite).
    pub frames: Vec<SheetFrame>,
    by_name: HashMap<String, usize>,
    pub tags: Vec<FrameTag>,
}

impl SheetAsset {
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn frame(&self, name: &str) -> Option<&SheetFrame> {
        self.by_name.get(name).map(|&i| &self.frames[i])
    }

    pub fn frame_index(&self, index: usize) -> Option<&SheetFrame> {
        self.frames.get(index)
    }

    /// Durée d'une frame en secondes (repli 100 ms si non renseignée).
    pub fn frame_duration(&self, index: usize) -> f32 {
        match self.frames.get(index).map(|f| f.duration_ms) {
            Some(0) | None => 0.1,
            Some(ms) => ms as f32 / 1000.0,
        }
    }
}

/// État d'avancement d'une animation par frames (porté par `GameState`).
#[derive(Debug, Clone, Copy, Default)]
pub struct AnimState {
    pub frame: usize,
    pub acc: f32,
}


/// Cache de spritesheets décodées, indexé par identifiant (ex: "chars", "coin_spin").
#[derive(Default)]
pub struct SheetCache {
    sheets: HashMap<String, Arc<SheetAsset>>,
    /// Assets embarqués (export standalone / import de package) : id -> (PNG brut, JSON).
    embedded: HashMap<String, (Vec<u8>, String)>,
}

impl SheetCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Installe des assets embarqués, prioritaires sur le disque.
    pub fn install_embedded(&mut self, map: HashMap<String, (Vec<u8>, String)>) {
        self.embedded = map;
    }

    pub fn get_or_load(&mut self, id: &str) -> Result<Arc<SheetAsset>, SvgError> {
        if let Some(s) = self.sheets.get(id) {
            return Ok(s.clone());
        }
        let (png_bytes, json) = if let Some((png, json)) = self.embedded.get(id) {
            (png.clone(), json.clone())
        } else {
            load_sheet_source(id)?
        };
        let atlas = decode_png(&png_bytes)?;
        let asset = Arc::new(parse_sheet_json(&atlas, &json)?);
        self.sheets.insert(id.to_string(), asset.clone());
        Ok(asset)
    }

    /// Accès en lecture seule (rendu) : `None` si absent ou échoué.
    pub fn get(&self, id: &str) -> Option<&Arc<SheetAsset>> {
        self.sheets.get(id)
    }
}

/// Octets (PNG, JSON) d'une spritesheet identifiée, sans décodage.
pub fn read_sheet_source(id: &str) -> Option<(Vec<u8>, String)> {
    load_sheet_source(id).ok()
}

/// Résout l'atlas PNG + son JSON pour un identifiant (`"chars"`, `"coin_spin"`, ou
/// chemin relatif). Les sheets vivent dans `assets/generated/`, les anims dans
/// `assets/generated/anims/`.
fn load_sheet_source(id: &str) -> Result<(Vec<u8>, String), SvgError> {
    let bases: Vec<String> = if id.ends_with(".png") {
        vec![id.trim_end_matches(".png").to_string()]
    } else {
        vec![
            format!("assets/generated/{id}"),
            format!("assets/generated/anims/{id}"),
            id.to_string(),
        ]
    };

    let abs = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    for base in &bases {
        for prefix in ["", "../../", abs.as_str()] {
            let png_path = format!("{prefix}{base}.png");
            let json_path = format!("{prefix}{base}.json");
            if let (Ok(png), Ok(json)) = (std::fs::read(&png_path), std::fs::read_to_string(&json_path)) {
                return Ok((png, json));
            }
        }
    }
    Err(SvgError(format!("spritesheet introuvable: {id}")))
}

/// Parse le JSON pxl (TexturePacker/Aseprite) en `SheetAsset`, frames recadrées.
fn parse_sheet_json(atlas: &RasterImage, json: &str) -> Result<SheetAsset, SvgError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| SvgError(e.to_string()))?;
    let frames_obj = v
        .get("frames")
        .and_then(|f| f.as_object())
        .ok_or_else(|| SvgError("JSON de spritesheet sans clé 'frames'".into()))?;

    let mut frames: Vec<SheetFrame> = Vec::with_capacity(frames_obj.len());
    for (name, f) in frames_obj {
        let rect = f
            .get("frame")
            .ok_or_else(|| SvgError(format!("frame '{name}' sans 'frame'")))?;
        let x = rect.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
        let y = rect.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
        let w = rect.get("w").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
        let h = rect.get("h").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
        let duration_ms = f.get("duration").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let image = atlas.crop(x, y, x + w, y + h);
        frames.push(SheetFrame {
            name: name.clone(),
            image,
            duration_ms,
            source_x: x,
            source_y: y,
        });
    }

    // Reconstruit l'ordre d'animation indépendamment de l'ordre alphabétique des
    // clés JSON (shelf packing : haut→bas puis gauche→droite).
    frames.sort_by_key(|f| (f.source_y, f.source_x));

    let mut by_name = HashMap::with_capacity(frames.len());
    for (i, f) in frames.iter().enumerate() {
        by_name.insert(f.name.clone(), i);
    }

    let tags = v
        .get("meta")
        .and_then(|m| m.get("frameTags"))
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    Some(FrameTag {
                        name: t.get("name")?.as_str()?.to_string(),
                        from: t.get("from")?.as_u64()? as usize,
                        to: t.get("to")?.as_u64()? as usize,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(SheetAsset {
        image: atlas.clone(),
        frames,
        by_name,
        tags,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_cache_loads_chars_sheet() {
        let mut cache = SheetCache::new();
        let sheet = cache.get_or_load("chars").expect("chars.json/png généré par pxl");
        assert_eq!(sheet.frame_count(), 11);
        assert!(sheet.frame("coin").is_some());
        assert!(sheet.frame("boss").is_some());
        // Frame recadrée : coin = 18x16 (outline inclus).
        let coin = sheet.frame("coin").unwrap();
        assert_eq!((coin.image.width, coin.image.height), (18, 16));
    }

    #[test]
    fn sheet_cache_loads_coin_spin_anim() {
        let mut cache = SheetCache::new();
        let sheet = cache.get_or_load("coin_spin").expect("coin_spin anim généré");
        assert_eq!(sheet.frame_count(), 8);
        // Durées uniformes (fps 10 -> 100 ms).
        assert!((sheet.frame_duration(0) - 0.1).abs() < 0.001);
        // Tag unique couvrant toutes les frames.
        assert_eq!(sheet.tags.len(), 1);
        assert_eq!(sheet.tags[0].name, "coin_spin");
        assert_eq!((sheet.tags[0].from, sheet.tags[0].to), (0, 7));
    }
}
