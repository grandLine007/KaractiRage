use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rotation: f32,
    /// Coordonnée X de la frame précédente pour interpolation de rendu variable (alpha lerp)
    #[serde(default)]
    pub prev_x: f32,
    /// Coordonnée Y de la frame précédente pour interpolation de rendu variable (alpha lerp)
    #[serde(default)]
    pub prev_y: f32,
    /// Identifiant de l'entité parente pour le graphe de scène hiérarchique
    #[serde(default)]
    pub parent_id: Option<usize>,
    /// Décalage X relatif à l'entité parente
    #[serde(default)]
    pub local_offset_x: f32,
    /// Décalage Y relatif à l'entité parente
    #[serde(default)]
    pub local_offset_y: f32,
}

impl Transform2D {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            rotation: 0.0,
            prev_x: x,
            prev_y: y,
            parent_id: None,
            local_offset_x: 0.0,
            local_offset_y: 0.0,
        }
    }

    #[inline]
    pub fn center_x(&self) -> f32 {
        self.x + self.w * 0.5
    }

    #[inline]
    pub fn center_y(&self) -> f32 {
        self.y + self.h * 0.5
    }

    /// Enregistre la position actuelle comme position précédente avant le tick de simulation
    #[inline]
    pub fn snapshot_prev(&mut self) {
        self.prev_x = self.x;
        self.prev_y = self.y;
    }

    /// Calcule la position X interpolée pour le rendu fluide à taux de rafraîchissement élevé
    #[inline]
    #[allow(dead_code)]
    pub fn interpolated_x(&self, alpha: f32) -> f32 {
        let a = alpha.clamp(0.0, 1.0);
        self.prev_x + (self.x - self.prev_x) * a
    }

    /// Calcule la position Y interpolée pour le rendu fluide à taux de rafraîchissement élevé
    #[inline]
    #[allow(dead_code)]
    pub fn interpolated_y(&self, alpha: f32) -> f32 {
        let a = alpha.clamp(0.0, 1.0);
        self.prev_y + (self.y - self.prev_y) * a
    }

    /// Attache cette entité à une entité parente avec un décalage spatial relatif
    #[allow(dead_code)]
    pub fn attach_to(&mut self, parent_id: usize, offset_x: f32, offset_y: f32) {
        self.parent_id = Some(parent_id);
        self.local_offset_x = offset_x;
        self.local_offset_y = offset_y;
    }

    /// Détache cette entité de son parent
    #[allow(dead_code)]
    pub fn detach(&mut self) {
        self.parent_id = None;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rect,
    Circle { radius: f32 },
    Text { label: String, scale: u32 },
    PixelMatrix { width: u32, height: u32, pixels: Vec<u32> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Renderable2D {
    pub shape: ShapeKind,
    pub color: u32,
    pub outline_color: Option<u32>,
    pub outline_width: u32,
    pub z_index: i32,
    pub visible: bool,
}

impl Renderable2D {
    pub fn rect(color: u32, z_index: i32) -> Self {
        Self {
            shape: ShapeKind::Rect,
            color,
            outline_color: None,
            outline_width: 0,
            z_index,
            visible: true,
        }
    }

    pub fn circle(radius: f32, color: u32, z_index: i32) -> Self {
        Self {
            shape: ShapeKind::Circle { radius },
            color,
            outline_color: None,
            outline_width: 0,
            z_index,
            visible: true,
        }
    }

    pub fn pixel_matrix(width: u32, height: u32, pixels: Vec<u32>, z_index: i32) -> Self {
        Self {
            shape: ShapeKind::PixelMatrix { width, height, pixels },
            color: 0x00FFFFFF,
            outline_color: None,
            outline_width: 0,
            z_index,
            visible: true,
        }
    }

    /// Construit un sprite pixel-art à partir d'une grille ASCII et d'une palette de couleurs (0x00000000 = transparent)
    pub fn from_ascii_grid(rows: &[&str], palette: &std::collections::HashMap<char, u32>, z_index: i32) -> Self {
        let height = rows.len() as u32;
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u32;
        let mut pixels = Vec::with_capacity((width * height) as usize);

        for row in rows {
            let mut count = 0;
            for ch in row.chars() {
                if count < width {
                    let color = palette.get(&ch).copied().unwrap_or(0x00000000);
                    pixels.push(color);
                    count += 1;
                }
            }
            while count < width {
                pixels.push(0x00000000);
                count += 1;
            }
        }

        Self::pixel_matrix(width, height, pixels, z_index)
    }

    /// Bibliothèque procédurale de sprites 16x16 pixel-art rétro haute fidélité pour KaractiRage
    pub fn procedural_sprite(archetype: &str, accent_color: u32, z_index: i32) -> Self {
        let mut pal = std::collections::HashMap::new();
        pal.insert('.', 0x00000000); // Transparent
        pal.insert('X', 0x000F172A); // Contour sombre / bordure
        pal.insert('A', accent_color); // Couleur d'accentuation principale
        pal.insert('W', 0x00FFFFFF); // Blanc éclat / reflet
        pal.insert('Y', 0x00FBBF24); // Or / Jaune vif
        pal.insert('R', 0x00EF4444); // Rouge rubis
        pal.insert('G', 0x0010B981); // Vert émeraude
        pal.insert('B', 0x0038BDF8); // Cyan / Bleu ciel
        pal.insert('D', 0x00334155); // Gris métal sombre
        pal.insert('L', 0x0094A3B8); // Gris métal clair
        pal.insert('P', 0x00A855F7); // Violet mystique
        pal.insert('O', 0x00F97316); // Orange magma

        let rows: &[&str] = match archetype.to_lowercase().as_str() {
            "ninja" | "player_ninja" => &[
                "....XXXXXXXX....",
                "...XAAAAAAAAX...",
                "..XAAAAAAAAAAX..",
                "..XAAWWXAAWWAAX.",
                "..XAAXXXAAXXXAX.",
                "..XAAAAAAAAAAX..",
                "...XXXXXXXXXX...",
                "....XAAAAAAX....",
                "...XAAAYYAAAX...",
                "..XAAAAXXAAAAX..",
                "..XAAAX..XAAAX..",
                "..XAAAX..XAAAX..",
                "..XXXXX..XXXXX..",
                "..XDDDX..XDDDX..",
                ".XDDDDX..XDDDDX.",
                "................",
            ],
            "knight" | "player_knight" | "warrior" => &[
                "....XLLLLLLX....",
                "...XLLLLLLLLX...",
                "..XLLLLYYYYLLX..",
                "..XLLWWXLLWWLX..",
                "..XLLXXXLLXXLX..",
                "..XLLLLLLLLLLX..",
                "...XLLLLLLLLX...",
                "....XAAAAAAX....",
                "...XAAAAAAAAX...",
                "..XAAAWWWAAAAX..",
                "..XAAAXLLXAAAX..",
                "..XLLX....XLLX..",
                "..XLLX....XLLX..",
                "..XDDX....XDDX..",
                ".XDDDX....XDDDX.",
                "................",
            ],
            "drone" | "enemy_drone" | "cyber_drone" => &[
                ".....XXXXXX.....",
                "...XXDDDDDXX...",
                "..XDDDLLLLDDX..",
                ".XDDLLWXXWLLDX.",
                ".XDLLXXAAXXLLX.",
                "XDLLXAAAAAAXLX.",
                "XDLLXAAAAAAXLX.",
                ".XDLLXXAAXXLLX.",
                ".XDDLLWXXWLLDX.",
                "..XDDDLLLLDDX..",
                "...XXDDDDDXX...",
                ".....XOOOOX.....",
                "....XOOYYOOX....",
                ".....XOOOOX.....",
                "......XYYX......",
                ".......XX.......",
            ],
            "slime" | "enemy_slime" => &[
                "................",
                "................",
                "......XXXX......",
                "....XXGGGGXX....",
                "...XGGGGGGGGX...",
                "..XGGWWGGWWGGX..",
                ".XGGWXXGGWXXGGX.",
                ".XGGGGGGGGGGGGX.",
                ".XGGAAAGGGAAAGX.",
                "XGGGAAGGGGAAAGX.",
                "XGGGGGGGGGGGGGX.",
                "XGGGGGGGGGGGGGX.",
                "XGGGGGGGGGGGGGX.",
                ".XXXXXXXXXXXXX..",
                "................",
                "................",
            ],
            "skull" | "skeleton" | "enemy_skull" => &[
                "....XXXXXXXX....",
                "..XXWWWWWWWWXX..",
                ".XWWWWWWWWWWWWX.",
                "XWWWWWWWWWWWWWWX",
                "XWWWWXXWWWWXXWWX",
                "XWWXXDDXXWWXXDDX",
                "XWWXXDDXXWWXXDDX",
                "XWWWWXXWWWWXXWWX",
                "XWWWWWWXXXXWWWWX",
                ".XWWWWWWWWWWWWX.",
                "..XXWXXWWXXWXX..",
                "...XWXXWWXXWX...",
                "...XWXXWWXXWX...",
                "....XXXXXXXX....",
                "................",
                "................",
            ],
            "boss" | "titan" | "golem" => &[
                "..XXXXXXXXXXXX..",
                ".XPPPPPPPPPPPPX.",
                "XPPXXPPPPPPXXPPX",
                "XPXWWXPPPPXWWXPX",
                "XPXXXPPPPPPXXXPX",
                "XPPPPPPPPPPPPPPX",
                "XPPPPPOOOOPPPPPX",
                ".XPPPOOYYOOPPPX.",
                ".XPPPOOYYOOPPPX.",
                "..XPPOOOOPPXX...",
                "..XDDDDDDDDDDX..",
                ".XDDWWDDDDWWDDX.",
                ".XDDDDDDDDDDDDX.",
                ".XDDXDDDDDDXDDX.",
                "..XXXDDDDDDXXX..",
                "....XXXXXXXX....",
            ],
            "chest" | "item_chest" | "treasure" => &[
                "................",
                "...XXXXXXXXXX...",
                "..XYYYYYYYYYYX..",
                ".XYOOOOOOOOOOYX.",
                ".XYOXXXXXXXXOYX.",
                ".XYOODDDDDDOOYX.",
                "XXXXXXXXXXXXXX..",
                "XDDDDDDDDDDDDX..",
                "XDDDDYYYYDDDDX..",
                "XDDDDYXXYDDDDX..",
                "XDDDDYYYYDDDDX..",
                "XDDDDDDDDDDDDX..",
                "XYYYYDDDDDDYYYYX",
                "XXXXXXXXXXXXXX..",
                "................",
                "................",
            ],
            "key" | "item_key" => &[
                "................",
                ".....YYYYY......",
                "....YYWWWWYY....",
                "...YYWXXXXWYY...",
                "...YYWXXXXWYY...",
                "....YYWWWWYY....",
                ".....YYYYY......",
                "......YYY.......",
                "......YYY.......",
                "......YYYYYY....",
                "......YYY.......",
                "......YYYYYY....",
                "......YYYY......",
                "......YYY.......",
                "......YY........",
                "................",
            ],
            "potion" | "item_potion" | "heal" => &[
                "......XXXX......",
                "......XDDX......",
                ".....XDDDDX.....",
                "....XWWWWWWX....",
                "...XWWXXXXWWX...",
                "..XWWXRRRRXWWX..",
                ".XWWXRRRRRRXWWX.",
                ".XWXRRRRRRRRXWX.",
                ".XWXRRWWRRRRXWX.",
                ".XWXRRWWRRRRXWX.",
                ".XWXRRRRRRRRXWX.",
                ".XWXRRRRRRRRXWX.",
                "..XWXXXXXXXXWX..",
                "...XXXXXXXXXX...",
                "................",
                "................",
            ],
            "door" | "portal" => &[
                "...XXXXXXXXXX...",
                "..XPPPPPPPPPPX..",
                ".XPWWPPPPPPWWPX.",
                ".XPWPBBBBBBWPXP.",
                "XPWPBBWWWWBBWPXP",
                "XPWPBBWXXWBBWPXP",
                "XPWPBBWXXWBBWPXP",
                "XPWPBBWWWWBBWPXP",
                "XPWPBBBBBBBBWPXP",
                "XPWPBBBBBBBBWPXP",
                "XPWPBBBBBBBBWPXP",
                "XPWPBBBBBBBBWPXP",
                ".XPWPBBBBBBWPXP.",
                ".XPWWPPPPPPWWPX.",
                "..XPPPPPPPPPPX..",
                "...XXXXXXXXXX...",
            ],
            "coin" | "item_coin" | "gem" => &[
                ".....XXXXXX.....",
                "...XXYYYYYYXX...",
                "..XYYWWWWWWYYX..",
                ".XYWWYYYYYYWWYX.",
                ".XYWYYYYYYYYWYX.",
                "XYWYYYXXXXYYYWYY",
                "XYWYYXXXXXXYYWYY",
                "XYWYYXXXXXXYYWYY",
                "XYWYYYXXXXYYYWYY",
                ".XYWYYYYYYYYWYX.",
                ".XYWWYYYYYYWWYX.",
                "..XYYWWWWWWYYX..",
                "...XXYYYYYYXX...",
                ".....XXXXXX.....",
                "................",
                "................",
            ],
            "spikes" | "hazard_spikes" => &[
                "................",
                "................",
                ".......X........",
                "......XWX.......",
                ".....XWWX.......",
                ".....XWLX...X...",
                "....XWLLX..XWX..",
                "...XWLLLX.XWWX..",
                "..XWLLLLXXWLLX..",
                ".XWLLLLLWLLLLX..",
                "XWLLLLLLLLLLLLX.",
                "XDDDDDDDDDDDDDDX",
                "XDDDDDDDDDDDDDDX",
                "XRRRRRRRRRRRRRRX",
                "XXXXXXXXXXXXXXXX",
                "................",
            ],
            _ => &[
                "XXXXXXXXXXXXXXXX",
                "XAAAAAAAAAAAAAAX",
                "XAWWWWWWWWWWWWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWAAAAAAAAAAWAX",
                "XAWWWWWWWWWWWWAX",
                "XAAAAAAAAAAAAAAX",
                "XXXXXXXXXXXXXXXX",
                "................",
            ],
        };

        Self::from_ascii_grid(rows, &pal, z_index)
    }

    pub fn with_outline(mut self, color: u32, width: u32) -> Self {
        self.outline_color = Some(color);
        self.outline_width = width;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Collider2D {
    pub offset_x: f32,
    pub offset_y: f32,
    pub w: f32,
    pub h: f32,
    pub is_solid: bool,
    pub is_trigger: bool,
    #[serde(default)]
    pub is_pushable: bool,
    pub layer: u32,
}

impl Collider2D {
    pub fn solid(w: f32, h: f32) -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            w,
            h,
            is_solid: true,
            is_trigger: false,
            is_pushable: false,
            layer: 1,
        }
    }

    pub fn trigger(w: f32, h: f32) -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            w,
            h,
            is_solid: false,
            is_trigger: true,
            is_pushable: false,
            layer: 2,
        }
    }

    /// Bloc poussable pour mécaniques de puzzles, Sokoban et obstacles dynamiques
    pub fn pushable(w: f32, h: f32) -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            w,
            h,
            is_solid: true,
            is_trigger: false,
            is_pushable: true,
            layer: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PatrolConfig {
    pub min_y: f32,
    pub max_y: f32,
    pub dir: f32,
}

/// Modes de déplacement et de physique multi-genres du moteur
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MovementMode {
    /// 1. Top-Down / Aventure / RPG (déplacement libre 4/8 directions sans gravité)
    TopDown,
    /// 2. Platformer 2D (Gravité continue, saut vertical impulsionnel et détection d'appui au sol)
    Platformer {
        gravity: f32,
        jump_force: f32,
        is_grounded: bool,
        max_fall_speed: f32,
    },
    /// 3. Vaisseau Spatial / Asteroids / Physique Newtonienne (rotation angulaire + poussée vectorielle + friction)
    SpaceInertia {
        angular_speed: f32,
        thrust: f32,
        drag: f32,
    },
    /// 4. Déplacement par Grille / Sokoban / Casse-tête / Tactique (déplacement pas-à-pas quantifié)
    Grid {
        cell_size: f32,
        cooldown: f32,
        timer: f32,
    },
}

impl Default for MovementMode {
    fn default() -> Self {
        Self::TopDown
    }
}

impl MovementMode {
    /// Libellé court et pédagogique du modèle physique (affiché dans le Studio et le HUD)
    pub fn label(&self) -> &'static str {
        match self {
            Self::TopDown => "TOP-DOWN",
            Self::Platformer { .. } => "PLATEFORME",
            Self::SpaceInertia { .. } => "INERTIE SPATIALE",
            Self::Grid { .. } => "GRILLE",
        }
    }

    /// Modèle physique suivant dans le cycle d'édition, avec des réglages par défaut sûrs
    pub fn next_preset(&self) -> Self {
        match self {
            Self::TopDown => Self::Platformer {
                gravity: 920.0,
                jump_force: 480.0,
                is_grounded: false,
                max_fall_speed: 650.0,
            },
            Self::Platformer { .. } => Self::SpaceInertia {
                angular_speed: 4.0,
                thrust: 520.0,
                drag: 0.985,
            },
            Self::SpaceInertia { .. } => Self::Grid {
                cell_size: 40.0,
                cooldown: 0.12,
                timer: 0.0,
            },
            Self::Grid { .. } => Self::TopDown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Movement2D {
    pub vx: f32,
    pub vy: f32,
    pub speed: f32,
    pub is_player_controlled: bool,
    pub patrol: Option<PatrolConfig>,
    #[serde(default)]
    pub mode: MovementMode,
}

impl Movement2D {
    pub fn new(speed: f32) -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            speed,
            is_player_controlled: false,
            patrol: None,
            mode: MovementMode::TopDown,
        }
    }

    pub fn player(speed: f32) -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            speed,
            is_player_controlled: true,
            patrol: None,
            mode: MovementMode::TopDown,
        }
    }

    /// Contrôleur de Plateforme 2D (Side-Scroller)
    #[allow(dead_code)]
    pub fn platformer(speed: f32, jump_force: f32, gravity: f32) -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            speed,
            is_player_controlled: true,
            patrol: None,
            mode: MovementMode::Platformer {
                gravity,
                jump_force,
                is_grounded: false,
                max_fall_speed: 680.0,
            },
        }
    }

    /// Contrôleur de Vaisseau Spatial / Shmup (Inertie vectorielle)
    #[allow(dead_code)]
    pub fn spaceship(speed: f32, angular_speed: f32, thrust: f32) -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            speed,
            is_player_controlled: true,
            patrol: None,
            mode: MovementMode::SpaceInertia {
                angular_speed,
                thrust,
                drag: 0.985,
            },
        }
    }

    /// Contrôleur de Déplacement Grille / Sokoban (déplacement quantifié pas-à-pas)
    #[allow(dead_code)]
    pub fn grid(cell_size: f32, cooldown: f32) -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            speed: cell_size / cooldown,
            is_player_controlled: true,
            patrol: None,
            mode: MovementMode::Grid {
                cell_size,
                cooldown,
                timer: 0.0,
            },
        }
    }

    pub fn patrol_vertical(speed: f32, min_y: f32, max_y: f32) -> Self {
        Self {
            vx: 0.0,
            vy: speed,
            speed,
            is_player_controlled: false,
            patrol: Some(PatrolConfig { min_y, max_y, dir: 1.0 }),
            mode: MovementMode::TopDown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisionSensing {
    pub range: f32,
    pub is_alerted: bool,
    pub alert_timer: f32,
}

impl VisionSensing {
    pub fn new(range: f32) -> Self {
        Self {
            range,
            is_alerted: false,
            alert_timer: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KVLScriptComponent {
    pub script_path: String,
    pub on_overlap_event: Option<String>,
    pub on_interact_event: Option<String>,
    pub on_seen_event: Option<String>,
}

impl KVLScriptComponent {
    pub fn new(script_path: impl Into<String>) -> Self {
        Self {
            script_path: script_path.into(),
            on_overlap_event: None,
            on_interact_event: None,
            on_seen_event: None,
        }
    }

    pub fn with_overlap(mut self, evt: impl Into<String>) -> Self {
        self.on_overlap_event = Some(evt.into());
        self
    }

    pub fn with_interact(mut self, evt: impl Into<String>) -> Self {
        self.on_interact_event = Some(evt.into());
        self
    }

    pub fn with_seen(mut self, evt: impl Into<String>) -> Self {
        self.on_seen_event = Some(evt.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmitterMode {
    Continuous,
    Burst,
}

/// Composant Émetteur de Particules Déclaratif ECS
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleEmitter2D {
    pub mode: EmitterMode,
    pub rate: f32,
    pub accumulator: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub angle: f32,
    pub spread: f32,
    pub gravity_x: f32,
    pub gravity_y: f32,
    pub color_start: u32,
    pub color_end: u32,
    pub is_emitting: bool,
}

impl ParticleEmitter2D {
    /// Gabarit : Flammes vives ascendantes de torche ou brasier
    pub fn torch_fire() -> Self {
        Self {
            mode: EmitterMode::Continuous,
            rate: 28.0,
            accumulator: 0.0,
            lifetime_min: 0.4,
            lifetime_max: 0.85,
            speed_min: 35.0,
            speed_max: 75.0,
            angle: -std::f32::consts::FRAC_PI_2, // Vers le haut
            spread: 0.65,
            gravity_x: 0.0,
            gravity_y: -45.0, // Flottabilité thermique
            color_start: 0x00FDE047, // Jaune d'or chaud
            color_end: 0x00DC2626,   // Rouge braise
            is_emitting: true,
        }
    }

    /// Gabarit : Aura magique mystique cyan vers pourpre
    #[allow(dead_code)]
    pub fn magic_aura() -> Self {
        Self {
            mode: EmitterMode::Continuous,
            rate: 20.0,
            accumulator: 0.0,
            lifetime_min: 0.6,
            lifetime_max: 1.2,
            speed_min: 15.0,
            speed_max: 45.0,
            angle: -std::f32::consts::FRAC_PI_2,
            spread: std::f32::consts::TAU, // Omnidirectionnel
            gravity_x: 0.0,
            gravity_y: -15.0,
            color_start: 0x0038BDF8, // Cyan éclatant
            color_end: 0x00A855F7,   // Violet arcanique
            is_emitting: true,
        }
    }

    /// Gabarit : Panache de fumée vaporeux grisâtre
    #[allow(dead_code)]
    pub fn smoke_plume() -> Self {
        Self {
            mode: EmitterMode::Continuous,
            rate: 15.0,
            accumulator: 0.0,
            lifetime_min: 0.8,
            lifetime_max: 1.6,
            speed_min: 20.0,
            speed_max: 50.0,
            angle: -std::f32::consts::FRAC_PI_2,
            spread: 0.85,
            gravity_x: 10.0, // Léger vent vers la droite
            gravity_y: -30.0,
            color_start: 0x0094A3B8, // Gris clair
            color_end: 0x00334155,   // Ardoise sombre
            is_emitting: true,
        }
    }

    /// Gabarit : Gerbe d'étincelles instantanée (Burst KVL)
    #[allow(dead_code)]
    pub fn spark_burst() -> Self {
        Self {
            mode: EmitterMode::Burst,
            rate: 0.0,
            accumulator: 0.0,
            lifetime_min: 0.3,
            lifetime_max: 0.7,
            speed_min: 80.0,
            speed_max: 200.0,
            angle: -std::f32::consts::FRAC_PI_2,
            spread: std::f32::consts::TAU,
            gravity_x: 0.0,
            gravity_y: 120.0, // Gravité retombante
            color_start: 0x00FDE047,
            color_end: 0x00EA580C,
            is_emitting: true,
        }
    }
}

/// Composant Projectile balistique universel (balles, lasers, flèches)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Projectile2D {
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub damage: i32,
    pub source_id: usize,
}

impl Projectile2D {
    pub fn new(max_lifetime: f32, damage: i32, source_id: usize) -> Self {
        Self {
            lifetime: 0.0,
            max_lifetime,
            damage,
            source_id,
        }
    }
}

/// Composant universel de Vitalité, Dégâts et Points de Vie (HP)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Health2D {
    pub current: f32,
    pub max: f32,
    #[serde(default)]
    pub invulnerability_timer: f32,
    #[serde(default)]
    pub is_dead: bool,
}

impl Health2D {
    pub fn new(max: f32) -> Self {
        Self {
            current: max,
            max,
            invulnerability_timer: 0.0,
            is_dead: false,
        }
    }

    /// Inflige des dégâts avec période d'invulnérabilité protectrice (i-frames)
    pub fn damage(&mut self, amount: f32) -> bool {
        if self.is_dead || self.invulnerability_timer > 0.0 {
            return false;
        }
        self.current = (self.current - amount).max(0.0);
        self.invulnerability_timer = 0.8; // 800ms de frames d'invulnérabilité
        if self.current <= 0.0 {
            self.is_dead = true;
        }
        true
    }

    /// Restaure des points de vie sans dépasser le maximum
    pub fn heal(&mut self, amount: f32) -> bool {
        if amount <= 0.0 || self.current >= self.max {
            return false;
        }
        self.current = (self.current + amount).min(self.max);
        if self.current > 0.0 {
            self.is_dead = false;
        }
        true
    }
}

/// Composant Dialogue et Quête pour PNJ (Personnages Non Joueurs) et narrateurs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialogueComponent {
    pub speaker: String,
    pub text: String,
    pub choices: Vec<String>,
    pub avatar_label: String,
    pub avatar_color: u32,
    pub interact_radius: f32,
    pub quest_id: Option<String>,
    pub quest_step: Option<i64>,
}

/// Composant d'objet ramassable dans le monde (World Item Pickup)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemPickupComponent {
    pub item: crate::inventory::Item,
    pub count: u32,
    pub bob_timer: f32,
    pub base_y: f32,
}

impl ItemPickupComponent {
    pub fn new(item: crate::inventory::Item, count: u32, base_y: f32) -> Self {
        Self {
            item,
            count,
            bob_timer: 0.0,
            base_y,
        }
    }
}

impl DialogueComponent {
    pub fn new(speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            speaker: speaker.into(),
            text: text.into(),
            choices: Vec::new(),
            avatar_label: "NPC".to_string(),
            avatar_color: 0x0038BDF8,
            interact_radius: 80.0,
            quest_id: None,
            quest_step: None,
        }
    }

    pub fn with_choices(mut self, choices: Vec<String>) -> Self {
        self.choices = choices;
        self
    }

    pub fn with_avatar(mut self, label: impl Into<String>, color: u32) -> Self {
        self.avatar_label = label.into();
        self.avatar_color = color;
        self
    }

    #[allow(dead_code)]
    pub fn with_quest(mut self, quest_id: impl Into<String>, step: i64) -> Self {
        self.quest_id = Some(quest_id.into());
        self.quest_step = Some(step);
        self
    }
}

/// Phases d'affrontement et états dynamiques de combat de Boss
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BossPhase {
    /// Phase 1 : Éveil du Titan (100% à 66% PV) - Tirs en éventail et anneaux lents
    Phase1,
    /// Phase 2 : Surcharge Plasmique (66% à 33% PV) - Vitesse accrue et frappes sismiques
    Phase2,
    /// Phase 3 : Mode Overdrive Enragé (33% à 0% PV) - Rafales frénétiques et bombardements orbitaux
    Phase3,
    /// Défaite : Vaincu et anéanti (0 PV)
    Defeated,
}

impl BossPhase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Phase1 => "PHASE 1 : EVEIL",
            Self::Phase2 => "PHASE 2 : SURCHARGE",
            Self::Phase3 => "PHASE 3 : OVERDRIVE ENRAGE",
            Self::Defeated => "VAINCU",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            Self::Phase1 => 0x0006B6D4, // Cyan éclatant
            Self::Phase2 => 0x00F59E0B, // Ambre flamboyant
            Self::Phase3 => 0x00EF4444, // Rouge cramoisi agressif
            Self::Defeated => 0x0010B981, // Vert triomphe
        }
    }
}

