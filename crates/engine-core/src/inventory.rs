use crate::audio::{AudioEngine, SoundType};
use crate::entity::Health2D;
use crate::renderer::Canvas;
use serde::{Deserialize, Serialize};

/// Nombre d'emplacements rapides dans la barre d'action (Hotbar)
pub const HOTBAR_SLOTS_COUNT: usize = 5;

/// Capacité par défaut de l'inventaire du joueur
pub const DEFAULT_INVENTORY_CAPACITY: usize = 16;

/// Typologie des objets de KaractiRage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    /// Objet consommable restaurant des PV ou modifiant des stats
    Consumable { heal_amount: i32 },
    /// Objet clef pour déverrouiller des accès, portes ou quêtes KVL
    KeyItem,
    /// Arme ou projectile équipable
    Weapon,
    /// Outil interactif de terrain
    Tool,
    /// Ressource ou objet divers
    Misc,
}

/// Définition unitaire d'un objet
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: ItemKind,
    pub icon_glyph: char,
    pub color: u32,
    pub max_stack: u32,
}

impl Item {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        kind: ItemKind,
        icon_glyph: char,
        color: u32,
        max_stack: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            kind,
            icon_glyph,
            color,
            max_stack,
        }
    }

    /// Gabarit : Potion de vitalité rouge restaurant des points de vie
    pub fn health_potion(heal_amount: i32) -> Self {
        Self::new(
            "potion_heal",
            "Potion de Soin",
            format!("Restaure +{} PV instantanement.", heal_amount),
            ItemKind::Consumable { heal_amount },
            'P',
            0x00EF4444, // Rouge rubis
            10,
        )
    }

    /// Gabarit : Clef ouvragée pour passages verrouillés ou donjons
    pub fn dungeon_key(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(
            id,
            name,
            "Clef doree permettant de deverrouiller une porte scellee.",
            ItemKind::KeyItem,
            'K',
            0x00F59E0B, // Or ambré
            1,
        )
    }

    /// Gabarit : Cristal énergétique brillant
    pub fn energy_crystal(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(
            id,
            name,
            "Cristal lumineux vibrant d'une énergie mystique.",
            ItemKind::KeyItem,
            'C',
            0x0038BDF8, // Cyan ciel
            5,
        )
    }

    /// Gabarit : Parchemin ancien de savoir
    #[allow(dead_code)]
    pub fn ancient_scroll(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(
            id,
            name,
            "Parchemin orné de runes anciennes d'un autre temps.",
            ItemKind::Misc,
            'S',
            0x00E0E7FF, // Parchemin blanc/indigo
            1,
        )
    }

    /// Gabarit : Élixir de vitesse
    #[allow(dead_code)]
    pub fn speed_elixir() -> Self {
        Self::new(
            "elixir_speed",
            "Élixir de Vitesse",
            "Accélère vivement les mouvements du joueur.",
            ItemKind::Consumable { heal_amount: 0 },
            'V',
            0x0010B981, // Émeraude vive
            5,
        )
    }
}

/// Emplacement d'inventaire contenant un objet et une quantité
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySlot {
    pub item: Item,
    pub count: u32,
}

impl InventorySlot {
    pub fn new(item: Item, count: u32) -> Self {
        Self { item, count }
    }
}

/// Résultat d'une tentative d'utilisation d'un objet
#[derive(Debug, Clone, PartialEq)]
pub enum ItemUseResult {
    None,
    ConsumedHeal { amount: i32, new_hp: f32 },
    KeyActivated { item_id: String },
    CannotUse { reason: String },
    Custom { item_id: String },
}

