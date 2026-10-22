use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera2D {
    /// Position X du coin supérieur gauche de la caméra dans le monde
    pub x: f32,
    /// Position Y du coin supérieur gauche de la caméra dans le monde
    pub y: f32,
    /// Cible X à suivre dans le monde (ex: centre du joueur)
    pub target_x: f32,
    /// Cible Y à suivre dans le monde (ex: centre du joueur)
    pub target_y: f32,
    /// Origine écran X du viewport (ex: 20px)
    pub viewport_x: i32,
    /// Origine écran Y du viewport (ex: 66px)
    pub viewport_y: i32,
    /// Largeur en pixels du viewport écran
    pub viewport_w: u32,
    /// Hauteur en pixels du viewport écran
    pub viewport_h: u32,
    /// Limites du monde (min_x, min_y, max_x, max_y)
    pub bounds: Option<(f32, f32, f32, f32)>,
    /// Vitesse de suivi interpolé (Lerp). 0.0 = téléportation instantanée
    pub lerp_speed: f32,
    /// Largeur de la zone morte (deadzone) où la cible peut bouger sans déplacer la caméra
    pub deadzone_w: f32,
    /// Hauteur de la zone morte (deadzone)
    pub deadzone_h: f32,
    /// Décalage X temporaire dû aux secousses d'écran
    pub shake_offset_x: f32,
    /// Décalage Y temporaire dû aux secousses d'écran
    pub shake_offset_y: f32,
    /// Intensité résiduelle de secousse d'écran
    pub shake_intensity: f32,
    /// Taux d'amortissement de la secousse par seconde
    pub shake_decay: f32,
    /// Horloge interne pour les secousses
    pub shake_timer: f32,
}

impl Camera2D {
    pub fn new(viewport_x: i32, viewport_y: i32, viewport_w: u32, viewport_h: u32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            viewport_x,
            viewport_y,
            viewport_w,
            viewport_h,
            bounds: None,
            lerp_speed: 6.0,
            deadzone_w: 64.0,
            deadzone_h: 48.0,
            shake_offset_x: 0.0,
            shake_offset_y: 0.0,
            shake_intensity: 0.0,
            shake_decay: 18.0,
            shake_timer: 0.0,
        }
    }

    /// Redéfinit la zone écran du viewport
    pub fn set_viewport(&mut self, vx: i32, vy: i32, vw: u32, vh: u32) {
        self.viewport_x = vx;
        self.viewport_y = vy;
        self.viewport_w = vw;
        self.viewport_h = vh;
        self.clamp_to_bounds();
    }

    /// Définit les frontières physiques maximales du niveau
    pub fn set_bounds(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
        self.bounds = Some((min_x, min_y, max_x, max_y));
        self.clamp_to_bounds();
    }

    /// Supprime les limites de défilement du monde
    #[allow(dead_code)]
    pub fn clear_bounds(&mut self) {
        self.bounds = None;
    }

    /// Modifie la coordonnée cible suivie par la caméra
    #[allow(dead_code)]
    pub fn set_target(&mut self, tx: f32, ty: f32) {
        self.target_x = tx;
        self.target_y = ty;
    }

    /// Centre instantanément la caméra sur la cible sans interpolation
    pub fn snap_to_target(&mut self) {
        self.x = self.target_x - (self.viewport_w as f32 * 0.5);
        self.y = self.target_y - (self.viewport_h as f32 * 0.5);
        self.clamp_to_bounds();
    }

    /// Ajoute une impulsion de secousse d'écran (Screen Shake)
    pub fn add_shake(&mut self, intensity: f32) {
        self.shake_intensity = (self.shake_intensity + intensity).min(28.0);
    }

    /// Déplacement direct de la caméra (pour navigation libre dans le Studio)
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
        self.target_x = self.x + (self.viewport_w as f32 * 0.5);
        self.target_y = self.y + (self.viewport_h as f32 * 0.5);
        self.clamp_to_bounds();
    }

    /// Met à jour la position de la caméra, la zone morte, le lerp et les secousses
    pub fn update(&mut self, dt: f32) {
        // 1. Simulation des secousses d'écran
        if self.shake_intensity > 0.0 {
            self.shake_timer += dt;
            self.shake_intensity = (self.shake_intensity - self.shake_decay * dt).max(0.0);
            self.shake_offset_x = (self.shake_timer * 45.0).sin() * self.shake_intensity;
            self.shake_offset_y = (self.shake_timer * 60.0).cos() * self.shake_intensity;
        } else {
            self.shake_offset_x = 0.0;
            self.shake_offset_y = 0.0;
            self.shake_timer = 0.0;
        }

        // 2. Calcul du centre actuel de la caméra dans le monde
        let cur_center_x = self.x + (self.viewport_w as f32 * 0.5);
        let cur_center_y = self.y + (self.viewport_h as f32 * 0.5);

        let dx = self.target_x - cur_center_x;
        let dy = self.target_y - cur_center_y;

        let half_dead_w = self.deadzone_w * 0.5;
        let half_dead_h = self.deadzone_h * 0.5;

        let mut move_x = 0.0;
        if dx > half_dead_w {
            move_x = dx - half_dead_w;
        } else if dx < -half_dead_w {
            move_x = dx + half_dead_w;
        }

        let mut move_y = 0.0;
        if dy > half_dead_h {
            move_y = dy - half_dead_h;
        } else if dy < -half_dead_h {
            move_y = dy + half_dead_h;
        }

        if self.lerp_speed <= 0.0 {
            self.x += move_x;
            self.y += move_y;
        } else {
            let factor = (self.lerp_speed * dt).min(1.0);
            self.x += move_x * factor;
            self.y += move_y * factor;
        }

        // 3. Confinement dans les bornes du monde
        self.clamp_to_bounds();
    }

    /// Contraint la caméra pour ne pas dépasser les limites du monde
    pub fn clamp_to_bounds(&mut self) {
        if let Some((min_x, min_y, max_x, max_y)) = self.bounds {
            let vw = self.viewport_w as f32;
            let vh = self.viewport_h as f32;

            if max_x - min_x >= vw {
                self.x = self.x.clamp(min_x, max_x - vw);
            } else {
                self.x = min_x + (max_x - min_x - vw) * 0.5;
            }

            if max_y - min_y >= vh {
                self.y = self.y.clamp(min_y, max_y - vh);
            } else {
                self.y = min_y + (max_y - min_y - vh) * 0.5;
            }
        }
    }

    /// Convertit des coordonnées Monde (World) en coordonnées Écran (Screen)
    pub fn world_to_screen(&self, wx: f32, wy: f32) -> (i32, i32) {
        let sx = self.viewport_x + ((wx - self.x) + self.shake_offset_x).round() as i32;
        let sy = self.viewport_y + ((wy - self.y) + self.shake_offset_y).round() as i32;
        (sx, sy)
    }

    /// Convertit des coordonnées Écran (Screen) en coordonnées Monde (World)
    pub fn screen_to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        let wx = self.x + (sx - self.viewport_x as f32) - self.shake_offset_x;
        let wy = self.y + (sy - self.viewport_y as f32) - self.shake_offset_y;
        (wx, wy)
    }

    /// Frustum Culling O(1) : vérifie si une boîte AABB monde est visible dans le viewport de la caméra
    pub fn is_visible(&self, wx: f32, wy: f32, ww: f32, wh: f32) -> bool {
        let margin = 32.0;
        let cam_x = self.x - margin;
        let cam_y = self.y - margin;
        let cam_w = self.viewport_w as f32 + (margin * 2.0);
        let cam_h = self.viewport_h as f32 + (margin * 2.0);

        !(wx + ww < cam_x || wx > cam_x + cam_w || wy + wh < cam_y || wy > cam_y + cam_h)
    }

    /// Retourne le centre actuel de la caméra dans le monde
    #[allow(dead_code)]
    pub fn center_world(&self) -> (f32, f32) {
        (
            self.x + (self.viewport_w as f32 * 0.5),
            self.y + (self.viewport_h as f32 * 0.5),
        )
    }
}

