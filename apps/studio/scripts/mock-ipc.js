// Mock IPC Tauri — harnais d'audit UX (WS0 / UX-001).
// Script autonome injecté via Playwright `addInitScript({ path })` AVANT le
// chargement de l'application : il définit `window.__TAURI_INTERNALS__.invoke`,
// ce qui fait basculer le frontend en mode Tauri (`isTauri()` → true) sans
// backend natif réel. Données factices cohérentes avec la baseline de l'audit 51
// (frame 1200×800, 12 entités, catalogue de nœuds, 4 projets de démo).
(() => {
  'use strict';

  const CANVAS_W = 1200;
  const CANVAS_H = 800;
  const PNG_1PX_B64 =
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

  const ENTITIES = [
    { id: 1, name: 'Héro', tag: 'player', x: 400, y: 300, w: 32, h: 32, health: 10, max_health: 10, speed: 200, movement_mode: 'free', is_solid: true, is_trigger: false, is_pushable: false, is_active: true, has_movement: true, has_vision: true, vision_range: 320, is_player_controlled: true },
    { id: 2, name: 'Sentinelle', tag: 'enemy', x: 620, y: 300, w: 32, h: 32, health: 4, max_health: 4, speed: 120, movement_mode: 'patrol', is_solid: true, is_trigger: false, is_pushable: false, is_active: true, has_movement: true, has_vision: true, vision_range: 240, is_player_controlled: false },
    { id: 3, name: 'Sentinelle', tag: 'enemy', x: 820, y: 460, w: 32, h: 32, health: 4, max_health: 4, speed: 120, movement_mode: 'patrol', is_solid: true, is_trigger: false, is_pushable: false, is_active: true, has_movement: true, has_vision: true, vision_range: 240, is_player_controlled: false },
    { id: 4, name: 'Pièce d’or', tag: 'collectible', x: 500, y: 240, w: 16, h: 16, is_solid: false, is_trigger: true, is_pushable: false, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
    { id: 5, name: 'Porte verrouillée', tag: 'door', x: 900, y: 200, w: 40, h: 64, is_solid: true, is_trigger: false, is_pushable: false, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
    { id: 6, name: 'Interrupteur', tag: 'switch', x: 760, y: 520, w: 24, h: 24, is_solid: false, is_trigger: true, is_pushable: false, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
    { id: 7, name: 'Plateforme', tag: 'platform', x: 640, y: 380, w: 128, h: 16, is_solid: true, is_trigger: false, is_pushable: false, is_active: true, has_movement: true, has_vision: false, is_player_controlled: false },
    { id: 8, name: 'Mur', tag: 'wall', x: 0, y: 0, w: 1200, h: 24, is_solid: true, is_trigger: false, is_pushable: false, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
    { id: 9, name: 'Mur', tag: 'wall', x: 0, y: 776, w: 1200, h: 24, is_solid: true, is_trigger: false, is_pushable: false, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
    { id: 10, name: 'Caisse', tag: 'pushable', x: 360, y: 500, w: 32, h: 32, is_solid: true, is_trigger: false, is_pushable: true, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
    { id: 11, name: 'Cœur', tag: 'pickup', x: 700, y: 220, w: 20, h: 20, is_solid: false, is_trigger: true, is_pushable: false, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
    { id: 12, name: 'Téléporteur', tag: 'teleport', x: 200, y: 600, w: 48, h: 48, is_solid: false, is_trigger: true, is_pushable: false, is_active: true, has_movement: false, has_vision: false, is_player_controlled: false },
  ];

  const PROJECTS = [
    { path: 'assets/scenes/level_01.scene.json', title: 'Donjon Niveau 1 — Aventure RPG Top-Down', description: 'Quêtes, clés, portes & sentinelles.', objective: 'Atteignez la sortie du donjon.', genre: 'Aventure', dimension: '2D', kind: 'scene', size_bytes: 36864, modified_at: 1752000000, entity_count: 39, is_recent: true, status: 'available' },
    { path: 'assets/scenes/platformer_01.scene.json', title: 'Cyber Platformer 2D — Gravité & Sauts', description: 'Moteurs de saut, plateformes mobiles.', objective: 'Survivez aux plateformes.', genre: 'Plateforme', dimension: '2D', kind: 'scene', size_bytes: 9011, modified_at: 1751900000, entity_count: 24, is_recent: true, status: 'available' },
    { path: 'assets/scenes/boss_01.scene.json', title: 'Arène du Titan — Combat de Boss & Télégraphie', description: 'Boss réactif KVL, télégraphie & phase 2.', objective: 'Terrassez le Titan.', genre: 'Combat', dimension: '2D', kind: 'scene', size_bytes: 11878, modified_at: 1751800000, entity_count: 18, is_recent: false, status: 'available' },
    { path: 'assets/scenes/arcade_01.scene.json', title: 'Raid Stellaire — Shoot them up', description: 'Vagues de projectiles, score, boss final.', objective: 'Détruisez le vaisseau amiral.', genre: 'Arcade', dimension: '2D', kind: 'scene', size_bytes: 12400, modified_at: 1751700000, entity_count: 30, is_recent: false, status: 'available' },
  ];

  const RECENT = [
    'assets/scenes/level_01.scene.json',
    'assets/scenes/platformer_01.scene.json',
  ];

  const BRUSH_TILES = [
    { id: 'wall', label: 'Mur Solide', short: 'W', color: '#7b6e8f' },
    { id: 'floor', label: 'Sol', short: 'F', color: '#3f5a4a' },
    { id: 'spike', label: 'Piques', short: 'S', color: '#d64550' },
    { id: 'coin', label: 'Pièce', short: 'C', color: '#e0b341' },
  ];

  const SPRITES = [
    { id: 'hero', file: 'assets/generated/sprites/hero.png', w: 32, h: 32 },
    { id: 'coin', file: 'assets/generated/sprites/coin.png', w: 16, h: 16 },
    { id: 'slime', file: 'assets/generated/sprites/slime.png', w: 32, h: 32 },
    { id: 'crate', file: 'assets/generated/sprites/crate.png', w: 32, h: 32 },
  ];

  const MESHES = [
    { id: 'crate', kind: 'procedural', parts: 6 },
    { id: 'pillar', kind: 'procedural', parts: 1 },
    { id: 'spike', kind: 'procedural', parts: 4 },
    { id: 'ramp', kind: 'procedural', parts: 2 },
  ];

  const CATALOG = {
    api_version: 1,
    plugin_count: 1,
    node_count: 3,
    load_order: ['karacti.pack.platformer'],
    plugins: [{ id: 'karacti.pack.platformer', name: 'Platformer Pack', kind: 'content', version: '1.0.0' }],
    nodes: [
      { id: 'Event:OnPlayerOverlap', flavor: 'event', label: 'Au contact du joueur', category: 'Interactions', description: 'Déclenché quand le joueur chevauche l’entité.', params: [], aliases: [], prefixes: [], default_argument: '', flow_input: false, flow_outputs: ['out'], plugin: 'karacti.pack.platformer' },
      { id: 'Action:AddScore', flavor: 'action', label: 'Ajouter du score', category: 'Progression', description: 'Ajoute un montant au score.', params: [{ name: 'amount', label: 'Montant', param_type: 'int', default: 100 }], aliases: [], prefixes: [], default_argument: '100', flow_input: true, flow_outputs: ['out'], plugin: null },
      { id: 'Condition:HasItem', flavor: 'condition', label: 'Possède un objet', category: 'Inventaire', description: 'Vrai si le joueur possède l’objet.', params: [{ name: 'item', label: 'Objet', param_type: 'string', default: 'key' }], aliases: [], prefixes: [], default_argument: 'key', flow_input: true, flow_outputs: ['then', 'else'], plugin: null },
    ],
    issues: [],
  };

  const VECTOR_MODEL = {
    width: 256,
    height: 256,
    layers: [
      { color: '#3fa34d', paths: [{ points: [[10, 10], [120, 10], [120, 120], [10, 120]], closed: true }] },
    ],
  };

  function b64ToBuffer(b64) {
    const bin = atob(b64);
    const u = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i);
    return u.buffer;
  }

  function frameBuffer() {
    return new Uint8Array(CANVAS_W * CANVAS_H * 4).buffer; // noir opaque
  }

  const HANDLERS = {
    // Boot & état
    engine_status: () => ({ state: 'ready', error: null, init_ms: 120 }),
    engine_retry_boot: () => true,
    engine_canvas_width: () => CANVAS_W,
    engine_canvas_height: () => CANVAS_H,
    engine_frame: () => frameBuffer(),
    engine_frame_packet: () => new ArrayBuffer(0),
    engine_telemetry: () => ({ fps: 60, total_us: 310, score: 0, status: 'OK' }),
    engine_system_diagnostics: () => JSON.stringify({ os: 'macOS', arch: 'arm64', cpu_cores: 8, fps: 60, entities: 12, projects_home: '~/KaractiRage/projects', engine: 'KaractiRage Engine Core 0.1.0' }),

    // Projets
    engine_list_projects: () => JSON.stringify(PROJECTS),
    engine_recent_projects: () => JSON.stringify(RECENT),
    engine_open_project: () => 0,
    engine_create_project: () => JSON.stringify(PROJECTS[0]),
    engine_duplicate_project: () => JSON.stringify(PROJECTS[0]),
    engine_rename_project: () => JSON.stringify(PROJECTS[0]),
    engine_archive_project: () => '',
    engine_remove_recent: () => true,
    engine_reveal_project: () => undefined,
    engine_new_blank_project: () => 0,
    engine_load_project_json: () => 0,
    engine_export_project_json: () => '{}',
    engine_export_standalone_html: () => '<html lang="fr"><body>KaractiRage export (audit)</body></html>',

    // Éditeur (lecture)
    engine_editor_state: () => JSON.stringify({ entities: ENTITIES, selected_id: 1, tool: 'select', tile: 'wall', tile_label: 'Mur Solide', can_undo: true, can_redo: false, status: '', mode: 'edit' }),
    engine_get_entities_summary: () => JSON.stringify(ENTITIES),
    engine_get_selected_entity: () => 1,
    engine_get_brush_tiles: () => JSON.stringify(BRUSH_TILES),
    engine_get_active_tool: () => 'select',
    engine_get_active_brush_tile: () => 'wall',
    engine_get_player_pos: () => ({ x: 400, y: 300 }),
    engine_get_score: () => 0,
    engine_get_status_message: () => 'OK',
    engine_has_pending_ai_patch: () => false,

    // Éditeur (mutations sans effet)
    engine_on_key_down: () => undefined,
    engine_on_key_up: () => undefined,
    engine_on_mouse_move: () => undefined,
    engine_on_mouse_down: () => undefined,
    engine_on_mouse_up: () => undefined,
    engine_set_active_tool: () => undefined,
    engine_set_brush_tile: () => 'wall',
    engine_cycle_brush_tile: () => 'wall',
    engine_set_editor_mode: () => 1,
    engine_toggle_editor_mode: () => 1,
    engine_set_embedded_mode: () => undefined,
    engine_select_entity: () => true,
    engine_update_entity_transform: () => true,
    engine_duplicate_selected_entity: () => 1,
    engine_delete_selected_entity: () => true,
    engine_delete_entity: () => true,
    engine_spawn_entity_preset: () => 1,
    engine_spawn_entity_at_screen: () => 1,
    engine_spawn_3d_preset: () => 1,
    engine_undo: () => true,
    engine_redo: () => true,
    engine_save_scene: () => true,
    engine_move_player_to: () => true,
    engine_interact: () => false,
    engine_move_entity_3d: () => true,
    engine_toggle_radar: () => false,
    engine_toggle_spatial_grid: () => false,

    // Archétypes & recettes
    engine_launch_archetype: () => 'Cyber Platformer 2D',
    engine_launch_archetype_3d: () => 12,
    engine_apply_recipe: () => 0,
    engine_validate_recipe: () => ({ is_valid: true, resolved_dependencies: [], warnings: [], conflicts: [] }),

    // IA
    engine_apply_ai_payload: () => ({ applied: true }),
    engine_accept_ai_patch: () => true,
    engine_reject_ai_patch: () => true,
    test_llm_connection: () => ({ ok: true, model: 'mock', latency_ms: 12 }),
    generate_ai_level: () => ({ ok: true, level_json: '{}' }),

    // Assets & palette
    engine_list_sprite_assets: () => JSON.stringify(SPRITES),
    engine_list_sheet_assets: () => JSON.stringify([]),
    engine_list_anim_assets: () => JSON.stringify([]),
    engine_list_mesh_assets: () => JSON.stringify(MESHES),
    engine_list_vector_assets: () => JSON.stringify([{ id: 'import_01', file: 'assets/vectors/import_01.vec.json' }]),
    engine_sprite_png: () => b64ToBuffer(PNG_1PX_B64),
    engine_sheet_png: () => b64ToBuffer(PNG_1PX_B64),

    // KVL
    engine_get_kvl_graphs: () => JSON.stringify([]),
    engine_get_kvl_graph: () => 'null',
    engine_validate_kvl_graph: () => '[]',
    engine_build_guided_graph: () => 'null',
    engine_attach_script: () => true,
    engine_save_kvl_graph: () => true,
    engine_kvl_add_node: () => '{}',
    engine_plugin_catalog: () => JSON.stringify(CATALOG),

    // Vectoriel
    engine_import_raster: () => [256, 256],
    engine_import_svg: () => [256, 256],
    engine_vectorize_raster: () => '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"/>',
    engine_vector_load: () => JSON.stringify(VECTOR_MODEL),
    engine_vector_save: () => 'ok',
    engine_vector_spawn: () => 1,

    // Atelier Pixel Art (pxl)
    pxl_available: () => true,
    pxl_build: () => ({ ok: true, stdout: '', stderr: '', duration_ms: 1 }),
    pxl_serve_start: () => 'http://localhost:8787',
    pxl_serve_stop: () => true,
    pxl_serve_status: () => ({ running: false, url: null, port: 0 }),
    pxl_list_sources: () => [],
    pxl_read_source: () => '',
    pxl_write_source: () => '/tmp/audit.px.js',
  };

  const FAULT = new URLSearchParams(window.location.search).get('fault') || null;

  function handle(cmd, args) {
    // Injection de pannes (UX-005) : ?fault=null|timeout|slow|empty
    if (FAULT === 'null' && (cmd === 'engine_canvas_width' || cmd === 'engine_canvas_height' || cmd === 'engine_frame')) {
      return Promise.reject(new Error('IPC indisponible (panne simulée)'));
    }
    if (FAULT === 'null' && cmd === 'engine_status') {
      return { state: 'failed', error: 'Moteur indisponible (panne simulée)', init_ms: 0 };
    }
    if (FAULT === 'timeout' && (cmd === 'engine_canvas_width' || cmd === 'engine_canvas_height')) {
      return new Promise(() => {}); // ne résout jamais → spinner « Préparation du moteur… »
    }
    if (FAULT === 'slow' && cmd === 'engine_canvas_width') {
      return new Promise((res) => setTimeout(() => res(CANVAS_W), 2500));
    }
    if (FAULT === 'empty' && cmd === 'engine_plugin_catalog') {
      return JSON.stringify({ api_version: 1, plugin_count: 0, node_count: 0, load_order: [], plugins: [], nodes: [], issues: [] });
    }
    const h = HANDLERS[cmd];
    if (h) return h(args || {});
    if (cmd.startsWith('engine_on_')) return undefined;
    if (cmd.startsWith('engine_toggle_')) return false;
    return undefined;
  }

  window.__TAURI_INTERNALS__ = {
    invoke: (cmd, args) => {
      try {
        return Promise.resolve(handle(cmd, args));
      } catch (e) {
        return Promise.reject(e);
      }
    },
    transformCallback: () => 0,
    convertFileSrc: (p) => p,
  };
})();
