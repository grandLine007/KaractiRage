#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Play,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedEntity {
    None,
    Player,
    Coin(usize),
    Door(usize),
}

pub struct EditorState {
    pub mode: EditorMode,
    pub selected: SelectedEntity,
    pub is_detailed_view: bool,
    pub is_dragging: bool,
    pub drag_offset_x: f32,
    pub drag_offset_y: f32,
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
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            EditorMode::Play => EditorMode::Edit,
            EditorMode::Edit => {
                self.selected = SelectedEntity::None;
                self.is_dragging = false;
                EditorMode::Play
            }
        };
    }

    pub fn toggle_view_detail(&mut self) {
        self.is_detailed_view = !self.is_detailed_view;
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
