/**
 * Service Pont Moteur KaractiRage (Engine Bridge)
 * 
 * Assure la communication bidirectionnelle haute performance entre :
 * 1. L'Atelier React 19 / TypeScript (Dark Atelier)
 * 2. Le cœur Rust compilé en WebAssembly (Wasm 60 FPS déterministe)
 * 3. Le backend natif Tauri v2 (IPC natif, fichiers disque, LLM local)
 */

import { invoke } from '@tauri-apps/api/core';

export interface EngineBootStatus {
  state: 'booting' | 'ready' | 'failed';
  error: string | null;
  init_ms: number;
}

/**
 * État du boot du moteur natif (non bloquant — le boot tourne en arrière-plan).
 */
export async function getEngineStatus(): Promise<EngineBootStatus> {
  return await invoke('engine_status');
}

/**
 * Relance l'initialisation du moteur après un échec (retourne false si déjà prêt).
 */
export async function retryEngineBoot(): Promise<boolean> {
  return await invoke('engine_retry_boot');
}

/**
 * Valide une recette de jeu (JSON) selon les 6 règles de cohérence de DependencyResolver
 */
export async function validateRecipe(recipeJson: string): Promise<{
  is_valid: boolean;
  resolved_dependencies: string[];
  warnings: string[];
  conflicts: string[];
}> {
  return await invoke('engine_validate_recipe', { recipeJson });
}

/**
 * Instancie une recette de jeu et charge la scène minimale jouable dans le moteur
 */
export async function applyRecipe(recipeJson: string): Promise<number> {
  return await invoke('engine_apply_recipe', { recipeJson });
}

/**
 * Lance le jeu de référence (scène officielle ciselée) d'un archétype en 1 clic.
 * Renvoie le nom réel de la scène chargée.
 */
export async function launchArchetypeGame(archetypeId: string): Promise<string> {
  return await invoke('engine_launch_archetype', { archetypeId });
}

/**
 * Lance la démo 3D thématique d'un archétype (scène procédurale dédiée).
 * Renvoie le nombre d'entités 3D générées.
 */