/// Archétypes d'attaques télégraphiées et de motifs balistiques de Boss
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BossAttackType {
    /// Anneau circulaire de projectiles expansifs (Nova à 360 degrés)
    RadialNova,
    /// Salve en éventail de 5 à 7 projectiles orientés vers le joueur
    TargetedSpread,
    /// Frappe tellurique / Écrasement au sol avec zone d'onde de choc dévastatrice
    SeismicSlam,
    /// Bombardement orbital / Pluie de météores ciblant les alentours du joueur
    OrbitalStrike,
}

impl BossAttackType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::RadialNova => "NOVA RADIALE",
            Self::TargetedSpread => "TIR EN EVENTAIL",
            Self::SeismicSlam => "SLAM SISMIQUE",
            Self::OrbitalStrike => "BOMBARDEMENT ORBITAL",
        }
    }
}

/// Forme géométrique de la zone d'avertissement au sol (Télégraphie)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TelegraphShape {
    Circle { radius: f32 },
    Rect { w: f32, h: f32 },
    Line { length: f32, thickness: f32, angle: f32 },
}

/// Zone de préavis visuel (Telegraphing) avant impact d'attaque
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelegraphZone {
    pub x: f32,
    pub y: f32,
    pub shape: TelegraphShape,
    pub duration: f32,
    pub timer: f32,
    pub attack_type: BossAttackType,
    pub color: u32,
    pub damage: f32,
    pub is_active: bool,
}

