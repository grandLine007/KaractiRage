#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Play,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedEntity {
    None,
    Entity(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWire {
    pub from_node: String,
    pub from_pin: String,
    pub start_x: i32,
    pub start_y: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PinHitbox {
    pub node_id: String,
    pub pin_id: String,
    pub is_output: bool,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub socket_x: i32,
    pub socket_y: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionHitbox {
    pub connection_id: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteItemHitbox {
    pub template_id: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub struct EditorState {
    pub mode: EditorMode,
    pub selected: SelectedEntity,
    pub is_detailed_view: bool,
    pub is_dragging: bool,
    pub drag_offset_x: f32,
    pub drag_offset_y: f32,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub active_wire: Option<ActiveWire>,
    pub pin_hitboxes: Vec<PinHitbox>,
    pub connection_hitboxes: Vec<ConnectionHitbox>,
    pub show_node_palette: bool,
    pub palette_hitboxes: Vec<PaletteItemHitbox>,
    pub save_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub add_node_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub view_btn_hitbox: Option<(i32, i32, u32, u32)>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            mode: EditorMode::Play,
            selected: SelectedEntity::None,
            is_detailed_view: false, // Vue simplifiée par défaut (Ergonomie KaractiRage)
            is_dragging: false,
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            active_wire: None,
            pin_hitboxes: Vec::new(),
            connection_hitboxes: Vec::new(),
            show_node_palette: false,
            palette_hitboxes: Vec::new(),
            save_btn_hitbox: None,
            add_node_btn_hitbox: None,
            view_btn_hitbox: None,
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            EditorMode::Play => EditorMode::Edit,
            EditorMode::Edit => {
                self.selected = SelectedEntity::None;
                self.is_dragging = false;
                self.active_wire = None;
                self.show_node_palette = false;
                EditorMode::Play
            }
        };
    }

    pub fn toggle_view_detail(&mut self) {
        self.is_detailed_view = !self.is_detailed_view;
        self.active_wire = None;
        self.show_node_palette = false;
    }

    pub fn toggle_palette(&mut self) {
        self.show_node_palette = !self.show_node_palette;
    }

    pub fn start_wire(&mut self, node_id: String, pin_id: String, start_x: i32, start_y: i32) {
        self.active_wire = Some(ActiveWire {
            from_node: node_id,
            from_pin: pin_id,
            start_x,
            start_y,
        });
    }

    pub fn cancel_wire(&mut self) {
        self.active_wire = None;
    }

    pub fn find_pin_at(&self, mx: f32, my: f32) -> Option<&PinHitbox> {
        self.pin_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn find_connection_cut_at(&self, mx: f32, my: f32) -> Option<&ConnectionHitbox> {
        self.connection_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn find_palette_item_at(&self, mx: f32, my: f32) -> Option<&PaletteItemHitbox> {
        self.palette_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn is_save_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.save_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_add_node_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.add_node_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_view_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.view_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
