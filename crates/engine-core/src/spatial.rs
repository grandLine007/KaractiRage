#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialKind {
    Solid,
    Trigger,
    Entity(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialItem {
    pub id: usize,
    pub kind: SpatialKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[allow(dead_code)]
impl SpatialItem {
    pub fn solid(id: usize, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { id, kind: SpatialKind::Solid, x, y, w, h }
    }

    pub fn trigger(id: usize, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { id, kind: SpatialKind::Trigger, x, y, w, h }
    }

    pub fn entity(id: usize, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { id, kind: SpatialKind::Entity(id), x, y, w, h }
    }

    #[inline]
    pub fn is_solid(&self) -> bool {
        self.kind == SpatialKind::Solid
    }

    #[inline]
    pub fn is_trigger(&self) -> bool {
        self.kind == SpatialKind::Trigger
    }
}

pub struct SpatialHashGrid {
    pub cell_size: f32,
    inv_cell_size: f32,
    buckets: Vec<Vec<SpatialItem>>,
    bucket_mask: usize,
    query_buffer: Vec<SpatialItem>,
    pub stats_queries: usize,
    pub stats_candidates: usize,
    pub debug_visible: bool,
}

impl SpatialHashGrid {
    pub fn new(cell_size: f32, bucket_count: usize) -> Self {
        // Garantir une puissance de deux pour le masque de hachage rapide
        let count = bucket_count.next_power_of_two().max(64);
        let mut buckets = Vec::with_capacity(count);
        for _ in 0..count {
            buckets.push(Vec::with_capacity(8));
        }

        Self {
            cell_size,
            inv_cell_size: 1.0 / cell_size,
            buckets,
            bucket_mask: count - 1,
            query_buffer: Vec::with_capacity(32),
            stats_queries: 0,
            stats_candidates: 0,
            debug_visible: false,
        }
    }

    #[inline]
    fn hash_cell(&self, cx: i32, cy: i32) -> usize {
        let h = (cx.wrapping_mul(73856093) ^ cy.wrapping_mul(19349663)) as usize;
        h & self.bucket_mask
    }

    /// Réinitialise les seaux sans désallouer la mémoire (Zero-Alloc Hot Path)
    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.stats_queries = 0;
        self.stats_candidates = 0;
    }

    /// Insère un élément AABB dans toutes les cellules qu'il chevauche
    pub fn insert(&mut self, item: SpatialItem) {
        let min_cx = (item.x * self.inv_cell_size).floor() as i32;
        let max_cx = ((item.x + item.w) * self.inv_cell_size).floor() as i32;
        let min_cy = (item.y * self.inv_cell_size).floor() as i32;
        let max_cy = ((item.y + item.h) * self.inv_cell_size).floor() as i32;

        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                let idx = self.hash_cell(cx, cy);
                self.buckets[idx].push(item);
            }
        }
    }

    /// Recherche les éléments chevauchant une boîte englobante AABB
    pub fn query_aabb(&mut self, qx: f32, qy: f32, qw: f32, qh: f32) -> &[SpatialItem] {
        self.query_buffer.clear();
        self.stats_queries += 1;

        let min_cx = (qx * self.inv_cell_size).floor() as i32;
        let max_cx = ((qx + qw) * self.inv_cell_size).floor() as i32;
        let min_cy = (qy * self.inv_cell_size).floor() as i32;
        let max_cy = ((qy + qh) * self.inv_cell_size).floor() as i32;

        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                let idx = self.hash_cell(cx, cy);
                for item in &self.buckets[idx] {
                    // Dédoublonnage instantané
                    if !self.query_buffer.iter().any(|e| e.id == item.id && e.kind == item.kind) {
                        // Test d'intersection AABB strict
                        if qx + qw > item.x && qx < item.x + item.w && qy + qh > item.y && qy < item.y + item.h {
                            self.query_buffer.push(*item);
                        }
                    }
                }
            }
        }

        self.stats_candidates += self.query_buffer.len();
        &self.query_buffer
    }

    /// Recherche les éléments traversant un segment de droite (pour Raycasting Line of Sight)
    pub fn query_segment(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &[SpatialItem] {
        let qx = x1.min(x2);
        let qy = y1.min(y2);
        let qw = (x2 - x1).abs().max(1.0);
        let qh = (y2 - y1).abs().max(1.0);
        self.query_aabb(qx, qy, qw, qh)
    }

    pub fn toggle_debug(&mut self) -> bool {
        self.debug_visible = !self.debug_visible;
        self.debug_visible
    }
}

impl Default for SpatialHashGrid {
    fn default() -> Self {
        Self::new(64.0, 256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_grid_insert_and_query_dedup() {
        let mut grid = SpatialHashGrid::new(64.0, 128);

        let item1 = SpatialItem {
            id: 1,
            kind: SpatialKind::Solid,
            x: 10.0,
            y: 10.0,
            w: 100.0, // Chevauche 2 cellules X (0 et 1)
            h: 30.0,
        };

        let item2 = SpatialItem {
            id: 2,
            kind: SpatialKind::Trigger,
            x: 200.0,
            y: 200.0,
            w: 20.0,
            h: 20.0,
        };

        grid.insert(item1);
        grid.insert(item2);

        // Requête dans la zone de item1
        let results = grid.query_aabb(50.0, 15.0, 20.0, 20.0);
        assert_eq!(results.len(), 1, "Un seul élément doit être retourné sans doublon");
        assert_eq!(results[0].id, 1);

        // Requête dans une zone vide
        let empty = grid.query_aabb(500.0, 500.0, 50.0, 50.0);
        assert!(empty.is_empty(), "La zone vide ne doit retourner aucun candidat");

        // Requête englobant item1 et item2
        let both = grid.query_aabb(0.0, 0.0, 300.0, 300.0);
        assert_eq!(both.len(), 2, "Les deux éléments doivent être trouvés");
    }

    #[test]
    fn test_spatial_grid_clear_zero_alloc() {
        let mut grid = SpatialHashGrid::new(64.0, 64);
        grid.insert(SpatialItem {
            id: 1,
            kind: SpatialKind::Solid,
            x: 10.0,
            y: 10.0,
            w: 20.0,
            h: 20.0,
        });

        assert_eq!(grid.query_aabb(0.0, 0.0, 50.0, 50.0).len(), 1);

        grid.clear();
        assert_eq!(grid.query_aabb(0.0, 0.0, 50.0, 50.0).len(), 0);
        assert_eq!(grid.stats_queries, 1);
    }
}