impl TelegraphZone {
    pub fn new_circle(x: f32, y: f32, radius: f32, duration: f32, attack_type: BossAttackType, color: u32, damage: f32) -> Self {
        Self {
            x,
            y,
            shape: TelegraphShape::Circle { radius },
            duration,
            timer: 0.0,
            attack_type,
            color,
            damage,
            is_active: true,
        }
    }

    #[allow(dead_code)]
    pub fn new_rect(x: f32, y: f32, w: f32, h: f32, duration: f32, attack_type: BossAttackType, color: u32, damage: f32) -> Self {
        Self {
            x,
            y,
            shape: TelegraphShape::Rect { w, h },
            duration,
            timer: 0.0,
            attack_type,
            color,
            damage,
            is_active: true,
        }
    }

    pub fn new_line(x: f32, y: f32, length: f32, thickness: f32, angle: f32, duration: f32, attack_type: BossAttackType, color: u32, damage: f32) -> Self {
        Self {
            x,
            y,
            shape: TelegraphShape::Line { length, thickness, angle },
            duration,
            timer: 0.0,
            attack_type,
            color,
            damage,
            is_active: true,
        }
    }

    #[inline]
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.timer / self.duration).clamp(0.0, 1.0)
        }
    }
}

/// Composant Boss ECS pour affrontements épiques multi-phases et télégraphies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossComponent {
    pub phase: BossPhase,
    pub phase_timer: f32,
    pub attack_cooldown: f32,
    pub attack_timer: f32,
    pub current_attack: Option<BossAttackType>,
    pub telegraphs: Vec<TelegraphZone>,
    pub enrage_multiplier: f32,
    pub attack_counter: usize,
    pub name: String,
    pub boss_title: String,
}

