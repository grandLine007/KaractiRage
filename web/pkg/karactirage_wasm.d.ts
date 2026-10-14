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
 * Récupère le FPS mesuré par le module de télémétrie
 */
export function wasm_get_fps(): number;

/**
 * Retourne le pointeur brut vers le buffer de pixels RGBA en mémoire linéaire WebAssembly
 */
export function wasm_get_framebuffer_ptr(): number;

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
 * Initialise l'instance WebAssembly du moteur KaractiRage
 */
export function wasm_init_engine(): number;

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
 * Avance la simulation physique d'un pas dt et génère le rendu dans le buffer RGBA
 */
export function wasm_render_frame(dt: number): number;

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

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_outputdevice_free: (a: number, b: number) => void;
    readonly outputdevice_close: (a: number) => void;
    readonly wasm_get_canvas_height: () => number;
    readonly wasm_get_canvas_width: () => number;
    readonly wasm_get_cpu_us: () => number;
    readonly wasm_get_fps: () => number;
    readonly wasm_get_framebuffer_ptr: () => number;
    readonly wasm_get_score: () => number;
    readonly wasm_get_spatial_queries: () => number;
    readonly wasm_get_status_message: () => [number, number];
    readonly wasm_init_engine: () => number;
    readonly wasm_on_key_down: (a: number) => void;
    readonly wasm_on_key_up: (a: number) => void;
    readonly wasm_on_mouse_down: (a: number) => void;
    readonly wasm_on_mouse_move: (a: number, b: number) => void;
    readonly wasm_on_mouse_up: (a: number) => void;
    readonly wasm_render_frame: (a: number) => number;
    readonly wasm_toggle_editor_mode: () => number;
    readonly wasm_toggle_radar: () => number;
    readonly wasm_toggle_spatial_grid: () => number;
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
