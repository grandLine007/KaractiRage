use serde::{Deserialize, Serialize};

/// Rôle d'un membre au sein de la meute / essaim
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlockRole {
    Follower,
    Leader,
    Scout,
}

impl Default for FlockRole {
    fn default() -> Self {
        FlockRole::Follower
    }
}

impl FlockRole {
    pub fn label(&self) -> &'static str {
        match self {
            FlockRole::Follower => "MEMBRE",
            FlockRole::Leader => "LEADER",
            FlockRole::Scout => "ECLAIREUR",
        }
    }
}

/// Composant ECS pour doter une entité d'un comportement collectif de meute (Boids de Reynolds)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlockMember {
    pub swarm_id: u32,
    pub role: FlockRole,
    pub perception_radius: f32,
    pub separation_radius: f32,
    pub max_speed: f32,
    pub max_force: f32,
    pub weight_separation: f32,
    pub weight_alignment: f32,
    pub weight_cohesion: f32,
    pub weight_target: f32,
    pub target: Option<(f32, f32)>,
    pub scatter_timer: f32,
}

impl Default for FlockMember {
    fn default() -> Self {
        Self {
            swarm_id: 1,
            role: FlockRole::Follower,
            perception_radius: 80.0,
            separation_radius: 26.0,
            max_speed: 130.0,
            max_force: 180.0,
            weight_separation: 1.6,
            weight_alignment: 1.0,
            weight_cohesion: 1.1,
            weight_target: 1.3,
            target: None,
            scatter_timer: 0.0,
        }
    }
}

#[allow(dead_code)]
impl FlockMember {
    pub fn new(swarm_id: u32) -> Self {
        Self {
            swarm_id,
            ..Default::default()
        }
    }

    /// Preset pour micro-drones véloces et réactifs
    pub fn drone(swarm_id: u32) -> Self {
        Self {
            swarm_id,
            role: FlockRole::Follower,
            perception_radius: 96.0,
            separation_radius: 30.0,
            max_speed: 160.0,
            max_force: 220.0,
            weight_separation: 1.8,
            weight_alignment: 1.2,
            weight_cohesion: 1.0,
            weight_target: 1.5,
            target: None,
            scatter_timer: 0.0,
        }
    }

    /// Preset pour meute terrestre (créatures, minions)
    pub fn minion(swarm_id: u32) -> Self {
        Self {
            swarm_id,
            role: FlockRole::Follower,
            perception_radius: 70.0,
            separation_radius: 22.0,
            max_speed: 105.0,
            max_force: 150.0,
            weight_separation: 1.5,
            weight_alignment: 0.9,
            weight_cohesion: 1.2,
            weight_target: 1.2,
            target: None,
            scatter_timer: 0.0,
        }
    }

    /// Transforme le membre en leader d'escadron
    pub fn with_leader(mut self, is_leader: bool) -> Self {
        self.role = if is_leader {
            FlockRole::Leader
        } else {
            FlockRole::Follower
        };
        self
    }

    /// Affecte une cible ponctuelle mondiale
    pub fn with_target(mut self, x: f32, y: f32) -> Self {
        self.target = Some((x, y));
        self
    }

    /// Déclenche un éparpillement d'urgence (panique)
    pub fn scatter(&mut self, duration: f32) {
        self.scatter_timer = duration.max(0.1);
    }

    #[inline]
    pub fn is_scattered(&self) -> bool {
        self.scatter_timer > 0.0
    }
}

/// Instantané léger d'un voisin pour le calcul de meute
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoidNeighbor {
    pub id: usize,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub is_leader: bool,
}

