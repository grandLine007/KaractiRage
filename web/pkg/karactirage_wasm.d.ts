/* tslint:disable */
/* eslint-disable */

/**
 * An opaque "handle" to platform-dependent audio output device.
 */
export class OutputDevice {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Closes the output device and release all system resources occupied by it. Any calls of this
     * method after the device was closed does nothing.
     */
    close(): void;
}

/**
 * Valide définitivement le patch IA actif (retire les halos violets)
 */
export function wasm_accept_ai_patch(): number;

/**
 * Applique un payload IA (format KARACTI_AI_SPEC.md) directement dans la scène active
 */
export function wasm_apply_ai_payload(payload_json: string, prompt: string): number;

/**
 * Instancie une recette de jeu et charge la scène minimale jouable dans le moteur
 */
export function wasm_apply_recipe(recipe_json: string): number;

/**
 * Fait défiler la tuile active du pinceau et retourne son libellé en français
 */
export function wasm_cycle_brush_tile(): string;

/**
 * Supprime une entité par son ID
 */
export function wasm_delete_entity(id: number): boolean;

/**
 * Exporte le package universel KaractiRage (.karacti.json)
 */
export function wasm_export_game_package(title: string, author: string): string;

/**
 * Exporte l'état complet du jeu actuel en format JSON KaractiProject (.karacti)
 */
export function wasm_export_project_json(): string;

/**
 * Exporte le jeu courant en fichier HTML5 standalone complet autonome
 */
export function wasm_export_standalone_html(title: string, author: string): string;

/**
 * Retourne le nom de la tuile active du pinceau
 */
export function wasm_get_active_brush_tile(): string;

/**
 * Récupère l'outil actif de l'atelier (0 = Select, 1 = Brush, 2 = Eraser)
 */
export function wasm_get_active_tool(): number;

/**
 * Retourne la hauteur native du Canvas (800 px)
 */
export function wasm_get_canvas_height(): number;

/**
 * Retourne la largeur native du Canvas (1200 px)
 */
export function wasm_get_canvas_width(): number;

/**
 * Récupère le temps moyen de calcul CPU par frame en microsecondes
 */
export function wasm_get_cpu_us(): number;

/**
 * Exporte la liste résumée de toutes les entités actives en mémoire au format JSON
 */
export function wasm_get_entities_summary_json(): string;

/**
 * Récupère le FPS mesuré par le module de télémétrie
 */
export function wasm_get_fps(): number;

/**
 * Retourne le pointeur brut vers le buffer de pixels RGBA en mémoire linéaire WebAssembly
 */
export function wasm_get_framebuffer_ptr(): number;

/**
 * Récupère la coordonnée X actuelle du joueur
 */
export function wasm_get_player_pos_x(): number;

/**
 * Récupère la coordonnée Y actuelle du joueur
 */
export function wasm_get_player_pos_y(): number;

/**
 * Récupère le score KVL actuel
 */
export function wasm_get_score(): number;

/**
 * Récupère le nombre de requêtes spatiales de la dernière frame
 */
export function wasm_get_spatial_queries(): number;

/**
 * Récupère le message de statut courant (UTF-8)
 */
export function wasm_get_status_message(): string;

/**
 * Indique si un patch IA est en attente de révision dans le moteur (1 = oui, 0 = non)
 */
export function wasm_has_pending_ai_patch(): number;

/**
 * Importe et instancie un package universel (.karacti.json) dans le moteur
 */
export function wasm_import_game_package(package_json: string): number;

/**
 * Initialise l'instance WebAssembly du moteur KaractiRage
 */
export function wasm_init_engine(): number;

/**
 * Déclenche une interaction (Touche E)
 */
export function wasm_interact(): boolean;

/**
 * Charge un projet KaractiProject complet depuis son format JSON (.karacti)
 */
export function wasm_load_project_json(project_json: string): number;

/**
 * Ordonne au joueur de naviguer vers une position cible avec évitement d'obstacles A*
 */
export function wasm_move_player_to(target_x: number, target_y: number): boolean;

/**
 * Initialise un nouveau projet vierge dans le moteur (Studio direct)
 */
export function wasm_new_blank_project(title: string, genre_str: string): number;

/**
 * Enregistre un appui touche depuis le navigateur
 */
export function wasm_on_key_down(key: number): void;

/**
 * Enregistre un relâchement touche depuis le navigateur
 */
export function wasm_on_key_up(key: number): void;

/**
 * Enregistre un clic de souris (bouton 0: gauche, bouton 2: droit)
 */