impl BossComponent {
    pub fn new(name: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            phase: BossPhase::Phase1,
            phase_timer: 0.0,
            attack_cooldown: 2.4,
            attack_timer: 0.8,
            current_attack: None,
            telegraphs: Vec::new(),
            enrage_multiplier: 1.0,
            attack_counter: 0,
            name: name.into(),
            boss_title: title.into(),
        }
    }

    pub fn with_cooldown(mut self, cooldown: f32) -> Self {
        self.attack_cooldown = cooldown;
        self
    }
}

/// Modèle d'Entité Universel du moteur KaractiRage
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: usize,
    pub name: String,
    pub tag: String,
    pub transform: Transform2D,
    pub renderable: Option<Renderable2D>,
    pub collider: Option<Collider2D>,
    pub movement: Option<Movement2D>,
    pub vision: Option<VisionSensing>,
    pub script: Option<KVLScriptComponent>,
    pub emitter: Option<ParticleEmitter2D>,
    pub projectile: Option<Projectile2D>,
    pub health: Option<Health2D>,
    #[serde(default)]
    pub dialogue: Option<DialogueComponent>,
    #[serde(default)]
    pub item_pickup: Option<ItemPickupComponent>,
    #[serde(default)]
    pub boss: Option<BossComponent>,
    #[serde(default)]
    pub nav_agent: Option<crate::navigation::NavAgent>,
    #[serde(default)]
    pub flock: Option<crate::flocking::FlockMember>,
    pub is_active: bool,
}