/// Calcule la force de pilotage (Steering Force) déterministe pour un membre de la meute
pub fn compute_flock_steering(
    member: &FlockMember,
    pos: (f32, f32),
    vel: (f32, f32),
    neighbors: &[BoidNeighbor],
    override_target: Option<(f32, f32)>,
) -> (f32, f32) {
    let (x, y) = pos;
    let (vx, vy) = vel;
    let max_speed = member.max_speed;

    // 1. SÉPARATION : éviter le surpeuplement intime
    let mut sep_x = 0.0;
    let mut sep_y = 0.0;
    let mut sep_count = 0;
    let sep_r_sq = member.separation_radius * member.separation_radius;

    for n in neighbors {
        let dx = x - n.x;
        let dy = y - n.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq > 0.0001 && dist_sq < sep_r_sq {
            let dist = dist_sq.sqrt();
            sep_x += (dx / dist) / dist;
            sep_y += (dy / dist) / dist;
            sep_count += 1;
        }
    }

    if sep_count > 0 {
        sep_x /= sep_count as f32;
        sep_y /= sep_count as f32;
        let mag = (sep_x * sep_x + sep_y * sep_y).sqrt();
        if mag > 0.0001 {
            sep_x = (sep_x / mag) * max_speed - vx;
            sep_y = (sep_y / mag) * max_speed - vy;
        }
    }

    // 2. ALIGNEMENT : égaliser la direction de vol avec les voisins
    let mut align_vx = 0.0;
    let mut align_vy = 0.0;
    let mut align_count = 0;
    let perc_r_sq = member.perception_radius * member.perception_radius;

    for n in neighbors {
        let dx = n.x - x;
        let dy = n.y - y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq > 0.0001 && dist_sq < perc_r_sq {
            align_vx += n.vx;
            align_vy += n.vy;
            align_count += 1;
        }
    }

    if align_count > 0 {
        align_vx /= align_count as f32;
        align_vy /= align_count as f32;
        let mag = (align_vx * align_vx + align_vy * align_vy).sqrt();
        if mag > 0.0001 {
            align_vx = (align_vx / mag) * max_speed - vx;
            align_vy = (align_vy / mag) * max_speed - vy;
        }
    }

    // 3. COHÉSION : attirer vers le centre de masse des voisins
    let mut coh_x = 0.0;
    let mut coh_y = 0.0;
    let mut coh_count = 0;

    for n in neighbors {
        let dx = n.x - x;
        let dy = n.y - y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq > 0.0001 && dist_sq < perc_r_sq {
            let weight = if n.is_leader { 2.5 } else { 1.0 };
            coh_x += n.x * weight;
            coh_y += n.y * weight;
            coh_count += 1;
        }
    }

    if coh_count > 0 {
        coh_x /= coh_count as f32;
        coh_y /= coh_count as f32;
        let to_center_x = coh_x - x;
        let to_center_y = coh_y - y;
        let mag = (to_center_x * to_center_x + to_center_y * to_center_y).sqrt();
        if mag > 0.0001 {
            coh_x = (to_center_x / mag) * max_speed - vx;
            coh_y = (to_center_y / mag) * max_speed - vy;
        }
    }

    // 4. ATTRACTION VERS LA CIBLE / JOUEUR
    let mut tgt_force_x = 0.0;
    let mut tgt_force_y = 0.0;
    let active_target = override_target.or(member.target);

    if let Some((tx, ty)) = active_target {
        let dx = tx - x;
        let dy = ty - y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > 8.0 {
            tgt_force_x = (dx / dist) * max_speed - vx;
            tgt_force_y = (dy / dist) * max_speed - vy;
        }
    }

    // 5. PONDÉRATION & MODE SCATTER (PANIQUE)
    let (w_sep, w_ali, w_coh, w_tgt) = if member.scatter_timer > 0.0 {
        (member.weight_separation * 3.5, 0.1, -1.5, 0.0)
    } else {
        (
            member.weight_separation,
            member.weight_alignment,
            member.weight_cohesion,
            member.weight_target,
        )
    };

    let mut fx = sep_x * w_sep + align_vx * w_ali + coh_x * w_coh + tgt_force_x * w_tgt;
    let mut fy = sep_y * w_sep + align_vy * w_ali + coh_y * w_coh + tgt_force_y * w_tgt;

    // Écrêtage à la force maximale
    let f_mag = (fx * fx + fy * fy).sqrt();
    if f_mag > member.max_force {
        fx = (fx / f_mag) * member.max_force;
        fy = (fy / f_mag) * member.max_force;
    }

    (fx, fy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flock_separation_repels_close_boids() {
        let boid = FlockMember::drone(1);
        let pos = (100.0, 100.0);
        let vel = (0.0, 0.0);

        // Voisin très proche à droite (110.0, 100.0), distance = 10 px < 30 px
        let neighbors = vec![BoidNeighbor {
            id: 2,
            x: 110.0,
            y: 100.0,
            vx: 0.0,
            vy: 0.0,
            is_leader: false,
        }];

        let (fx, fy) = compute_flock_steering(&boid, pos, vel, &neighbors, None);
        // Doit pousser vigoureusement vers la gauche (fx < 0)
        assert!(fx < -50.0, "La séparation doit repousser le boid vers la gauche (fx: {})", fx);
        assert!(fy.abs() < 5.0, "Pas de force verticale attendue (fy: {})", fy);
    }

    #[test]
    fn test_flock_alignment_matches_neighbor_velocities() {
        let mut boid = FlockMember::drone(1);
        boid.weight_separation = 0.0;
        boid.weight_cohesion = 0.0;
        boid.weight_target = 0.0;
        boid.weight_alignment = 1.0;

        let pos = (100.0, 100.0);
        let vel = (0.0, 0.0);

        // Deux voisins filant vers le bas à 100 px/s
        let neighbors = vec![
            BoidNeighbor { id: 2, x: 120.0, y: 100.0, vx: 0.0, vy: 100.0, is_leader: false },
            BoidNeighbor { id: 3, x: 80.0, y: 100.0, vx: 0.0, vy: 100.0, is_leader: false },
        ];

        let (fx, fy) = compute_flock_steering(&boid, pos, vel, &neighbors, None);
        assert!(fy > 50.0, "L'alignement doit accélérer vers le bas (fy: {})", fy);
        assert!(fx.abs() < 5.0, "Pas de dérive horizontale (fx: {})", fx);
    }

    #[test]
    fn test_flock_cohesion_pulls_towards_center_of_mass() {
        let mut boid = FlockMember::drone(1);
        boid.weight_separation = 0.0;
        boid.weight_alignment = 0.0;
        boid.weight_target = 0.0;
        boid.weight_cohesion = 1.0;

        let pos = (100.0, 100.0);
        let vel = (0.0, 0.0);

        // Voisins situés en (150.0, 100.0), centre de masse à droite
        let neighbors = vec![
            BoidNeighbor { id: 2, x: 150.0, y: 90.0, vx: 0.0, vy: 0.0, is_leader: false },
            BoidNeighbor { id: 3, x: 150.0, y: 110.0, vx: 0.0, vy: 0.0, is_leader: false },
        ];

        let (fx, _fy) = compute_flock_steering(&boid, pos, vel, &neighbors, None);
        assert!(fx > 50.0, "La cohésion doit attirer vers le centre de masse à droite (fx: {})", fx);
    }

    #[test]
    fn test_flock_scatter_mode_disperses_swarm() {
        let mut boid = FlockMember::drone(1);
        boid.scatter(2.0);
        assert!(boid.is_scattered());

        let pos = (100.0, 100.0);
        let vel = (0.0, 0.0);

        // Voisin à droite
        let neighbors = vec![BoidNeighbor {
            id: 2,
            x: 140.0,
            y: 100.0,
            vx: 0.0,
            vy: 0.0,
            is_leader: false,
        }];

        let (fx, _) = compute_flock_steering(&boid, pos, vel, &neighbors, None);
        // En mode panique/scatter, la répulsion doit être décuplée
        assert!(fx < -50.0, "L'éparpillement doit disperser le boid loin du centre");
    }

    #[test]
    fn test_flock_deterministic_simulation() {
        let boid = FlockMember::drone(1);
        let pos = (200.0, 200.0);
        let vel = (10.0, -5.0);

        let neighbors = vec![
            BoidNeighbor { id: 2, x: 220.0, y: 190.0, vx: 15.0, vy: -2.0, is_leader: false },
            BoidNeighbor { id: 3, x: 190.0, y: 230.0, vx: 8.0, vy: -10.0, is_leader: true },
        ];

        let (fx1, fy1) = compute_flock_steering(&boid, pos, vel, &neighbors, Some((300.0, 200.0)));
        let (fx2, fy2) = compute_flock_steering(&boid, pos, vel, &neighbors, Some((300.0, 200.0)));

        assert_eq!(fx1, fx2, "Le calcul de flocking doit être strictement déterministe");
        assert_eq!(fy1, fy2, "Le calcul de flocking doit être strictement déterministe");
    }
}