/// Moteur d'Inventaire et Barre d'Actions (Hotbar)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<Option<InventorySlot>>,
    pub capacity: usize,
    pub selected_hotbar_index: usize,
    #[serde(skip)]
    pub hotbar_hitboxes: [Option<(i32, i32, u32, u32)>; HOTBAR_SLOTS_COUNT],
    #[serde(skip)]
    pub use_btn_hitbox: Option<(i32, i32, u32, u32)>,
    #[serde(skip)]
    pub notification: Option<(String, f32)>, // (message, temps restant en secondes)
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(DEFAULT_INVENTORY_CAPACITY)
    }
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(HOTBAR_SLOTS_COUNT);
        let mut slots = Vec::with_capacity(cap);
        for _ in 0..cap {
            slots.push(None);
        }
        Self {
            slots,
            capacity: cap,
            selected_hotbar_index: 0,
            hotbar_hitboxes: [None; HOTBAR_SLOTS_COUNT],
            use_btn_hitbox: None,
            notification: None,
        }
    }

    pub fn is_use_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.use_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    /// Ajoute des objets dans l'inventaire en empilant en priorité, puis sur slots libres.
    /// Retourne la quantité restante qui n'a pas pu être insérée (0 si tout rentre).
    pub fn add_item(&mut self, item: Item, mut count: u32) -> u32 {
        if count == 0 {
            return 0;
        }

        let item_id = item.id.clone();
        let max_stack = item.max_stack;
        let mut added_any = false;

        // 1. Tenter d'empiler sur un emplacement existant du même objet
        for slot_opt in self.slots.iter_mut() {
            if let Some(ref mut slot) = slot_opt {
                if slot.item.id == item_id && slot.count < max_stack {
                    let available_space = max_stack - slot.count;
                    let to_add = count.min(available_space);
                    slot.count += to_add;
                    count -= to_add;
                    added_any = true;
                    if count == 0 {
                        break;
                    }
                }
            }
        }

        // 2. Tenter d'occuper les premiers emplacements vides
        if count > 0 {
            for slot_opt in self.slots.iter_mut() {
                if slot_opt.is_none() {
                    let to_add = count.min(max_stack);
                    *slot_opt = Some(InventorySlot::new(item.clone(), to_add));
                    count -= to_add;
                    added_any = true;
                    if count == 0 {
                        break;
                    }
                }
            }
        }

        if added_any {
            self.set_notification(format!("+{} {}", item.name, if item.max_stack > 1 { "" } else { "" }));
        }

        count
    }

    /// Retire une certaine quantité d'un objet identifié par son id
    #[allow(dead_code)]
    pub fn remove_item(&mut self, item_id: &str, mut count: u32) -> bool {
        if !self.has_item(item_id) {
            return false;
        }

        let total_have = self.get_item_count(item_id);
        if total_have < count {
            return false;
        }

        for slot_opt in self.slots.iter_mut().rev() {
            if let Some(ref mut slot) = slot_opt {
                if slot.item.id == item_id {
                    if slot.count <= count {
                        count -= slot.count;
                        *slot_opt = None;
                    } else {
                        slot.count -= count;
                        count = 0;
                    }
                    if count == 0 {
                        break;
                    }
                }
            }
        }

        true
    }

    /// Vérifie si au moins 1 exemplaire de l'objet est présent
    #[allow(dead_code)]
    pub fn has_item(&self, item_id: &str) -> bool {
        self.get_item_count(item_id) > 0
    }

    /// Calcule la quantité totale possédée d'un objet
    pub fn get_item_count(&self, item_id: &str) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.item.id == item_id)
            .map(|s| s.count)
            .sum()
    }

    /// Sélectionne un slot de la barre rapide (0 à HOTBAR_SLOTS_COUNT - 1)
    pub fn select_slot(&mut self, index: usize) {
        if index < HOTBAR_SLOTS_COUNT {
            self.selected_hotbar_index = index;
        }
    }

    /// Retourne la référence sur le slot actuellement sélectionné dans la barre rapide
    pub fn selected_slot(&self) -> Option<&InventorySlot> {
        if self.selected_hotbar_index < self.slots.len() {
            self.slots[self.selected_hotbar_index].as_ref()
        } else {
            None
        }
    }

    /// Déclenche une notification visuelle temporaire au-dessus de la barre
    pub fn set_notification(&mut self, msg: impl Into<String>) {
        self.notification = Some((msg.into(), 2.8)); // 2.8 secondes d'affichage
    }

    /// Utilise l'objet présent sur un slot donné de la hotbar
    pub fn use_slot(
        &mut self,
        index: usize,
        mut health: Option<&mut Health2D>,
        audio: &mut AudioEngine,
    ) -> ItemUseResult {
        if index >= HOTBAR_SLOTS_COUNT || index >= self.slots.len() {
            return ItemUseResult::None;
        }

        let slot_opt = self.slots[index].clone();
        let slot = match slot_opt {
            Some(s) => s,
            None => return ItemUseResult::None,
        };

        match slot.item.kind {
            ItemKind::Consumable { heal_amount } => {
                if let Some(ref mut hp) = health {
                    if hp.current >= hp.max {
                        audio.play(SoundType::AccessDenied);
                        self.set_notification("Vie déjà au maximum !");
                        return ItemUseResult::CannotUse {
                            reason: "Santé déjà pleine".to_string(),
                        };
                    }

                    hp.heal(heal_amount as f32);
                    let new_hp = hp.current;
                    audio.play(SoundType::PotionDrink);
                    self.set_notification(format!("Soin : +{} PV !", heal_amount));

                    // Consommer 1 exemplaire
                    if slot.count <= 1 {
                        self.slots[index] = None;
                    } else if let Some(ref mut cur_slot) = self.slots[index] {
                        cur_slot.count -= 1;
                    }

                    ItemUseResult::ConsumedHeal {
                        amount: heal_amount,
                        new_hp,
                    }
                } else {
                    ItemUseResult::None
                }
            }

            ItemKind::KeyItem => {
                audio.play(SoundType::KeyUnlock);
                self.set_notification(format!("{}: Prêt à l'usage !", slot.item.name));
                ItemUseResult::KeyActivated {
                    item_id: slot.item.id.clone(),
                }
            }

            ItemKind::Misc | ItemKind::Tool | ItemKind::Weapon => {
                audio.play(SoundType::UiClick);
                self.set_notification(format!("{}: Action activée", slot.item.name));
                ItemUseResult::Custom {
                    item_id: slot.item.id.clone(),
                }
            }
        }
    }

    /// Utilise le slot actuellement sélectionné
    #[allow(dead_code)]
    pub fn use_selected(
        &mut self,
        health: Option<&mut Health2D>,
        audio: &mut AudioEngine,
    ) -> ItemUseResult {
        let idx = self.selected_hotbar_index;
        self.use_slot(idx, health, audio)
    }

    /// Mise à jour temporelle (zéro allocation par frame)
    pub fn update(&mut self, dt: f32) {
        if let Some((_, ref mut timer)) = self.notification {
            *timer -= dt;
            if *timer <= 0.0 {
                self.notification = None;
            }
        }
    }

    /// Rendu de la barre d'action rapide (Hotbar) et des notifications au bas de l'écran
    #[allow(dead_code)]
    pub fn render_hotbar(&mut self, canvas: &mut Canvas, screen_w: u32, screen_h: u32) {
        self.render_hotbar_at(canvas, (screen_w / 2) as i32, (screen_h - 14) as i32);
    }

    /// Rendu de la barre d'action rapide (Hotbar) centrée à des coordonnées précises
    pub fn render_hotbar_at(&mut self, canvas: &mut Canvas, center_x: i32, bottom_y: i32) {
        let slot_size: u32 = 36;
        let spacing: u32 = 6;
        let slots_w = HOTBAR_SLOTS_COUNT as u32 * slot_size + (HOTBAR_SLOTS_COUNT as u32 - 1) * spacing;
        let use_btn_w = 66u32;
        let total_w = slots_w + spacing + use_btn_w;
        let start_x = (center_x - (total_w as i32 / 2)).max(10);
        let start_y = (bottom_y - slot_size as i32).max(10);

        // 1. Fond du bandeau de la barre d'actions
        let pad = 6;
        let bg_x = start_x - pad;
        let bg_y = start_y - pad;
        let bg_w = total_w + (pad as u32 * 2);
        let bg_h = slot_size + (pad as u32 * 2);
        canvas.draw_rect(bg_x, bg_y, bg_w, bg_h, 0x000F172A); // Ardoise très sombre
        canvas.draw_rect_outline(bg_x, bg_y, bg_w, bg_h, 0x00334155, 1);
        canvas.draw_corner_brackets(bg_x, bg_y, bg_w, bg_h, 6, 0x000284C7);

        // 2. Rendu des 5 emplacements
        for i in 0..HOTBAR_SLOTS_COUNT {
            let slot_x = start_x + (i as u32 * (slot_size + spacing)) as i32;
            let slot_y = start_y;
            let is_selected = i == self.selected_hotbar_index;

            // Enregistrer la hitbox pour les clics souris
            self.hotbar_hitboxes[i] = Some((slot_x, slot_y, slot_size, slot_size));

            let slot_bg = if is_selected { 0x001E293B } else { 0x00111827 };
            canvas.draw_rect(slot_x, slot_y, slot_size, slot_size, slot_bg);

            let border_color = if is_selected {
                0x00F59E0B // Halo doré ambré pour le slot sélectionné
            } else {
                0x00374151 // Gris discret
            };
            let thickness = if is_selected { 2 } else { 1 };
            canvas.draw_rect_outline(slot_x, slot_y, slot_size, slot_size, border_color, thickness);

            // Numéro du slot [1..5]
            let num_str = match i {
                0 => "1",
                1 => "2",
                2 => "3",
                3 => "4",
                _ => "5",
            };
            canvas.draw_text(
                slot_x + 3,
                slot_y + 3,
                num_str,
                if is_selected { 0x00FCD34D } else { 0x0094A3B8 },
                1,
            );

            // Contenu du slot
            if let Some(ref slot) = self.slots[i] {
                // Icône / Glyphe central coloré
                let mut icon_buf = [0u8; 4];
                let icon_str = slot.item.icon_glyph.encode_utf8(&mut icon_buf);
                canvas.draw_text(
                    slot_x + 16,
                    slot_y + 13,
                    icon_str,
                    slot.item.color,
                    2,
                );

                // Quantité affichée en bas à droite si > 1
                if slot.count > 1 {
                    let count_str = format!("x{}", slot.count);
                    canvas.draw_text(
                        slot_x + slot_size as i32 - (count_str.len() as i32 * 6) - 2,
                        slot_y + slot_size as i32 - 10,
                        &count_str,
                        0x00F8FAFC,
                        1,
                    );
                }
            }
        }

        // 3. Bouton d'utilisation rapide [U] UTIL
        let use_x = start_x + slots_w as i32 + spacing as i32;
        let use_y = start_y;
        canvas.draw_badge(use_x, use_y, use_btn_w, slot_size, 0x00064E3B, 0x0010B981, "[U] UTIL", 0x00D1FAE5, 1);
        self.use_btn_hitbox = Some((use_x, use_y, use_btn_w, slot_size));

        // 4. Info-bulle de l'objet sélectionné (au-dessus de la barre)
        if let Some(ref slot) = self.selected_slot() {
            let label = format!("[{}] {}", self.selected_hotbar_index + 1, slot.item.name);
            let tip_x = start_x;
            // Sous la notification (bg_y - 42) : les deux libellés ne se touchent plus.
            let tip_y = bg_y - 18;
            canvas.draw_text(tip_x, tip_y, &label, 0x0038BDF8, 1);
        }

        // 4. Notification flottante de ramassage / action (au-dessus du nom)
        if let Some((ref msg, _)) = self.notification {
            let notif_y = bg_y - 42;
            let notif_w = (msg.len() * 6 + 16) as u32;
            let notif_x = center_x - (notif_w as i32 / 2);
            canvas.draw_badge(
                notif_x,
                notif_y,
                notif_w,
                16,
                0x000F172A,
                0x0010B981, // Vert émeraude
                msg,
                0x00F0FDF4,
                1,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_add_stack_and_remove() {
        let mut inv = Inventory::new(16);
        assert!(!inv.has_item("potion_heal"));
        assert_eq!(inv.get_item_count("potion_heal"), 0);

        let potion = Item::health_potion(2);
        let rem = inv.add_item(potion.clone(), 3);
        assert_eq!(rem, 0);
        assert!(inv.has_item("potion_heal"));
        assert_eq!(inv.get_item_count("potion_heal"), 3);

        // Empilage sur le même slot
        let rem2 = inv.add_item(potion.clone(), 4);
        assert_eq!(rem2, 0);
        assert_eq!(inv.get_item_count("potion_heal"), 7);

        // Retrait partiel
        assert!(inv.remove_item("potion_heal", 2));
        assert_eq!(inv.get_item_count("potion_heal"), 5);

        // Retrait total
        assert!(inv.remove_item("potion_heal", 5));
        assert_eq!(inv.get_item_count("potion_heal"), 0);
        assert!(!inv.has_item("potion_heal"));
    }

    #[test]
    fn test_inventory_capacity_and_stack_overflow() {
        let mut inv = Inventory::new(5); // 5 slots
        let key = Item::dungeon_key("key_gold", "Clef d'Or"); // max_stack = 1

        let rem = inv.add_item(key.clone(), 3);
        assert_eq!(rem, 0);
        assert_eq!(inv.get_item_count("key_gold"), 3);

        // Remplir les 2 derniers slots
        inv.add_item(key.clone(), 2);
        assert_eq!(inv.get_item_count("key_gold"), 5);

        // Tenter d'en ajouter une 6e alors que les 5 slots sont pleins
        let overflow = inv.add_item(key, 1);
        assert_eq!(overflow, 1);
        assert_eq!(inv.get_item_count("key_gold"), 5);
    }

    #[test]
    fn test_potion_consumption_and_health_restoration() {
        let mut inv = Inventory::new(16);
        let mut audio = AudioEngine::new();
        let mut health = Health2D::new(5.0);
        health.current = 2.0; // 2/5 PV

        let potion = Item::health_potion(2);
        inv.add_item(potion, 2);
        inv.select_slot(0);

        // Consommer la première potion : 2 + 2 = 4 PV
        let res = inv.use_selected(Some(&mut health), &mut audio);
        assert_eq!(res, ItemUseResult::ConsumedHeal { amount: 2, new_hp: 4.0 });
        assert_eq!(health.current, 4.0);
        assert_eq!(inv.get_item_count("potion_heal"), 1);

        // Consommer la seconde potion : 4 + 2 capé à 5 PV
        let res2 = inv.use_selected(Some(&mut health), &mut audio);
        assert_eq!(res2, ItemUseResult::ConsumedHeal { amount: 2, new_hp: 5.0 });
        assert_eq!(health.current, 5.0);
        assert_eq!(inv.get_item_count("potion_heal"), 0);

        // Tenter de consommer sur slot vide
        let res_empty = inv.use_selected(Some(&mut health), &mut audio);
        assert_eq!(res_empty, ItemUseResult::None);
    }

    #[test]
    fn test_full_health_prevents_potion_waste() {
        let mut inv = Inventory::new(16);
        let mut audio = AudioEngine::new();
        let mut health = Health2D::new(5.0); // 5/5 PV

        inv.add_item(Item::health_potion(2), 1);
        inv.select_slot(0);

        let res = inv.use_selected(Some(&mut health), &mut audio);
        match res {
            ItemUseResult::CannotUse { .. } => {}
            other => panic!("Expected CannotUse when health is full, got {:?}", other),
        }
        // La potion ne doit PAS être consommée
        assert_eq!(inv.get_item_count("potion_heal"), 1);
        assert_eq!(health.current, 5.0);
    }

    #[test]
    fn test_hotbar_selection_and_rendering() {
        let mut inv = Inventory::new(16);
        inv.select_slot(3);
        assert_eq!(inv.selected_hotbar_index, 3);

        // Vérifier que la sélection est bornée à HOTBAR_SLOTS_COUNT
        inv.select_slot(12);
        assert_eq!(inv.selected_hotbar_index, 3); // inchangé car 12 >= 5

        let mut canvas = Canvas::new(800, 600);
        inv.add_item(Item::health_potion(2), 3);
        inv.add_item(Item::dungeon_key("key_1", "Clef"), 1);

        inv.render_hotbar(&mut canvas, 800, 600);

        // Vérifier que les hitboxes des 5 slots ont bien été enregistrées
        for i in 0..HOTBAR_SLOTS_COUNT {
            assert!(inv.hotbar_hitboxes[i].is_some(), "Slot {} must have a valid hitbox", i);
        }
    }
}