impl Entity {
    pub fn new(id: usize, name: impl Into<String>, tag: impl Into<String>, transform: Transform2D) -> Self {
        Self {
            id,
            name: name.into(),
            tag: tag.into(),
            transform,
            renderable: None,
            collider: None,
            movement: None,
            vision: None,
            script: None,
            emitter: None,
            projectile: None,
            health: None,
            dialogue: None,
            item_pickup: None,
            boss: None,
            nav_agent: None,
            flock: None,
            is_active: true,
        }
    }

    #[allow(dead_code)]
    pub fn with_flock(mut self, flock: crate::flocking::FlockMember) -> Self {
        self.flock = Some(flock);
        self
    }

    #[allow(dead_code)]
    pub fn with_nav_agent(mut self, nav: crate::navigation::NavAgent) -> Self {
        self.nav_agent = Some(nav);
        self
    }

    pub fn with_boss(mut self, b: BossComponent) -> Self {
        self.boss = Some(b);
        self
    }

    pub fn with_health(mut self, h: Health2D) -> Self {
        self.health = Some(h);
        self
    }

    pub fn with_dialogue(mut self, d: DialogueComponent) -> Self {
        self.dialogue = Some(d);
        self
    }

    pub fn with_item_pickup(mut self, pickup: ItemPickupComponent) -> Self {
        self.item_pickup = Some(pickup);
        self
    }

