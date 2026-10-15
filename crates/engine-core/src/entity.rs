use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rotation: f32,
}

impl Transform2D {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            rotation: 0.0,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rect,
    Circle { radius: f32 },
    Text { label: String, scale: u32 },
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
    #[allow(dead_code)]
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
            is_active: true,
        }
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
}
