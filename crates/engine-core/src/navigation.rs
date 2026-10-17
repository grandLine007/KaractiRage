use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use serde::{Deserialize, Serialize};

/// Mode de comportement de l'agent de navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavMode {
    Idle,
    GoTo { x: i32, y: i32 },
    ChasePlayer,
    FleePlayer,
    PatrolWaypoints,
}

impl Default for NavMode {
    fn default() -> Self {
        NavMode::Idle
    }
}

impl NavMode {
    pub fn label(&self) -> &'static str {
        match self {
            NavMode::Idle => "INACTIF",
            NavMode::GoTo { .. } => "VERS CIBLE",
            NavMode::ChasePlayer => "TRAQUE JOUEUR",
            NavMode::FleePlayer => "FUITE JOUEUR",
            NavMode::PatrolWaypoints => "PATROUILLE",
        }
    }
}

/// Composant ECS pour doter une entité d'une navigation intelligente autonome
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavAgent {
    pub mode: NavMode,
    pub speed: f32,
    pub waypoints: Vec<(f32, f32)>,
    pub current_waypoint_idx: usize,
    pub repath_timer: f32,
    pub repath_interval: f32,
    pub reach_distance: f32,
    pub target_pos: Option<(f32, f32)>,
    pub avoid_hazards: bool,
    pub is_stuck: bool,
    pub stuck_timer: f32,
    pub last_pos: (f32, f32),
}

impl Default for NavAgent {
    fn default() -> Self {
        Self {
            mode: NavMode::Idle,
            speed: 90.0,
            waypoints: Vec::new(),
            current_waypoint_idx: 0,
            repath_timer: 0.0,
            repath_interval: 0.25, // Recalcul toutes les 250ms ou si la cible dévie
            reach_distance: 12.0,
            target_pos: None,
            avoid_hazards: true,
            is_stuck: false,
            stuck_timer: 0.0,
            last_pos: (0.0, 0.0),
        }
    }
}

#[allow(dead_code)]
impl NavAgent {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            ..Default::default()
        }
    }

    pub fn with_mode(mut self, mode: NavMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn chase_player(speed: f32) -> Self {
        Self {
            mode: NavMode::ChasePlayer,
            speed,
            ..Default::default()
        }
    }

    pub fn go_to(speed: f32, x: f32, y: f32) -> Self {
        Self {
            mode: NavMode::GoTo {
                x: x.round() as i32,
                y: y.round() as i32,
            },
            speed,
            target_pos: Some((x, y)),
            ..Default::default()
        }
    }

    pub fn stop(&mut self) {
        self.mode = NavMode::Idle;
        self.waypoints.clear();
        self.current_waypoint_idx = 0;
        self.target_pos = None;
    }

    pub fn has_path(&self) -> bool {
        self.current_waypoint_idx < self.waypoints.len()
    }

    pub fn current_target(&self) -> Option<(f32, f32)> {
        if self.current_waypoint_idx < self.waypoints.len() {
            Some(self.waypoints[self.current_waypoint_idx])
        } else {
            None
        }
    }
}

/// Nœud de recherche A* pour la file à priorité
#[derive(Copy, Clone, Eq, PartialEq)]
struct AStarNode {
    pos: (i32, i32),
    f_score: u32,
    h_score: u32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Ordre inversé pour Min-Heap (f_score le plus bas en tête)
        // Tie-breaking déterministe via pos pour garantir 100% de répétabilité bit-exacte
        other.f_score.cmp(&self.f_score)
            .then_with(|| other.h_score.cmp(&self.h_score))
            .then_with(|| (other.pos.0, other.pos.1).cmp(&(self.pos.0, self.pos.1)))
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Moteur de Pathfinding A* 2D optimisé sur grille avec réutilisation de mémoire
pub struct AStarPathfinder {
    pub cell_size: f32,
    pub max_iterations: usize,
    open_set: BinaryHeap<AStarNode>,
    came_from: HashMap<(i32, i32), (i32, i32)>,
    g_score: HashMap<(i32, i32), u32>,
    closed_set: HashMap<(i32, i32), bool>,
}

impl AStarPathfinder {
    pub fn new(cell_size: f32, max_iterations: usize) -> Self {
        Self {
            cell_size,
            max_iterations,
            open_set: BinaryHeap::with_capacity(128),
            came_from: HashMap::with_capacity(256),
            g_score: HashMap::with_capacity(256),
            closed_set: HashMap::with_capacity(256),
        }
    }