    pub fn with_renderable(mut self, r: Renderable2D) -> Self {
        self.renderable = Some(r);
        self
    }

    pub fn with_collider(mut self, c: Collider2D) -> Self {
        self.collider = Some(c);
        self
    }

    pub fn with_movement(mut self, m: Movement2D) -> Self {
        self.movement = Some(m);
        self
    }

    pub fn with_vision(mut self, v: VisionSensing) -> Self {
        self.vision = Some(v);
        self
    }

    pub fn with_script(mut self, s: KVLScriptComponent) -> Self {
        self.script = Some(s);
        self
    }

    pub fn with_emitter(mut self, e: ParticleEmitter2D) -> Self {
        self.emitter = Some(e);
        self
    }

    pub fn with_projectile(mut self, p: Projectile2D) -> Self {
        self.projectile = Some(p);
        self
    }

    /// Gabarit : Projectile balistique universel (balle, laser)
    pub fn spawn_projectile(id: usize, x: f32, y: f32, vx: f32, vy: f32, source_id: usize) -> Self {
        Self::new(id, format!("bullet_{}", id), "projectile", Transform2D::new(x, y, 8.0, 8.0))
            .with_renderable(Renderable2D::circle(4.0, 0x00FDE047, 12))
            .with_collider(Collider2D::trigger(8.0, 8.0))
            .with_movement(Movement2D {
                vx,
                vy,
                speed: (vx * vx + vy * vy).sqrt(),
                is_player_controlled: false,
                patrol: None,
                mode: MovementMode::TopDown,
            })
            .with_projectile(Projectile2D::new(2.0, 10, source_id))
    }

