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
            layer: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PatrolConfig {
    pub min_y: f32,
    pub max_y: f32,
    pub dir: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Movement2D {
    pub vx: f32,
    pub vy: f32,
    pub speed: f32,
    pub is_player_controlled: bool,
    pub patrol: Option<PatrolConfig>,
}

impl Movement2D {
    pub fn player(speed: f32) -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            speed,
            is_player_controlled: true,
            patrol: None,
        }
    }

    pub fn patrol_vertical(speed: f32, min_y: f32, max_y: f32) -> Self {
        Self {
            vx: 0.0,
            vy: speed,
            speed,
            is_player_controlled: false,
            patrol: Some(PatrolConfig { min_y, max_y, dir: 1.0 }),
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
}
