/* @ts-self-types="./karactirage_wasm.d.ts" */

/**
 * An opaque "handle" to platform-dependent audio output device.
 */
export class OutputDevice {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        OutputDeviceFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_outputdevice_free(ptr, 0);
    }
    /**
     * Closes the output device and release all system resources occupied by it. Any calls of this
     * method after the device was closed does nothing.
     */
    close() {
        wasm.outputdevice_close(this.__wbg_ptr);
    }
}
if (Symbol.dispose) OutputDevice.prototype[Symbol.dispose] = OutputDevice.prototype.free;

/**
 * Valide définitivement le patch IA actif (retire les halos violets)
 * @returns {number}
 */
export function wasm_accept_ai_patch() {
    const ret = wasm.wasm_accept_ai_patch();
    return ret;
}

/**
 * Applique un payload IA (format KARACTI_AI_SPEC.md) directement dans la scène active
 * @param {string} payload_json
 * @param {string} prompt
 * @returns {number}
 */
export function wasm_apply_ai_payload(payload_json, prompt) {
    const ptr0 = passStringToWasm0(payload_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(prompt, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.wasm_apply_ai_payload(ptr0, len0, ptr1, len1);
    return ret;
}

/**
 * Instancie une recette de jeu et charge la scène minimale jouable dans le moteur
 * @param {string} recipe_json
 * @returns {number}
 */
export function wasm_apply_recipe(recipe_json) {
    const ptr0 = passStringToWasm0(recipe_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.wasm_apply_recipe(ptr0, len0);
    return ret;
}

/**
 * Fait défiler la tuile active du pinceau et retourne son libellé en français
 * @returns {string}
 */
export function wasm_cycle_brush_tile() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.wasm_cycle_brush_tile();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Supprime une entité par son ID
 * @param {number} id
 * @returns {boolean}
 */
export function wasm_delete_entity(id) {
    const ret = wasm.wasm_delete_entity(id);
    return ret !== 0;
}

/**
 * Exporte le package universel KaractiRage (.karacti.json)
 * @param {string} title
 * @param {string} author
 * @returns {string}
 */
export function wasm_export_game_package(title, author) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(author, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasm_export_game_package(ptr0, len0, ptr1, len1);
        deferred3_0 = ret[0];
        deferred3_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Exporte l'état complet du jeu actuel en format JSON KaractiProject (.karacti)
 * @returns {string}
 */
export function wasm_export_project_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.wasm_export_project_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Exporte le jeu courant en fichier HTML5 standalone complet autonome
 * @param {string} title
 * @param {string} author
 * @returns {string}
 */
export function wasm_export_standalone_html(title, author) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(author, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasm_export_standalone_html(ptr0, len0, ptr1, len1);
        deferred3_0 = ret[0];
        deferred3_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Retourne le nom de la tuile active du pinceau
 * @returns {string}
 */
export function wasm_get_active_brush_tile() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.wasm_get_active_brush_tile();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Récupère l'outil actif de l'atelier (0 = Select, 1 = Brush, 2 = Eraser)
 * @returns {number}
 */
export function wasm_get_active_tool() {
    const ret = wasm.wasm_get_active_tool();
    return ret >>> 0;
}

/**
 * Retourne la hauteur native du Canvas (800 px)
 * @returns {number}
 */
export function wasm_get_canvas_height() {
    const ret = wasm.wasm_get_canvas_height();
    return ret >>> 0;
}

/**
 * Retourne la largeur native du Canvas (1200 px)
 * @returns {number}
 */
export function wasm_get_canvas_width() {
    const ret = wasm.wasm_get_canvas_width();
    return ret >>> 0;
}

/**
 * Récupère le temps moyen de calcul CPU par frame en microsecondes
 * @returns {number}
 */
export function wasm_get_cpu_us() {
    const ret = wasm.wasm_get_cpu_us();
    return ret >>> 0;
}

/**
 * Exporte la liste résumée de toutes les entités actives en mémoire au format JSON
 * @returns {string}
 */
export function wasm_get_entities_summary_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.wasm_get_entities_summary_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Récupère le FPS mesuré par le module de télémétrie
 * @returns {number}
 */
export function wasm_get_fps() {
    const ret = wasm.wasm_get_fps();
    return ret;
}

/**
 * Retourne le pointeur brut vers le buffer de pixels RGBA en mémoire linéaire WebAssembly
 * @returns {number}
 */
export function wasm_get_framebuffer_ptr() {
    const ret = wasm.wasm_get_framebuffer_ptr();
    return ret >>> 0;
}

/**
 * Récupère la coordonnée X actuelle du joueur
 * @returns {number}
 */
export function wasm_get_player_pos_x() {
    const ret = wasm.wasm_get_player_pos_x();
    return ret;
}

/**
 * Récupère la coordonnée Y actuelle du joueur
 * @returns {number}
 */
export function wasm_get_player_pos_y() {
    const ret = wasm.wasm_get_player_pos_y();
    return ret;
}

/**
 * Récupère le score KVL actuel
 * @returns {number}
 */
export function wasm_get_score() {
    const ret = wasm.wasm_get_score();
    return ret;
}

/**
 * Récupère le nombre de requêtes spatiales de la dernière frame
 * @returns {number}
 */
export function wasm_get_spatial_queries() {
    const ret = wasm.wasm_get_spatial_queries();
    return ret >>> 0;
}

/**
 * Récupère le message de statut courant (UTF-8)
 * @returns {string}
 */
export function wasm_get_status_message() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.wasm_get_status_message();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Indique si un patch IA est en attente de révision dans le moteur (1 = oui, 0 = non)
 * @returns {number}
 */
export function wasm_has_pending_ai_patch() {
    const ret = wasm.wasm_has_pending_ai_patch();
    return ret;
}

/**
 * Importe et instancie un package universel (.karacti.json) dans le moteur
 * @param {string} package_json
 * @returns {number}
 */
export function wasm_import_game_package(package_json) {
    const ptr0 = passStringToWasm0(package_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.wasm_import_game_package(ptr0, len0);
    return ret;
}

/**
 * Initialise l'instance WebAssembly du moteur KaractiRage
 * @returns {number}
 */
export function wasm_init_engine() {
    const ret = wasm.wasm_init_engine();
    return ret;
}

/**
 * Déclenche une interaction (Touche E)
 * @returns {boolean}
 */
export function wasm_interact() {
    const ret = wasm.wasm_interact();
    return ret !== 0;
}

/**
 * Charge un projet KaractiProject complet depuis son format JSON (.karacti)
 * @param {string} project_json
 * @returns {number}
 */
export function wasm_load_project_json(project_json) {
    const ptr0 = passStringToWasm0(project_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.wasm_load_project_json(ptr0, len0);
    return ret;
}

/**
 * Ordonne au joueur de naviguer vers une position cible avec évitement d'obstacles A*
 * @param {number} target_x
 * @param {number} target_y
 * @returns {boolean}
 */
export function wasm_move_player_to(target_x, target_y) {
    const ret = wasm.wasm_move_player_to(target_x, target_y);
    return ret !== 0;
}

/**
 * Initialise un nouveau projet vierge dans le moteur (Studio direct)
 * @param {string} title
 * @param {string} genre_str
 * @returns {number}
 */
export function wasm_new_blank_project(title, genre_str) {
    const ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(genre_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.wasm_new_blank_project(ptr0, len0, ptr1, len1);
    return ret;
}

/**
 * Enregistre un appui touche depuis le navigateur
 * @param {number} key
 */
export function wasm_on_key_down(key) {
    wasm.wasm_on_key_down(key);
}

/**
 * Enregistre un relâchement touche depuis le navigateur
 * @param {number} key
 */
export function wasm_on_key_up(key) {
    wasm.wasm_on_key_up(key);
}

/**
 * Enregistre un clic de souris (bouton 0: gauche, bouton 2: droit)
 * @param {number} button
 */
export function wasm_on_mouse_down(button) {
    wasm.wasm_on_mouse_down(button);
}

/**
 * Enregistre le déplacement de la souris sur le Canvas
 * @param {number} x
 * @param {number} y
 */
export function wasm_on_mouse_move(x, y) {
    wasm.wasm_on_mouse_move(x, y);
}

/**
 * Enregistre le relâchement du clic de souris
 * @param {number} button
 */
export function wasm_on_mouse_up(button) {
    wasm.wasm_on_mouse_up(button);
}

/**
 * Rejette le patch IA et applique un rollback instantané vers l'état antérieur
 * @returns {number}
 */
export function wasm_reject_ai_patch() {
    const ret = wasm.wasm_reject_ai_patch();
    return ret;
}

/**
 * Avance la simulation physique d'un pas dt et génère le rendu dans le buffer RGBA
 * @param {number} dt
 * @returns {number}
 */
export function wasm_render_frame(dt) {
    const ret = wasm.wasm_render_frame(dt);
    return ret >>> 0;
}

/**
 * Commute l'outil actif de l'atelier Studio (0 = Select/V, 1 = Brush/B, 2 = Eraser/X)
 * @param {number} tool
 * @returns {number}
 */
export function wasm_set_active_tool(tool) {
    const ret = wasm.wasm_set_active_tool(tool);
    return ret >>> 0;
}

/**
 * Active ou désactive le mode embarqué (désactive les barres chromées softbuffer pour laisser React gérer le HUD/Studio)
 * @param {boolean} enabled
 */
export function wasm_set_embedded_mode(enabled) {
    wasm.wasm_set_embedded_mode(enabled);
}

/**
 * Instancie une entité selon un préfabriqué (coin, potion, key, chest, door, platform, wall, spikes, boss, crate, plate)
 * @param {string} preset
 * @param {number} x
 * @param {number} y
 * @returns {number}
 */
export function wasm_spawn_entity_preset(preset, x, y) {
    const ptr0 = passStringToWasm0(preset, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.wasm_spawn_entity_preset(ptr0, len0, x, y);
    return ret >>> 0;
}

/**
 * Bascule le mode éditeur KVL
 * @returns {number}
 */
export function wasm_toggle_editor_mode() {
    const ret = wasm.wasm_toggle_editor_mode();
    return ret >>> 0;
}

/**
 * Bascule la minicarte radar
 * @returns {number}
 */
export function wasm_toggle_radar() {
    const ret = wasm.wasm_toggle_radar();
    return ret >>> 0;
}

/**
 * Bascule la grille de partitionnement spatial
 * @returns {number}
 */
export function wasm_toggle_spatial_grid() {
    const ret = wasm.wasm_toggle_spatial_grid();
    return ret >>> 0;
}

/**
 * Met à jour les coordonnées et dimensions d'une entité par son ID
 * @param {number} id
 * @param {number} x
 * @param {number} y
 * @param {number} w
 * @param {number} h
 * @returns {boolean}
 */
export function wasm_update_entity_transform(id, x, y, w, h) {
    const ret = wasm.wasm_update_entity_transform(id, x, y, w, h);
    return ret !== 0;
}

/**
 * Valide une recette de jeu (JSON) et résout automatiquement ses dépendances
 * @param {string} recipe_json
 * @returns {string}
 */
export function wasm_validate_recipe(recipe_json) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(recipe_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasm_validate_recipe(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_debug_string_4687d8d8c2017d52: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_undefined_8865fb403f8fe9d8: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_throw_41e9ee4f547fc59a: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_dcc1a90847f04c41: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_close_d6be50a702eae8fc: function() { return handleError(function (arg0) {
            const ret = arg0.close();
            return ret;
        }, arguments); },
        __wbg_connect_30bfff6aec59fc83: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.connect(arg1);
            return ret;
        }, arguments); },
        __wbg_copyToChannel_459cca8c62b0e606: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.copyToChannel(getArrayF32FromWasm0(arg1, arg2), arg3);
        }, arguments); },
        __wbg_createBufferSource_2cbe6214f761dfcb: function() { return handleError(function (arg0) {
            const ret = arg0.createBufferSource();
            return ret;
        }, arguments); },
        __wbg_createBuffer_41ea68f490cd6844: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.createBuffer(arg1 >>> 0, arg2 >>> 0, arg3);
            return ret;
        }, arguments); },
        __wbg_currentTime_eb1592316a1d4450: function(arg0) {
            const ret = arg0.currentTime;
            return ret;
        },
        __wbg_destination_05471d87031cba86: function(arg0) {
            const ret = arg0.destination;
            return ret;
        },
        __wbg_instanceof_Window_82d71df4eddf88bc: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_new_617a8cdb8bb1130e: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_with_context_options_d6eac9ebf7bbea7b: function() { return handleError(function (arg0) {
            const ret = new lAudioContext(arg0);
            return ret;
        }, arguments); },
        __wbg_resume_262977d0e323b264: function() { return handleError(function (arg0) {
            const ret = arg0.resume();
            return ret;
        }, arguments); },
        __wbg_setTimeout_db7bbc18a17e152a: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.setTimeout(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_buffer_7ab48e13895631dc: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_onended_1f4c7dfbe58d2a9a: function(arg0, arg1) {
            arg0.onended = arg1;
        },
        __wbg_set_sample_rate_b1db9f69dace8e15: function(arg0, arg1) {
            arg0.sampleRate = arg1;
        },
        __wbg_start_6d670bddcbed7c75: function() { return handleError(function (arg0, arg1) {
            arg0.start(arg1);
        }, arguments); },
        __wbg_static_accessor_GLOBAL_266715b9d96ba635: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_10fb7dc1ae063179: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_0b583911f537483a: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_d7f903d1508cbdc4: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 166, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen__convert__closures_____invoke__hf89d7653fac0de47);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./karactirage_wasm_bg.js": import0,
    };
}

const lAudioContext = (typeof AudioContext !== 'undefined' ? AudioContext : (typeof webkitAudioContext !== 'undefined' ? webkitAudioContext : undefined));
function wasm_bindgen__convert__closures_____invoke__hf89d7653fac0de47(arg0, arg1) {
    wasm.wasm_bindgen__convert__closures_____invoke__hf89d7653fac0de47(arg0, arg1);
}

const OutputDeviceFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_outputdevice_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('karactirage_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