export async function launchArchetypeGame3D(archetypeId: string): Promise<number> {
  return await invoke('engine_launch_archetype_3d', { archetypeId });
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
  return await invoke('engine_load_project_json', { projectJson });
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
 * Met à jour les dimensions et coordonnées d'une entité dans le moteur.
 * `recordHistory` pousse une commande d'annulation (un seul commit Ctrl+Z).
 */
export async function updateEntityTransform(
  id: number,
  x: number,
  y: number,
  w: number,
  h: number,
  recordHistory: boolean = true,
): Promise<boolean> {
  return await invoke('engine_update_entity_transform', { id, x, y, w, h, recordHistory });
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

// ---------------------------------------------------------------------------
// Atelier connecté — état d'édition temps réel, sélection, outils et historique
// ---------------------------------------------------------------------------

export interface StudioEntitySummary {
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
  script_path?: string | null;
  has_movement: boolean;
  has_vision: boolean;
  vision_range?: number;
  is_player_controlled: boolean;
}

export interface BrushTileInfo {
  id: string;
  label: string;
  short: string;
  color: string;
}

export interface StudioEditorState {
  entities: StudioEntitySummary[];
  selected_id: number | null;
  tool: 'select' | 'brush' | 'eraser';
  tile: string;
  tile_label: string;
  can_undo: boolean;
  can_redo: boolean;
  status: string;
  mode: 'edit' | 'play';
}

/**
 * Récupère un instantané complet de l'éditeur (entités + sélection + outil + tuile).
 */
export async function getEditorState(): Promise<StudioEditorState> {
  const raw: string = await invoke('engine_editor_state');
  try {
    return JSON.parse(raw);
  } catch {
    return {
      entities: [],
      selected_id: null,
      tool: 'select',
      tile: 'wall',
      tile_label: 'Mur Solide',
      can_undo: false,
      can_redo: false,
      status: '',
      mode: 'edit',
    };
  }
}

export async function selectEntity(id: number): Promise<boolean> {
  return await invoke('engine_select_entity', { id });
}

export async function getSelectedEntity(): Promise<number | null> {
  return await invoke('engine_get_selected_entity');
}

export async function deleteSelectedEntity(): Promise<boolean> {
  return await invoke('engine_delete_selected_entity');
}

export async function duplicateSelectedEntity(): Promise<number | null> {
  return await invoke('engine_duplicate_selected_entity');
}

export async function undo(): Promise<boolean> {
  return await invoke('engine_undo');
}

export async function redo(): Promise<boolean> {
  return await invoke('engine_redo');
}

export async function setEditorMode(mode: 'edit' | 'play'): Promise<number> {
  return await invoke('engine_set_editor_mode', { mode: mode === 'edit' ? 1 : 0 });
}

export async function setBrushTile(tileId: string): Promise<string> {
  return await invoke('engine_set_brush_tile', { tileId });
}

export async function getBrushTiles(): Promise<BrushTileInfo[]> {
  const raw: string = await invoke('engine_get_brush_tiles');
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

/**
 * Instancie un préfabriqué à une position écran (coordonnées canvas).
 */
export async function spawnEntityAtScreen(preset: string, sx: number, sy: number): Promise<number> {
  return await invoke('engine_spawn_entity_at_screen', { preset, sx, sy });
}

export async function saveScene(): Promise<boolean> {
  return await invoke('engine_save_scene');
}

// ---------------------------------------------------------------------------
// Sprites pixel-art (pxl) — liste, miniatures et spawn
// ---------------------------------------------------------------------------

export interface SpriteAsset {
  id: string;
  file: string;
  w: number;
  h: number;
}

/** Liste des sprites pixel-art disponibles (assets/generated/sprites/*.png). */
export async function listSpriteAssets(): Promise<SpriteAsset[]> {
  const raw: string = await invoke('engine_list_sprite_assets');
  try {
    return JSON.parse(raw) as SpriteAsset[];
  } catch {
    return [];
  }
}

export interface MeshAssetSummary {
  id: string;
  kind: 'procedural' | 'gltf';
  parts: number;
}

/** Liste des assets de mesh 3D (manifest procédural + références glTF). */
export async function listMeshAssets(): Promise<MeshAssetSummary[]> {
  const raw: string = await invoke('engine_list_mesh_assets');
  try {
    return JSON.parse(raw) as MeshAssetSummary[];
  } catch {
    return [];
  }
}

/** Instancie un preset 3D (primitive ou asset du manifest) à une position. */
export async function spawn3dPreset(preset: string, x: number, y: number, z: number): Promise<number> {
  return await invoke<number>('engine_spawn_3d_preset', { preset, x, y, z });
}

/** Octets PNG bruts d'un sprite (miniature). */
export async function getSpritePngBytes(id: string): Promise<ArrayBuffer> {
  return await invoke<ArrayBuffer>('engine_sprite_png', { id });
}

const spriteUrlCache = new Map<string, string>();

/** URL objet (Blob) pour afficher un sprite dans une balise `<img>`. */
export async function getSpritePngUrl(id: string): Promise<string> {
  const hit = spriteUrlCache.get(id);
  if (hit) return hit;
  const bytes = await getSpritePngBytes(id);
  const blob = new Blob([bytes], { type: 'image/png' });
  const url = URL.createObjectURL(blob);
  spriteUrlCache.set(id, url);
  return url;
}

export interface AnimAsset {
  id: string;
  file: string;
  frames: number;
}

export interface SheetAsset {
  id: string;
  file: string;
  frames: string[];
}

/** Liste des animations pxl (assets/generated/anims/*.json). */
export async function listAnimAssets(): Promise<AnimAsset[]> {
  const raw: string = await invoke('engine_list_anim_assets');
  try {
    return JSON.parse(raw) as AnimAsset[];
  } catch {
    return [];
  }
}

/** Liste des atlas pxl (assets/generated/<sheet>.json). */
export async function listSheetAssets(): Promise<SheetAsset[]> {
  const raw: string = await invoke('engine_list_sheet_assets');
  try {
    return JSON.parse(raw) as SheetAsset[];
  } catch {
    return [];
  }
}

/** Octets PNG bruts d'un atlas / bande d'animation (miniature). */
export async function getSheetPngBytes(id: string): Promise<ArrayBuffer> {
  return await invoke<ArrayBuffer>('engine_sheet_png', { id });
}

const sheetUrlCache = new Map<string, string>();

/** URL objet (Blob) pour afficher un atlas ou une bande d'animation. */
export async function getSheetPngUrl(id: string): Promise<string> {
  const hit = sheetUrlCache.get(id);
  if (hit) return hit;
  const bytes = await getSheetPngBytes(id);
  const blob = new Blob([bytes], { type: 'image/png' });
  const url = URL.createObjectURL(blob);
  sheetUrlCache.set(id, url);
  return url;
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Atelier Pixel Art (pxl) — build + viewer live
// ---------------------------------------------------------------------------

export interface PxlBuildResult {
  ok: boolean;
  stdout: string;
  stderr: string;
  duration_ms: number;
}

export interface PxlServeStatus {
  running: boolean;
  url: string | null;
  port: number;
}

export async function pxlAvailable(): Promise<boolean> {
  return await invoke<boolean>('pxl_available');
}

export async function pxlBuild(name: string): Promise<PxlBuildResult> {
  return await invoke('pxl_build', { name });
}

export async function pxlServeStart(name: string, port: number): Promise<string> {
  return await invoke('pxl_serve_start', { name, port });
}

export async function pxlServeStop(): Promise<boolean> {
  return await invoke('pxl_serve_stop');
}

export async function pxlServeStatus(port: number): Promise<PxlServeStatus> {
  return await invoke('pxl_serve_status', { port });
}

/** Liste les sources d'art pxl disponibles (assets/art/*.px.js). */
export async function pxlListSources(): Promise<string[]> {
  return await invoke<string[]>('pxl_list_sources');
}

/** Lit le contenu d'une source d'art pxl. */
export async function pxlReadSource(name: string): Promise<string> {
  return await invoke<string>('pxl_read_source', { name });
}

/** Écrit (ou crée) une source d'art pxl. Retourne le chemin absolu écrit. */
export async function pxlWriteSource(name: string, content: string): Promise<string> {
  return await invoke<string>('pxl_write_source', { name, content });
}

// KVL Guidé — graphes, validation et recettes en un clic
// ---------------------------------------------------------------------------

export interface KvlGraphSummary {
  path: string;
  id: string;
  name: string;
  description: string;
  node_count: number;
  connection_count: number;
}

export interface KvlDiagnostic {
  severity: 'error' | 'warning';
  message: string;
  node_id: string | null;
  pin_id: string | null;
}

export interface GuidedGraphResult {
  ok: boolean;
  error?: string;
  script_path?: string;
  name?: string;
  description?: string;
  summary?: string;
  trigger_event?: string;
  attached?: boolean;
  target_entity?: string | null;
  node_count?: number;
  diagnostics?: KvlDiagnostic[];
  graph_json?: string;
}

export async function getKvlGraphs(): Promise<KvlGraphSummary[]> {
  const raw: string = await invoke('engine_get_kvl_graphs');
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

export async function getKvlGraph(path: string): Promise<string | null> {
  const raw: string = await invoke('engine_get_kvl_graph', { path });
  return raw === 'null' ? null : raw;
}

export async function validateKvlGraph(json: string): Promise<KvlDiagnostic[]> {
  const raw: string = await invoke('engine_validate_kvl_graph', { json });
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

export async function saveKvlGraph(path: string, json: string): Promise<boolean> {
  return await invoke('engine_save_kvl_graph', { path, json });
}

/** Schéma d'un type de nœud exposé par le catalogue de plugins du moteur. */
export interface KvlNodeSchema {
  id: string;
  flavor: 'event' | 'condition' | 'action' | 'composite';
  label: string;
  category: unknown;
  description: string;
  params: Array<{ name: string; label: string; param_type: unknown; default?: unknown }>;
  aliases: string[];
  prefixes: string[];
  default_argument: string;
  flow_input: boolean;
  flow_outputs: string[];
  plugin: string | null;
}

export interface PluginCatalog {
  api_version: number;
  plugin_count: number;
  node_count: number;
  load_order: string[];
  plugins: unknown[];
  nodes: KvlNodeSchema[];
  issues: Array<{ plugin: string; kind: string; message: string }>;
}

/** Catalogue des plugins et des nœuds (page Extensions / palette KVL). */
export async function getPluginCatalog(): Promise<PluginCatalog | null> {
  try {
    const raw: string = await invoke('engine_plugin_catalog');
    return JSON.parse(raw) as PluginCatalog;
  } catch {
    return null;
  }
}

/** Insère un nœud du catalogue dans un graphe et renvoie le graphe sérialisé. */
export async function addKvlNode(path: string, typeId: string, x: number, y: number): Promise<string> {
  return await invoke('engine_kvl_add_node', { path, typeId, x, y });
}

/**
 * Construit et attache un graphe KVL guidé à partir d'une recette + paramètres.
 */
export async function buildGuidedGraph(recipe: string, params: Record<string, unknown>): Promise<GuidedGraphResult> {
  const raw: string = await invoke('engine_build_guided_graph', { recipe, params });
  try {
    return JSON.parse(raw);
  } catch {
    return { ok: false, error: 'Réponse moteur illisible.' };
  }
}

export async function attachScript(entityId: number, scriptPath: string): Promise<boolean> {
  return await invoke('engine_attach_script', { entityId, scriptPath });
}

// Exposition sur l'objet global window pour pilotage externe et tests
if (typeof window !== 'undefined') {
  (window as any).karactiEngine = {
    movePlayerTo,
    getPlayerPos,
    interact,
    applyArchetype,
    launchArchetypeGame,
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
  return await invoke('engine_apply_ai_payload', { payloadJson, prompt });
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