impl Default for Camera2D {
    fn default() -> Self {
        Self::new(20, 66, 1240, 470)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_initialization_and_snap() {
        let mut cam = Camera2D::new(20, 66, 800, 600);
        cam.set_target(500.0, 500.0);
        cam.snap_to_target();

        assert_eq!(cam.x, 500.0 - 400.0);
        assert_eq!(cam.y, 500.0 - 300.0);
        assert_eq!(cam.center_world(), (500.0, 500.0));
    }

    #[test]
    fn test_camera_coordinates_roundtrip() {
        let mut cam = Camera2D::new(20, 66, 800, 600);
        cam.x = 250.0;
        cam.y = 150.0;

        let world_pt = (350.0, 250.0);
        let (sx, sy) = cam.world_to_screen(world_pt.0, world_pt.1);
        let back_world = cam.screen_to_world(sx as f32, sy as f32);

        assert!((back_world.0 - world_pt.0).abs() <= 1.0);
        assert!((back_world.1 - world_pt.1).abs() <= 1.0);
    }

    #[test]
    fn test_camera_deadzone_and_lerp() {
        let mut cam = Camera2D::new(0, 0, 800, 600);
        cam.lerp_speed = 10.0;
        cam.deadzone_w = 100.0;
        cam.deadzone_h = 100.0;
        cam.snap_to_target();

        let initial_x = cam.x;
        // Déplacement à l'intérieur de la zone morte (30px < 50px)
        cam.set_target(cam.target_x + 30.0, cam.target_y);
        cam.update(0.016);
        assert_eq!(cam.x, initial_x, "La caméra ne doit pas bouger dans la zone morte");

        // Déplacement au-delà de la zone morte (80px > 50px)
        cam.set_target(cam.target_x + 50.0, cam.target_y);
        cam.update(0.016);
        assert!(cam.x > initial_x, "La caméra doit bouger quand la cible quitte la zone morte");
    }

    #[test]
    fn test_camera_bounds_clamping() {
        let mut cam = Camera2D::new(0, 0, 800, 600);
        cam.set_bounds(0.0, 0.0, 1600.0, 1200.0);

        // Essayer de centrer à la position x = 0 (dépasserait à gauche)
        cam.set_target(0.0, 0.0);
        cam.snap_to_target();
        assert_eq!(cam.x, 0.0);
        assert_eq!(cam.y, 0.0);

        // Essayer de dépasser à droite
        cam.set_target(2000.0, 1500.0);
        cam.snap_to_target();
        assert_eq!(cam.x, 1600.0 - 800.0);
        assert_eq!(cam.y, 1200.0 - 600.0);
    }

    #[test]
    fn test_camera_frustum_culling() {
        let mut cam = Camera2D::new(0, 0, 800, 600);
        cam.x = 100.0;
        cam.y = 100.0;

        // Entité au centre
        assert!(cam.is_visible(300.0, 300.0, 32.0, 32.0));
        // Entité loin à droite (x=1200 > 100+800+32)
        assert!(!cam.is_visible(1200.0, 300.0, 32.0, 32.0));
        // Entité loin au-dessus (y=-200 < 100-32)
        assert!(!cam.is_visible(300.0, -200.0, 32.0, 32.0));
    }

    #[test]
    fn test_camera_screen_shake_decay() {
        let mut cam = Camera2D::new(0, 0, 800, 600);
        cam.add_shake(10.0);
        assert_eq!(cam.shake_intensity, 10.0);

        cam.update(0.1);
        assert!(cam.shake_intensity < 10.0);
        assert!(cam.shake_offset_x != 0.0 || cam.shake_offset_y != 0.0);

        // Après amortissement complet
        cam.update(1.0);
        assert_eq!(cam.shake_intensity, 0.0);
        assert_eq!(cam.shake_offset_x, 0.0);
        assert_eq!(cam.shake_offset_y, 0.0);
    }
}

// ===== Caméra 3D (Lot 3) =====

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraProjection {
    Perspective { fov_deg: f32, near: f32, far: f32 },
    Orthographic { size: f32, near: f32, far: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3D {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub projection: CameraProjection,
    /// Largeur/hauteur, renseigné à la frame (recalculé par le viewport).
    pub aspect: f32,
}

impl Camera3D {
    pub fn perspective(
        position: Vec3,
        target: Vec3,
        fov_deg: f32,
        near: f32,
        far: f32,
        aspect: f32,
    ) -> Self {
        Self {
            position,
            target,
            up: Vec3::Y,
            projection: CameraProjection::Perspective { fov_deg, near, far },
            aspect,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.position, self.target, self.up)
    }

    /// Matrice de projection (GL clip, NDC z ∈ [-1, 1]). Utile pour les tests et un
    /// futur backend natif ; le `RenderPacket` transporte fov/aspect/near/far (c'est le
    /// renderer three.js qui construit sa propre projection).
    pub fn projection_matrix(&self) -> Mat4 {
        match self.projection {
            CameraProjection::Perspective { fov_deg, near, far } => {
                glam::camera::rh::proj::opengl::perspective(fov_deg.to_radians(), self.aspect, near, far)
            }
            CameraProjection::Orthographic { size, near, far } => {
                let half_h = size * 0.5;
                let half_w = half_h * self.aspect;
                glam::camera::rh::proj::opengl::orthographic(-half_w, half_w, -half_h, half_h, near, far)
            }
        }
    }
}

#[cfg(test)]
mod camera3d_tests {
    use super::*;
    use glam::{Vec3, Vec4};

    #[test]
    fn test_look_at_rh_maps_target_to_center() {
        let cam = Camera3D::perspective(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 60.0, 0.1, 100.0, 1.0);
        let view = cam.view_matrix();
        let origin_cam = view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(origin_cam.x.abs() < 1e-5);
        assert!(origin_cam.y.abs() < 1e-5);
        assert!((origin_cam.z + 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_perspective_focal() {
        let cam = Camera3D::perspective(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 60.0, 0.1, 100.0, 1.0);
        let proj = cam.projection_matrix();
        let focal = 1.0 / (30.0f32).to_radians().tan();
        assert!((proj.x_axis.x - focal).abs() < 1e-4);
    }

    #[test]
    fn test_orthographic_half_extents() {
        let mut cam = Camera3D::perspective(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 60.0, 0.1, 100.0, 2.0);
        cam.projection = CameraProjection::Orthographic { size: 10.0, near: 0.1, far: 100.0 };
        let proj = cam.projection_matrix();
        // half_w = (10/2) * aspect 2 = 10 → proj.x_axis.x = 1 / half_w = 0.1.
        assert!((proj.x_axis.x - 0.1).abs() < 1e-5);
    }
}