export function wasm_on_mouse_down(button: number): void;

/**
 * Enregistre le déplacement de la souris sur le Canvas
 */
export function wasm_on_mouse_move(x: number, y: number): void;

/**
 * Enregistre le relâchement du clic de souris
 */
export function wasm_on_mouse_up(button: number): void;

/**
 * Rejette le patch IA et applique un rollback instantané vers l'état antérieur
 */
export function wasm_reject_ai_patch(): number;

/**
 * Avance la simulation physique d'un pas dt et génère le rendu dans le buffer RGBA
 */
export function wasm_render_frame(dt: number): number;

/**
 * Commute l'outil actif de l'atelier Studio (0 = Select/V, 1 = Brush/B, 2 = Eraser/X)
 */
export function wasm_set_active_tool(tool: number): number;

/**
 * Active ou désactive le mode embarqué (désactive les barres chromées softbuffer pour laisser React gérer le HUD/Studio)
 */
export function wasm_set_embedded_mode(enabled: boolean): void;

/**
 * Instancie une entité selon un préfabriqué (coin, potion, key, chest, door, platform, wall, spikes, boss, crate, plate)
 */
export function wasm_spawn_entity_preset(preset: string, x: number, y: number): number;

/**
 * Bascule le mode éditeur KVL
 */
export function wasm_toggle_editor_mode(): number;

/**
 * Bascule la minicarte radar
 */
export function wasm_toggle_radar(): number;

/**
 * Bascule la grille de partitionnement spatial
 */
export function wasm_toggle_spatial_grid(): number;

/**
 * Met à jour les coordonnées et dimensions d'une entité par son ID
 */
export function wasm_update_entity_transform(id: number, x: number, y: number, w: number, h: number): boolean;

/**
 * Valide une recette de jeu (JSON) et résout automatiquement ses dépendances
 */
export function wasm_validate_recipe(recipe_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_outputdevice_free: (a: number, b: number) => void;
    readonly outputdevice_close: (a: number) => void;
    readonly wasm_accept_ai_patch: () => number;
    readonly wasm_apply_ai_payload: (a: number, b: number, c: number, d: number) => number;
    readonly wasm_apply_recipe: (a: number, b: number) => number;
    readonly wasm_cycle_brush_tile: () => [number, number];
    readonly wasm_delete_entity: (a: number) => number;
    readonly wasm_export_game_package: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasm_export_project_json: () => [number, number];
    readonly wasm_export_standalone_html: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasm_get_active_brush_tile: () => [number, number];
    readonly wasm_get_active_tool: () => number;
    readonly wasm_get_canvas_height: () => number;
    readonly wasm_get_canvas_width: () => number;
    readonly wasm_get_cpu_us: () => number;
    readonly wasm_get_entities_summary_json: () => [number, number];
    readonly wasm_get_fps: () => number;
    readonly wasm_get_framebuffer_ptr: () => number;
    readonly wasm_get_player_pos_x: () => number;
    readonly wasm_get_player_pos_y: () => number;
    readonly wasm_get_score: () => number;
    readonly wasm_get_spatial_queries: () => number;
    readonly wasm_get_status_message: () => [number, number];
    readonly wasm_has_pending_ai_patch: () => number;
    readonly wasm_import_game_package: (a: number, b: number) => number;
    readonly wasm_init_engine: () => number;
    readonly wasm_interact: () => number;
    readonly wasm_load_project_json: (a: number, b: number) => number;
    readonly wasm_move_player_to: (a: number, b: number) => number;
    readonly wasm_new_blank_project: (a: number, b: number, c: number, d: number) => number;
    readonly wasm_on_key_down: (a: number) => void;
    readonly wasm_on_key_up: (a: number) => void;
    readonly wasm_on_mouse_down: (a: number) => void;
    readonly wasm_on_mouse_move: (a: number, b: number) => void;
    readonly wasm_on_mouse_up: (a: number) => void;
    readonly wasm_reject_ai_patch: () => number;
    readonly wasm_render_frame: (a: number) => number;
    readonly wasm_set_active_tool: (a: number) => number;
    readonly wasm_set_embedded_mode: (a: number) => void;
    readonly wasm_spawn_entity_preset: (a: number, b: number, c: number, d: number) => number;
    readonly wasm_toggle_editor_mode: () => number;
    readonly wasm_toggle_radar: () => number;
    readonly wasm_toggle_spatial_grid: () => number;
    readonly wasm_update_entity_transform: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasm_validate_recipe: (a: number, b: number) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__hf89d7653fac0de47: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