    /// Gabarit : Mur solide bloquant AABB
    pub fn spawn_wall(id: usize, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new(id, format!("wall_{}", id), "wall", Transform2D::new(x, y, w, h))
            .with_renderable(Renderable2D::rect(0x001E293B, 1).with_outline(0x00475569, 2))
            .with_collider(Collider2D::solid(w, h))
    }

    /// Gabarit : Objet collectible doré (pièce)
    pub fn spawn_collectible(id: usize, x: f32, y: f32, radius: f32, script_path: &str) -> Self {
        let size = radius * 2.0;
        Self::new(id, format!("coin_{}", id), "collectible", Transform2D::new(x, y, size, size))
            .with_renderable(Renderable2D::circle(radius, 0x00F59E0B, 5))
            .with_collider(Collider2D::trigger(size, size))
            .with_script(KVLScriptComponent::new(script_path).with_overlap("OnPlayerOverlap"))
    }

    /// Gabarit : Porte sécurisée interactive
    pub fn spawn_door(id: usize, x: f32, y: f32, w: f32, h: f32, script_path: &str) -> Self {
        Self::new(id, format!("door_{}", id), "door", Transform2D::new(x, y, w, h))
            .with_renderable(Renderable2D::rect(0x00EF4444, 2))
            .with_collider(Collider2D::solid(w, h))
            .with_script(KVLScriptComponent::new(script_path).with_interact("OnDoorInteract"))
    }

    /// Gabarit : Sentinelle patrouilleuse avec cône de vision
    pub fn spawn_enemy(id: usize, x: f32, y: f32, size: f32, speed: f32, min_y: f32, max_y: f32, script_path: &str) -> Self {
        Self::new(id, format!("enemy_{}", id), "enemy", Transform2D::new(x, y, size, size))
            .with_renderable(Renderable2D::rect(0x00DC2626, 8).with_outline(0x00F87171, 1))
            .with_movement(Movement2D::patrol_vertical(speed, min_y, max_y))
            .with_vision(VisionSensing::new(200.0))
            .with_script(KVLScriptComponent::new(script_path).with_seen("OnPlayerSeen"))
    }

    /// Gabarit : Téléporteur quantique interactif
    pub fn spawn_portal(id: usize, x: f32, y: f32, w: f32, h: f32, script_path: &str) -> Self {
        Self::new(id, format!("portal_{}", id), "portal", Transform2D::new(x, y, w, h))
            .with_renderable(Renderable2D::rect(0x0006B6D4, 3).with_outline(0x0038BDF8, 2))
            .with_collider(Collider2D::trigger(w, h))
            .with_script(KVLScriptComponent::new(script_path).with_overlap("OnPlayerOverlap"))
    }

    /// Gabarit : Torche murale avec émetteur de flammes ascendantes
    pub fn spawn_torch(id: usize, x: f32, y: f32) -> Self {
        Self::new(id, format!("torch_{}", id), "torch", Transform2D::new(x, y, 16.0, 24.0))
            .with_renderable(Renderable2D::rect(0x0078350F, 3).with_outline(0x00B45309, 1))
            .with_emitter(ParticleEmitter2D::torch_fire())
    }

