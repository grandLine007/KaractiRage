use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Movement,
    Interactions,
    Combat,
    ArtificialIntelligence,
    UserInterface,
    Progression,
    World,
    Audio,
    MathLogic,
    Custom(String),
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Movement => write!(f, "Mouvement"),
            Category::Interactions => write!(f, "Interactions"),
            Category::Combat => write!(f, "Combat"),
            Category::ArtificialIntelligence => write!(f, "Intelligence Artificielle"),
            Category::UserInterface => write!(f, "Interface"),
            Category::Progression => write!(f, "Progression"),
            Category::World => write!(f, "Monde"),
            Category::Audio => write!(f, "Audio"),
            Category::MathLogic => write!(f, "Maths & Logique"),
            Category::Custom(name) => write!(f, "Custom ({})", name),
        }
    }
}
