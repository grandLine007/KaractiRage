/**
 * Service Pont Moteur KaractiRage (Engine Bridge)
 * 
 * Assure la communication bidirectionnelle haute performance entre :
 * 1. L'Atelier React 19 / TypeScript (Dark Atelier)
 * 2. Le cœur Rust compilé en WebAssembly (Wasm 60 FPS déterministe)
 * 3. Le backend natif Tauri v2 (IPC natif, fichiers disque, LLM local)
 */

// @ts-ignore
import * as wasmRawBindings from '../wasm/karactirage_wasm.js';

let activeBindings: any = wasmRawBindings;

export function setWasmBindings(bindings: any) {
  activeBindings = bindings;
}

export function getWasmBindings(): any {
  return activeBindings;
}

export function isEngineReady(): boolean {
  return !!activeBindings && typeof activeBindings.wasm_render_frame === 'function';
}

/**
 * Valide une recette de jeu (JSON) selon les 6 règles de cohérence de DependencyResolver
 */
export function validateRecipe(recipeJson: string): {
  is_valid: boolean;
  resolved_dependencies: string[];
  warnings: string[];
  conflicts: string[];
} {
  if (!activeBindings?.wasm_validate_recipe) {
    return { is_valid: false, resolved_dependencies: [], warnings: [], conflicts: ["Moteur Wasm non initialisé"] };
  }
  try {
    const raw = activeBindings.wasm_validate_recipe(recipeJson);
    return JSON.parse(raw);
  } catch (err: any) {
    return { is_valid: false, resolved_dependencies: [], warnings: [], conflicts: [err?.message || "Erreur parsing JSON"] };
  }
}

/**
 * Instancie une recette de jeu et charge la scène minimale jouable dans le moteur
 */
export function applyRecipe(recipeJson: string): number {
  if (!activeBindings?.wasm_apply_recipe) return -99;
  return activeBindings.wasm_apply_recipe(recipeJson);
}

/**
 * Bascule le mode éditeur (Play <-> Edit)
 */
export function toggleEditorMode(): number {
  if (!activeBindings?.wasm_toggle_editor_mode) return 0;
  return activeBindings.wasm_toggle_editor_mode();
}

/**
 * Ordonne au joueur de naviguer vers une position cible avec évitement d'obstacles A*
 */
export function movePlayerTo(targetX: number, targetY: number): boolean {
  if (!activeBindings?.wasm_move_player_to) return false;
  return activeBindings.wasm_move_player_to(targetX, targetY);
}

/**
 * Récupère la position actuelle du joueur
 */
export function getPlayerPos(): { x: number; y: number } {
  if (!activeBindings?.wasm_get_player_pos_x || !activeBindings?.wasm_get_player_pos_y) {
    return { x: 0, y: 0 };
  }
  return {
    x: activeBindings.wasm_get_player_pos_x(),
    y: activeBindings.wasm_get_player_pos_y(),
  };
}

/**
 * Déclenche une interaction (Touche E)
 */
export function interact(): boolean {
  if (!activeBindings?.wasm_interact) return false;
  return activeBindings.wasm_interact();
}

export interface LiveEntitySummary {
  id: number;
  name: string;
  tag: string;
  x: number;
  y: number;
  w: number;
  h: number;
  health?: number;
  max_health?: number;
  speed?: number;
  movement_mode?: string;
  is_solid: boolean;
  is_trigger: boolean;
  is_pushable: boolean;
  is_active: boolean;
}

/**
 * Initialise un projet vierge avec le Studio immédiatement ouvert
 */
export function newBlankProject(title: string, genre: string = "adventure"): number {
  if (!activeBindings?.wasm_new_blank_project) return -99;
  return activeBindings.wasm_new_blank_project(title, genre);
}

/**
 * Charge un projet KaractiProject complet depuis son format JSON
 */
export function loadProjectJson(projectJson: string): number {
  if (!activeBindings?.wasm_load_project_json) return -99;
  return activeBindings.wasm_load_project_json(projectJson);
}

/**
 * Exporte l'état complet du projet actuel au format JSON .karacti
 */
export function exportProjectJson(): string {
  if (!activeBindings?.wasm_export_project_json) return "{}";
  return activeBindings.wasm_export_project_json();
}

/**
 * Récupère en temps réel la liste exacte de toutes les entités du moteur Rust
 */