    /// Convertit des coordonnées monde continues en cellule de grille entière
    #[inline]
    pub fn world_to_cell(&self, wx: f32, wy: f32) -> (i32, i32) {
        (
            (wx / self.cell_size).floor() as i32,
            (wy / self.cell_size).floor() as i32,
        )
    }

    /// Convertit une cellule entière en coordonnées monde (centre de la cellule)
    #[inline]
    pub fn cell_to_world_center(&self, cx: i32, cy: i32) -> (f32, f32) {
        (
            cx as f32 * self.cell_size + self.cell_size * 0.5,
            cy as f32 * self.cell_size + self.cell_size * 0.5,
        )
    }

    /// Heuristique de Manhattan déterministe (en dixièmes d'unités de coût)
    #[inline]
    fn heuristic(&self, from: (i32, i32), to: (i32, i32)) -> u32 {
        let dx = (from.0 - to.0).abs() as u32;
        let dy = (from.1 - to.1).abs() as u32;
        // Coût cardinal = 10
        (dx + dy) * 10
    }

    /// Calcule le chemin optimal entre `start_world` et `target_world`.
    /// `is_walkable` est un prédicat retournant (franchissable, cout_additionnel_piege).
    pub fn find_path<F>(
        &mut self,
        start_world: (f32, f32),
        target_world: (f32, f32),
        mut is_walkable: F,
    ) -> Option<Vec<(f32, f32)>>
    where
        F: FnMut(i32, i32) -> (bool, u32),
    {
        let start = self.world_to_cell(start_world.0, start_world.1);
        let goal = self.world_to_cell(target_world.0, target_world.1);

        // Si déjà arrivé sur la cellule cible
        if start == goal {
            return Some(vec![target_world]);
        }

        // Nettoyage sans désallocation (Zero-Alloc Hot Path)
        self.open_set.clear();
        self.came_from.clear();
        self.g_score.clear();
        self.closed_set.clear();

        let start_h = self.heuristic(start, goal);
        self.g_score.insert(start, 0);
        self.open_set.push(AStarNode {
            pos: start,
            f_score: start_h,
            h_score: start_h,
        });

        // Voisins en 4 directions (Haut, Bas, Gauche, Droite) pour une navigation solide sur grille
        const NEIGHBORS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

        let mut iterations = 0;
        let mut best_closest = start;
        let mut best_closest_dist = start_h;

        while let Some(current) = self.open_set.pop() {
            iterations += 1;
            if iterations > self.max_iterations {
                // Limite de calcul atteinte : renvoyer le chemin partiel vers le point le plus proche
                break;
            }

            if current.pos == goal {
                return Some(self.reconstruct_path(start, goal, target_world));
            }

            self.closed_set.insert(current.pos, true);

            let current_g = match self.g_score.get(&current.pos) {
                Some(&g) => g,
                None => continue,
            };

            for &(dx, dy) in &NEIGHBORS {
                let neighbor = (current.pos.0 + dx, current.pos.1 + dy);

                if self.closed_set.contains_key(&neighbor) {
                    continue;
                }

                let (walkable, penalty) = is_walkable(neighbor.0, neighbor.1);
                // Si la cellule est bloquée et que ce n'est pas l'objectif direct
                if !walkable && neighbor != goal {
                    continue;
                }

                let step_cost = 10 + penalty;
                let tentative_g = current_g + step_cost;

                let existing_g = self.g_score.get(&neighbor).copied().unwrap_or(u32::MAX);

                if tentative_g < existing_g {
                    self.came_from.insert(neighbor, current.pos);
                    self.g_score.insert(neighbor, tentative_g);
                    let h = self.heuristic(neighbor, goal);
                    let f = tentative_g + h;

                    if h < best_closest_dist && (walkable || neighbor == goal) {
                        best_closest = neighbor;
                        best_closest_dist = h;
                    }

                    self.open_set.push(AStarNode {
                        pos: neighbor,
                        f_score: f,
                        h_score: h,
                    });
                }
            }
        }

        // Si l'objectif exact est inaccessible (ex: muré ou au-delà de max_iterations),
        // générer un chemin vers la cellule la plus proche atteinte.
        if best_closest != start {
            let (target_wx, target_wy) = self.cell_to_world_center(best_closest.0, best_closest.1);
            Some(self.reconstruct_path(start, best_closest, (target_wx, target_wy)))
        } else {
            None
        }
    }