    /// Gabarit : Bloc poussable (puzzles, Sokoban, obstacles dynamiques)
    pub fn spawn_crate(id: usize, x: f32, y: f32, size: f32) -> Self {
        Self::new(id, format!("crate_{}", id), "crate", Transform2D::new(x, y, size, size))
            .with_renderable(Renderable2D::rect(0x00B45309, 5).with_outline(0x00F4AF6A, 2))
            .with_collider(Collider2D::pushable(size, size))
    }

    /// Gabarit : Dalle de pression (s'active quand un bloc poussable la recouvre)
    pub fn spawn_plate(id: usize, x: f32, y: f32, size: f32) -> Self {
        Self::new(id, format!("plate_{}", id), "plate", Transform2D::new(x, y, size, size))
            .with_renderable(Renderable2D::rect(0x00082F49, 2).with_outline(0x000284C7, 2))
            .with_collider(Collider2D::trigger(size, size))
    }

    /// Gabarit : Piège à piques / Danger environnemental (inflige des dégâts au joueur)
    pub fn spawn_hazard(id: usize, x: f32, y: f32, size: f32) -> Self {
        Self::new(id, format!("trap_{}", id), "hazard", Transform2D::new(x, y, size, size))
            .with_renderable(Renderable2D::rect(0x00450A0A, 3).with_outline(0x00EF4444, 2))
            .with_collider(Collider2D::trigger(size, size))
    }

    /// Gabarit : Fiole de vitalité / Cœur (restaure de la santé)
    pub fn spawn_heart(id: usize, x: f32, y: f32, radius: f32) -> Self {
        Self::new(id, format!("heart_{}", id), "heart", Transform2D::new(x - radius, y - radius, radius * 2.0, radius * 2.0))
            .with_renderable(Renderable2D::circle(radius, 0x00EF4444, 6).with_outline(0x00FECACA, 1))
            .with_collider(Collider2D::trigger(radius * 2.0, radius * 2.0))
    }

    /// Gabarit : PNJ / Gardien / Sage interactif avec dialogues et quêtes KVL
    pub fn spawn_npc(
        id: usize,
        name: impl Into<String>,
        x: f32,
        y: f32,
        color: u32,
        speaker: impl Into<String>,
        text: impl Into<String>,
        choices: Vec<String>,
    ) -> Self {
        let speaker_str = speaker.into();
        let av_lbl = if speaker_str.len() >= 3 {
            speaker_str[..3].to_uppercase()
        } else {
            "NPC".to_string()
        };
        Self::new(id, name, "npc", Transform2D::new(x, y, 32.0, 32.0))
            .with_renderable(Renderable2D::circle(16.0, color, 6).with_outline(0x00E0E7FF, 2))
            .with_collider(Collider2D::solid(32.0, 32.0))
            .with_dialogue(
                DialogueComponent::new(speaker_str, text)
                    .with_choices(choices)
                    .with_avatar(av_lbl, color),
            )
    }

    /// Gabarit : Objet d'inventaire ramassable dans le monde (World Item Pickup)
    pub fn spawn_item_pickup(
        id: usize,
        x: f32,
        y: f32,
        item: crate::inventory::Item,
        count: u32,
    ) -> Self {
        let size = 22.0;
        let color = item.color;
        let name = format!("pickup_{}_{}", item.id, id);
        Self::new(id, name, "item_pickup", Transform2D::new(x, y, size, size))
            .with_renderable(Renderable2D::circle(size * 0.5, color, 6).with_outline(0x00FFFFFF, 1))
            .with_collider(Collider2D::trigger(size, size))
            .with_item_pickup(ItemPickupComponent::new(item, count, y))
    }

    /// Gabarit : Boss imposant multi-phases avec télégraphie et motifs balistiques
    pub fn spawn_boss(
        id: usize,
        name: impl Into<String>,
        x: f32,
        y: f32,
        size: f32,
        max_hp: f32,
        script_path: &str,
    ) -> Self {
        let name_str = name.into();
        Self::new(id, name_str.clone(), "boss", Transform2D::new(x, y, size, size))
            .with_renderable(
                Renderable2D::rect(0x0006B6D4, 9)
                    .with_outline(0x0038BDF8, 3),
            )
            .with_collider(Collider2D::solid(size, size))
            .with_health(Health2D::new(max_hp))
            .with_boss(BossComponent::new(name_str, "GARDIEN ANCESTRAL KVL").with_cooldown(2.2))
            .with_script(
                KVLScriptComponent::new(script_path)
                    .with_overlap("OnBossOverlap")
                    .with_seen("OnBossPhaseChange"),
            )
    }

    /// Gabarit : Drone de meute intelligent (Essaim Boid)
    #[allow(dead_code)]
    pub fn spawn_flock_drone(
        id: usize,
        swarm_id: u32,
        x: f32,
        y: f32,
        is_leader: bool,
    ) -> Self {
        let size = if is_leader { 22.0 } else { 16.0 };
        let color = if is_leader { 0x00F59E0B } else { 0x0038BDF8 };
        let outline = if is_leader { 0x00FEF08A } else { 0x00E0E7FF };
        let tag = if is_leader { "swarm_leader" } else { "swarm_drone" };
        let name = format!("boid_{}_{}", swarm_id, id);

        let mut flock = crate::flocking::FlockMember::drone(swarm_id);
        flock = flock.with_leader(is_leader);

        Self::new(id, name, tag, Transform2D::new(x, y, size, size))
            .with_renderable(Renderable2D::circle(size * 0.5, color, 7).with_outline(outline, 2))
            .with_collider(Collider2D::solid(size, size))
            .with_movement(Movement2D {
                vx: 0.0,
                vy: 0.0,
                speed: flock.max_speed,
                is_player_controlled: false,
                patrol: None,
                mode: MovementMode::TopDown,
            })
            .with_flock(flock)
    }
}