export function getEntitiesSummary(): LiveEntitySummary[] {
  if (!activeBindings?.wasm_get_entities_summary_json) return [];
  try {
    const raw = activeBindings.wasm_get_entities_summary_json();
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

/**
 * Met à jour les dimensions et coordonnées d'une entité dans le moteur
 */
export function updateEntityTransform(id: number, x: number, y: number, w: number, h: number): boolean {
  if (!activeBindings?.wasm_update_entity_transform) return false;
  return activeBindings.wasm_update_entity_transform(id, x, y, w, h);
}

/**
 * Supprime définitivement une entité du moteur
 */
export function deleteEntity(id: number): boolean {
  if (!activeBindings?.wasm_delete_entity) return false;
  return activeBindings.wasm_delete_entity(id);
}

/**
 * Instancie une entité préfabriquée dans le monde
 */
export function spawnEntityPreset(preset: string, x: number, y: number): number {
  if (!activeBindings?.wasm_spawn_entity_preset) return 0;
  return activeBindings.wasm_spawn_entity_preset(preset, x, y);
}

// Exposition sur l'objet global window pour pilotage externe et tests
if (typeof window !== 'undefined') {
  (window as any).karactiEngine = {
    movePlayerTo,
    getPlayerPos,
    interact,
    applyArchetype,
    newBlankProject,
    loadProjectJson,
    exportProjectJson,
    getEntitiesSummary,
    updateEntityTransform,
    deleteEntity,
    spawnEntityPreset,
    toggleEditorMode,
    downloadStandaloneGame,
    getScore: () => activeBindings?.wasm_get_score ? activeBindings.wasm_get_score() : 0,
    getStatus: () => activeBindings?.wasm_get_status_message ? activeBindings.wasm_get_status_message() : '',
  };
}

/**
 * Active ou désactive le mode embarqué (masque le chrome softbuffer natif)
 */
export function setEmbeddedMode(enabled: boolean): void {
  if (typeof activeBindings?.wasm_set_embedded_mode === 'function') {
    activeBindings.wasm_set_embedded_mode(enabled);
  }
}

/**
 * Commute l'outil actif de l'atelier Studio : 0 = Select (V), 1 = Brush (B), 2 = Eraser (X)
 */
export function setActiveTool(tool: 'select' | 'brush' | 'eraser'): void {
  if (!activeBindings?.wasm_set_active_tool) return;
  const toolId = tool === 'select' ? 0 : tool === 'brush' ? 1 : 2;
  activeBindings.wasm_set_active_tool(toolId);
}

export function getActiveTool(): 'select' | 'brush' | 'eraser' {
  if (!activeBindings?.wasm_get_active_tool) return 'select';
  const id = activeBindings.wasm_get_active_tool();
  return id === 1 ? 'brush' : id === 2 ? 'eraser' : 'select';
}

export function cycleBrushTile(): string {
  if (!activeBindings?.wasm_cycle_brush_tile) return '';
  return activeBindings.wasm_cycle_brush_tile();
}

export function getActiveBrushTile(): string {
  if (!activeBindings?.wasm_get_active_brush_tile) return 'Mur solide';
  return activeBindings.wasm_get_active_brush_tile();
}

/**
 * Applique un payload IA (format KARACTI_AI_SPEC.md) directement dans la scène active
 */
export function applyAiPayload(payloadJson: string, prompt: string): number {
  if (!activeBindings?.wasm_apply_ai_payload) return -99;
  return activeBindings.wasm_apply_ai_payload(payloadJson, prompt);
}

export function acceptAiPatch(): number {
  if (!activeBindings?.wasm_accept_ai_patch) return -99;
  return activeBindings.wasm_accept_ai_patch();
}

export function rejectAiPatch(): number {
  if (!activeBindings?.wasm_reject_ai_patch) return -99;
  return activeBindings.wasm_reject_ai_patch();
}

export function hasPendingAiPatch(): boolean {
  if (!activeBindings?.wasm_has_pending_ai_patch) return false;
  return activeBindings.wasm_has_pending_ai_patch() === 1;
}

/**
 * Exporte le jeu courant en fichier HTML5 standalone complet autonome
 */
export function exportStandaloneHtml(title = "Mon Jeu KaractiRage", author = "Créateur"): string {
  if (!activeBindings?.wasm_export_standalone_html) return '';
  return activeBindings.wasm_export_standalone_html(title, author);
}

/**
 * Déclenche le téléchargement du package standalone HTML5 dans le navigateur
 */
export function downloadStandaloneGame(title = "Mon Jeu KaractiRage", author = "Créateur"): void {
  const htmlContent = exportStandaloneHtml(title, author);
  if (!htmlContent) {
    alert("Erreur lors de la génération du standalone HTML5.");
    return;
  }

  const blob = new Blob([htmlContent], { type: 'text/html;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${title.toLowerCase().replace(/[^a-z0-9]/g, '_')}_standalone.html`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

/**
 * Générateur des 5 Archétypes Spécifiés dans `systeme_de_mode_de_jeu.md`
 */
export function getArchetypeRecipe(archetypeId: string): any {
  switch (archetypeId) {
    case 'rpg':
      return {
        schema_version: 1,
        title: "Donjon RPG — La Crypte Émeraude",
        author: "Atelier KaractiRage",
        genres: { primary: "adventure", secondary: ["rpg", "action"] },
        dimension: "two_d",
        camera: "follow_player",
        player: {
          name: "Paladin de Jade",
          directions: "eight_directions",
          abilities: { jump: false, double_jump: false, run: true, crouch: false, dash: true, climb: false, swim: false, teleport: false, interact: true },
          combat: "melee",
          max_health: 8,
          speed: 190
        },
        world: {
          gravity_enabled: false,
          day_night_cycle: false,
          dynamic_lighting: false,
          interactive_elements: ["door", "chest", "collectible", "lever", "trap"]
        },
        gameplay: {
          inventory: true,
          dialogues: true,
          quest_objectives: true,
          save_system: true,
          score_system: true,
          boss_fight: false,
          wave_spawner: false
        },
        performance: "low_end"
      };

    case 'platformer':
      return {
        schema_version: 1,
        title: "Plateforme & Action — L'Épreuve du Ninja",
        author: "Atelier KaractiRage",
        genres: { primary: "platformer", secondary: ["action"] },
        dimension: "two_d",
        camera: "follow_player",
        player: {
          name: "Ninja Shinobi",
          directions: "horizontal_only",
          abilities: { jump: true, double_jump: true, run: true, crouch: true, dash: true, climb: false, swim: false, teleport: false, interact: true },
          combat: "melee",
          max_health: 5,
          speed: 220
        },
        world: {
          gravity_enabled: true,
          day_night_cycle: false,
          dynamic_lighting: false,
          interactive_elements: ["collectible", "door", "trap", "moving_platform"]
        },
        gameplay: {
          inventory: false,
          dialogues: false,
          quest_objectives: true,
          save_system: true,
          score_system: true,
          boss_fight: false,
          wave_spawner: false
        },
        performance: "low_end"
      };

    case 'shmup':
      return {
        schema_version: 1,
        title: "Shmup Spatial — Alerte Nébuleuse",
        author: "Atelier KaractiRage",
        genres: { primary: "shoot_em_up", secondary: ["action"] },
        dimension: "two_d",
        camera: "auto_scroll_vertical",
        player: {
          name: "Vaisseau Stellaire",
          directions: "free_inertia",
          abilities: { jump: false, double_jump: false, run: true, crouch: false, dash: true, climb: false, swim: false, teleport: false, interact: false },
          combat: "projectiles",
          max_health: 4,
          speed: 260
        },
        world: {
          gravity_enabled: false,
          day_night_cycle: false,
          dynamic_lighting: false,
          interactive_elements: ["collectible", "trap"]
        },
        gameplay: {
          inventory: false,
          dialogues: false,
          quest_objectives: true,
          save_system: true,
          score_system: true,
          boss_fight: true,
          wave_spawner: true
        },
        performance: "low_end"
      };

    case 'sokoban':
      return {
        schema_version: 1,
        title: "Puzzle & Énigmes — Le Labyrinthe Quantique",
        author: "Atelier KaractiRage",
        genres: { primary: "puzzle", secondary: ["strategy"] },
        dimension: "two_d",
        camera: "fixed",
        player: {
          name: "Ingénieur Logique",
          directions: "grid",
          abilities: { jump: false, double_jump: false, run: false, crouch: false, dash: false, climb: false, swim: false, teleport: false, interact: true },
          combat: "none",
          max_health: 1,
          speed: 140
        },
        world: {
          gravity_enabled: false,
          day_night_cycle: false,
          dynamic_lighting: false,
          interactive_elements: ["pressure_plate", "door", "collectible", "breakable_wall"]
        },
        gameplay: {
          inventory: false,
          dialogues: true,
          quest_objectives: true,
          save_system: true,
          score_system: true,
          boss_fight: false,
          wave_spawner: false
        },
        performance: "low_end"
      };

    case 'boss':
    default:
      return {
        schema_version: 1,
        title: "Titan Arène — Duel contre le Colosse d'Obsidienne",
        author: "Atelier KaractiRage",
        genres: { primary: "combat", secondary: ["action", "adventure"] },
        dimension: "two_d",
        camera: "follow_player",
        player: {
          name: "Chasseur de Titans",
          directions: "eight_directions",
          abilities: { jump: true, double_jump: false, run: true, crouch: false, dash: true, climb: false, swim: false, teleport: false, interact: true },
          combat: "hybrid",
          max_health: 10,
          speed: 200
        },
        world: {
          gravity_enabled: false,
          day_night_cycle: false,
          dynamic_lighting: false,
          interactive_elements: ["door", "trap", "collectible"]
        },
        gameplay: {
          inventory: true,
          dialogues: false,
          quest_objectives: true,
          save_system: true,
          score_system: true,
          boss_fight: true,
          wave_spawner: false
        },
        performance: "low_end"
      };
  }
}

/**
 * Lance un des 5 archétypes directement dans le moteur
 */
export function applyArchetype(archetypeId: string): boolean {
  const recipe = getArchetypeRecipe(archetypeId);
  const jsonStr = JSON.stringify(recipe);
  const code = applyRecipe(jsonStr);
  return code === 0;
}