    /// Reconstruit et lisse la liste des waypoints monde à partir du graphe parent
    fn reconstruct_path(
        &self,
        start: (i32, i32),
        goal: (i32, i32),
        exact_target: (f32, f32),
    ) -> Vec<(f32, f32)> {
        let mut path_cells = Vec::new();
        let mut curr = goal;
        path_cells.push(curr);

        while curr != start {
            if let Some(&prev) = self.came_from.get(&curr) {
                curr = prev;
                path_cells.push(curr);
            } else {
                break;
            }
        }

        path_cells.reverse();

        // Convertir en coordonnées monde
        let mut waypoints = Vec::with_capacity(path_cells.len());
        // Sauter le premier point (la cellule où se trouve déjà l'agent) si on a plusieurs points
        let start_idx = if path_cells.len() > 1 { 1 } else { 0 };

        for i in start_idx..path_cells.len() {
            if i == path_cells.len() - 1 {
                waypoints.push(exact_target);
            } else {
                let cell = path_cells[i];
                waypoints.push(self.cell_to_world_center(cell.0, cell.1));
            }
        }

        waypoints
    }
}

impl Default for AStarPathfinder {
    fn default() -> Self {
        Self::new(32.0, 384)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_straight_line_path() {
        let mut pf = AStarPathfinder::new(32.0, 200);
        // De (16, 16) [cell 0,0] vers (112, 16) [cell 3,0]
        let path = pf.find_path((16.0, 16.0), (112.0, 16.0), |_cx, _cy| (true, 0));
        assert!(path.is_some(), "Un chemin direct doit être trouvé");
        let wps = path.unwrap();
        assert!(!wps.is_empty());
        let last = wps.last().unwrap();
        assert_eq!(*last, (112.0, 16.0));
    }

    #[test]
    fn test_obstacle_avoidance_path() {
        let mut pf = AStarPathfinder::new(32.0, 200);

        // Mur vertical en x = 2 bloquant de y = 0 à y = 2
        let is_walkable = |cx: i32, cy: i32| {
            if cx == 2 && (0..=2).contains(&cy) {
                (false, 0)
            } else {
                (true, 0)
            }
        };

        // Départ en (0, 1) vers cible en (4, 1)
        let path = pf.find_path((16.0, 48.0), (144.0, 48.0), is_walkable);
        assert!(path.is_some(), "Le chemin doit contourner le mur");
        let wps = path.unwrap();

        // Vérifier qu'aucun waypoint n'est dans le mur (cx = 2, cy = 0..=2)
        for &(wx, wy) in &wps {
            let (cx, cy) = pf.world_to_cell(wx, wy);
            assert!(!(cx == 2 && (0..=2).contains(&cy)), "Le chemin ne doit pas traverser l'obstacle");
        }
    }

    #[test]
    fn test_hazard_penalty_avoidance() {
        let mut pf = AStarPathfinder::new(32.0, 200);

        // Deux chemins possibles de (0,1) à (2,1) :
        // - Ligne droite en (1, 1) qui contient un piège (pénalité +100)
        // - Contournement par (1, 2) qui est dégagé (pénalité 0)
        let is_walkable = |cx: i32, cy: i32| {
            if cx == 1 && cy == 1 {
                (true, 100) // Piège traversable mais très coûteux
            } else {
                (true, 0)
            }
        };

        let path = pf.find_path((16.0, 48.0), (80.0, 48.0), is_walkable);
        assert!(path.is_some());
        let wps = path.unwrap();

        // Le pathfinder doit avoir choisi d'éviter le piège (1, 1) en passant par une autre ordonnée
        let touches_hazard = wps.iter().any(|&(wx, wy)| {
            let (cx, cy) = pf.world_to_cell(wx, wy);
            cx == 1 && cy == 1
        });
        assert!(!touches_hazard, "L'agent intelligent doit contourner le piège grâce à la pénalisation de coût");
    }

    #[test]
    fn test_deterministic_repeatability() {
        let mut pf = AStarPathfinder::new(32.0, 200);
        let start = (10.0, 25.0);
        let goal = (250.0, 180.0);

        let path1 = pf.find_path(start, goal, |cx, cy| (cx != 3 || cy != 2, 0)).unwrap();
        let path2 = pf.find_path(start, goal, |cx, cy| (cx != 3 || cy != 2, 0)).unwrap();

        assert_eq!(path1, path2, "Le pathfinding A* doit être 100% déterministe et reproductible au bit près");
    }
}
