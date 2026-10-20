/**
 * Service Pont Moteur KaractiRage (Engine Bridge)
 * 
 * Assure la communication bidirectionnelle haute performance entre :
 * 1. L'Atelier React 19 / TypeScript (Dark Atelier)
 * 2. Le cœur Rust compilé en WebAssembly (Wasm 60 FPS déterministe)
 * 3. Le backend natif Tauri v2 (IPC natif, fichiers disque, LLM local)
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Valide une recette de jeu (JSON) selon les 6 règles de cohérence de DependencyResolver
 */
export async function validateRecipe(recipeJson: string): Promise<{
  is_valid: boolean;
  resolved_dependencies: string[];
  warnings: string[];
  conflicts: string[];
}> {
  return await invoke('engine_validate_recipe', { recipe_json: recipeJson });
}

/**
 * Instancie une recette de jeu et charge la scène minimale jouable dans le moteur
 */
export async function applyRecipe(recipeJson: string): Promise<number> {
  return await invoke('engine_apply_recipe', { recipe_json: recipeJson });
}

/**
 * Bascule le mode éditeur (Play <-> Edit)
 */
export async function toggleEditorMode(): Promise<number> {
  return await invoke('engine_toggle_editor_mode');
}

/**
 * Ordonne au joueur de naviguer vers une position cible avec évitement d'obstacles A*
 */
export async function movePlayerTo(targetX: number, targetY: number): Promise<boolean> {
  return await invoke('engine_move_player_to', { x: targetX, y: targetY });
}

/**
 * Récupère la position actuelle du joueur
 */
export async function getPlayerPos(): Promise<{ x: number; y: number }> {
  return await invoke('engine_get_player_pos');
}

/**
 * Déclenche une interaction (Touche E)
 */
export async function interact(): Promise<boolean> {
  return await invoke('engine_interact');
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
export async function newBlankProject(title: string, genre: string = "adventure"): Promise<number> {
  return await invoke('engine_new_blank_project', { title, genre });
}

/**
 * Charge un projet KaractiProject complet depuis son format JSON
 */
export async function loadProjectJson(projectJson: string): Promise<number> {
  return await invoke('engine_load_project_json', { project_json: projectJson });
}

/**
 * Exporte l'état complet du projet actuel au format JSON .karacti
 */
export async function exportProjectJson(): Promise<string> {
  return await invoke('engine_export_project_json');
}

/**
 * Récupère en temps réel la liste exacte de toutes les entités du moteur Rust
 */
export async function getEntitiesSummary(): Promise<LiveEntitySummary[]> {
  const raw: string = await invoke('engine_get_entities_summary');
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

/**
 * Met à jour les dimensions et coordonnées d'une entité dans le moteur
 */
export async function updateEntityTransform(id: number, x: number, y: number, w: number, h: number): Promise<boolean> {
  return await invoke('engine_update_entity_transform', { id, x, y, w, h });
}

/**
 * Supprime définitivement une entité du moteur
 */
export async function deleteEntity(id: number): Promise<boolean> {
  return await invoke('engine_delete_entity', { id });
}

/**
 * Instancie une entité préfabriquée dans le monde
 */
export async function spawnEntityPreset(preset: string, x: number, y: number): Promise<number> {
  return await invoke('engine_spawn_entity_preset', { preset, x, y });
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
    getScore: () => invoke('engine_get_score'),
    getStatus: () => invoke('engine_get_status_message'),
  };
}

/**
 * Active ou désactive le mode embarqué (masque le chrome softbuffer natif)
 */
export async function setEmbeddedMode(enabled: boolean): Promise<void> {
  await invoke('engine_set_embedded_mode', { enabled });
}

/**
 * Commute l'outil actif de l'atelier Studio : 0 = Select (V), 1 = Brush (B), 2 = Eraser (X)
 */
export async function setActiveTool(tool: 'select' | 'brush' | 'eraser'): Promise<void> {
  const toolId = tool === 'select' ? 0 : tool === 'brush' ? 1 : 2;
  await invoke('engine_set_active_tool', { tool: toolId });
}

export async function getActiveTool(): Promise<'select' | 'brush' | 'eraser'> {
  const id: number = await invoke('engine_get_active_tool');
  return id === 1 ? 'brush' : id === 2 ? 'eraser' : 'select';
}

export async function cycleBrushTile(): Promise<string> {
  return await invoke('engine_cycle_brush_tile');
}

export async function getActiveBrushTile(): Promise<string> {
  return await invoke('engine_get_active_brush_tile');
}

/**
 * Applique un payload IA (format KARACTI_AI_SPEC.md) directement dans la scène active
 */
export async function applyAiPayload(payloadJson: string, prompt: string): Promise<number> {
  return await invoke('engine_apply_ai_payload', { payload_json: payloadJson, prompt });
}

export async function acceptAiPatch(): Promise<number> {
  return await invoke('engine_accept_ai_patch');
}

export async function rejectAiPatch(): Promise<number> {
  return await invoke('engine_reject_ai_patch');
}

export async function hasPendingAiPatch(): Promise<boolean> {
  const v: number = await invoke('engine_has_pending_ai_patch');
  return v === 1;
}

/**
 * Exporte le jeu courant en fichier HTML5 standalone complet autonome
 */
export async function exportStandaloneHtml(title = "Mon Jeu KaractiRage", author = "Créateur"): Promise<string> {
  return await invoke('engine_export_standalone_html', { title, author });
}

/**
 * Déclenche le téléchargement du package standalone HTML5 dans le navigateur
 */
export async function downloadStandaloneGame(title = "Mon Jeu KaractiRage", author = "Créateur"): Promise<void> {
  const htmlContent = await exportStandaloneHtml(title, author);
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
export async function applyArchetype(archetypeId: string): Promise<boolean> {
  const recipe = getArchetypeRecipe(archetypeId);
  const jsonStr = JSON.stringify(recipe);
  const code = await applyRecipe(jsonStr);
  return code === 0;
}
