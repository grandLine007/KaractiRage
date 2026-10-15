#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Play,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioTool {
    Select,
    Brush,
    Eraser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushTile {
    Wall,
    Hazard,
    Collectible,
    Heart,
    Crate,
    Plate,
}

impl BrushTile {
    pub fn id(&self) -> &'static str {
        match self {
            BrushTile::Wall => "wall",
            BrushTile::Hazard => "hazard",
            BrushTile::Collectible => "collectible",
            BrushTile::Heart => "heart",
            BrushTile::Crate => "crate",
            BrushTile::Plate => "plate",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BrushTile::Wall => "Mur Solide",
            BrushTile::Hazard => "Piege Piques",
            BrushTile::Collectible => "Piece KVL",
            BrushTile::Heart => "Fiole Coeur",
            BrushTile::Crate => "Bloc Poussable",
            BrushTile::Plate => "Dalle Pression",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            BrushTile::Wall => "Mur",
            BrushTile::Hazard => "Piege",
            BrushTile::Collectible => "Piece",
            BrushTile::Heart => "Coeur",
            BrushTile::Crate => "Caisse",
            BrushTile::Plate => "Dalle",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            BrushTile::Wall => 0x0094A3B8,
            BrushTile::Hazard => 0x00EF4444,
            BrushTile::Collectible => 0x00F59E0B,
            BrushTile::Heart => 0x00EC4899,
            BrushTile::Crate => 0x00F4AF6A,
            BrushTile::Plate => 0x0034D399,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            BrushTile::Wall => BrushTile::Hazard,
            BrushTile::Hazard => BrushTile::Collectible,
            BrushTile::Collectible => BrushTile::Heart,
            BrushTile::Heart => BrushTile::Crate,
            BrushTile::Crate => BrushTile::Plate,
            BrushTile::Plate => BrushTile::Wall,
        }
    }
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
    pub active_tool: StudioTool,
    pub brush_tile: BrushTile,
    pub last_painted_cell: Option<(i32, i32)>,
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
    pub top_save_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub add_node_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub view_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub show_spawn_palette: bool,
    pub spawn_palette_hitboxes: Vec<PaletteItemHitbox>,
    pub spawn_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub duplicate_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub delete_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_select_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_brush_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_eraser_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_tile_cycle_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub add_comp_hitboxes: Vec<(String, i32, i32, u32, u32)>,
    pub show_help_modal: bool,
    pub help_btn_hitbox: Option<(i32, i32, u32, u32)>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            mode: EditorMode::Play,
            selected: SelectedEntity::None,
            active_tool: StudioTool::Select,
            brush_tile: BrushTile::Wall,
            last_painted_cell: None,
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
            top_save_btn_hitbox: None,
            add_node_btn_hitbox: None,
            view_btn_hitbox: None,
            show_spawn_palette: false,
            spawn_palette_hitboxes: Vec::new(),
            spawn_btn_hitbox: None,
            duplicate_btn_hitbox: None,
            delete_btn_hitbox: None,
            tool_select_btn_hitbox: None,
            tool_brush_btn_hitbox: None,
            tool_eraser_btn_hitbox: None,
            tool_tile_cycle_btn_hitbox: None,
            add_comp_hitboxes: Vec::new(),
            show_help_modal: false,
            help_btn_hitbox: None,
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
                self.show_spawn_palette = false;
                self.show_help_modal = false;
                self.active_tool = StudioTool::Select;
                self.last_painted_cell = None;
                EditorMode::Play
            }
        };
    }

    pub fn set_tool(&mut self, tool: StudioTool) {
        self.active_tool = tool;
        self.last_painted_cell = None;
        if tool != StudioTool::Select {
            self.selected = SelectedEntity::None;
            self.is_dragging = false;
            self.active_wire = None;
            self.show_node_palette = false;
            self.show_spawn_palette = false;
        }
    }

    pub fn cycle_brush_tile(&mut self) -> BrushTile {
        self.brush_tile = self.brush_tile.next();
        self.brush_tile
    }

    pub fn toggle_help(&mut self) {
        self.show_help_modal = !self.show_help_modal;
    }

    pub fn toggle_view_detail(&mut self) {
        self.is_detailed_view = !self.is_detailed_view;
        self.active_wire = None;
        self.show_node_palette = false;
        self.show_spawn_palette = false;
    }

    pub fn toggle_palette(&mut self) {
        self.show_node_palette = !self.show_node_palette;
        if self.show_node_palette {
            self.show_spawn_palette = false;
        }
    }

    pub fn toggle_spawn_palette(&mut self) {
        self.show_spawn_palette = !self.show_spawn_palette;
        if self.show_spawn_palette {
            self.show_node_palette = false;
        }
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

    pub fn find_spawn_palette_item_at(&self, mx: f32, my: f32) -> Option<&PaletteItemHitbox> {
        self.spawn_palette_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn find_add_comp_at(&self, mx: f32, my: f32) -> Option<&str> {
        self.add_comp_hitboxes.iter().find(|(_, bx, by, bw, bh)| {
            mx >= *bx as f32 && mx <= (*bx + *bw as i32) as f32
                && my >= *by as f32 && my <= (*by + *bh as i32) as f32
        }).map(|(comp, _, _, _, _)| comp.as_str())
    }

    pub fn is_save_btn_clicked(&self, mx: f32, my: f32) -> bool {
        let in_box = |opt: Option<(i32, i32, u32, u32)>| {
            if let Some((bx, by, bw, bh)) = opt {
                mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
            } else {
                false
            }
        };
        in_box(self.save_btn_hitbox) || in_box(self.top_save_btn_hitbox)
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

    pub fn is_spawn_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.spawn_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_duplicate_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.duplicate_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_delete_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.delete_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_help_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.help_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_select_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_select_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_brush_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_brush_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_eraser_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_eraser_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_tile_cycle_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_tile_cycle_btn_hitbox {
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
