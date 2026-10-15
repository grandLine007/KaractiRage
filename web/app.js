import init, {
  wasm_init_engine,
  wasm_get_canvas_width,
  wasm_get_canvas_height,
  wasm_get_framebuffer_ptr,
  wasm_on_key_down,
  wasm_on_key_up,
  wasm_on_mouse_move,
  wasm_on_mouse_down,
  wasm_on_mouse_up,
  wasm_render_frame,
  wasm_get_score,
  wasm_get_fps,
  wasm_get_cpu_us,
  wasm_get_spatial_queries,
  wasm_toggle_radar,
  wasm_toggle_spatial_grid,
  wasm_toggle_editor_mode,
  wasm_get_status_message
} from './pkg/karactirage_wasm.js';

// Table de correspondance des touches du navigateur vers les codes d'entrée WebKey
const KEY_MAP = {
  'ArrowLeft': 0,
  'ArrowRight': 1,
  'ArrowUp': 2,
  'ArrowDown': 3,
  'KeyW': 4,
  'KeyA': 5,
  'KeyS': 6,
  'KeyD': 7,
  'KeyZ': 8,
  'KeyQ': 9,
  'KeyE': 10,
  'Space': 11,
  'Tab': 12,
  'KeyN': 13,
  'KeyG': 14,
  'KeyM': 15,
  'F1': 16,
  'F2': 17,
  'F3': 18,
  'KeyR': 19,
  'Escape': 20,
  'ControlLeft': 21,
  'ControlRight': 21,
  'MetaLeft': 21,
  'MetaRight': 21,
  'KeyV': 22,
  'KeyB': 23,
  'KeyX': 24,
  'KeyT': 25,
  'KeyP': 26,
  'KeyH': 27,
  'KeyU': 28,
  'F4': 29,
  'KeyL': 29,
  'Digit1': 30,
  'Digit2': 31,
  'Digit3': 32,
  'Digit4': 33,
  'Digit5': 34
};

async function main() {
  const loadingOverlay = document.getElementById('loading-overlay');
  const loadingText = document.getElementById('loading-text');
  const canvas = document.getElementById('game-canvas');
  const ctx = canvas.getContext('2d', { alpha: false });

  // Éléments du HUD
  const statusEl = document.getElementById('engine-status');
  const fpsEl = document.getElementById('metric-fps');
  const cpuEl = document.getElementById('metric-cpu');
  const budgetEl = document.getElementById('metric-budget');
  const spatialEl = document.getElementById('metric-spatial');
  const scoreEl = document.getElementById('metric-score');
  const modeEl = document.getElementById('metric-mode');

  try {
    loadingText.textContent = "Téléchargement & initialisation de karactirage_wasm_bg.wasm...";
    const wasm = await init();
    
    loadingText.textContent = "Démarrage du moteur et chargement des scripts KVL...";
    const initRes = wasm_init_engine();
    if (initRes !== 0) {
      throw new Error(`Échec de wasm_init_engine: code ${initRes}`);
    }

    const width = wasm_get_canvas_width();
    const height = wasm_get_canvas_height();
    canvas.width = width;
    canvas.height = height;

    loadingOverlay.classList.add('hidden');
    statusEl.textContent = wasm_get_status_message() || "Prêt ! ZQSD/Flèches pour jouer.";

    // ==========================================
    // GESTION DES ENTRÉES CLAVIER & SOURIS
    // ==========================================
    window.addEventListener('keydown', (e) => {
      if (KEY_MAP.hasOwnProperty(e.code)) {
        if (['Space', 'Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'F1', 'F2', 'F3'].includes(e.code)) {
          e.preventDefault();
        }
        wasm_on_key_down(KEY_MAP[e.code]);
      }
    });

    window.addEventListener('keyup', (e) => {
      if (KEY_MAP.hasOwnProperty(e.code)) {
        wasm_on_key_up(KEY_MAP[e.code]);
      }
    });

    const getCanvasCoordinates = (e) => {
      const rect = canvas.getBoundingClientRect();
      const scaleX = canvas.width / rect.width;
      const scaleY = canvas.height / rect.height;
      return {
        x: (e.clientX - rect.left) * scaleX,
        y: (e.clientY - rect.top) * scaleY,
      };
    };

    canvas.addEventListener('mousemove', (e) => {
      const coords = getCanvasCoordinates(e);
      wasm_on_mouse_move(coords.x, coords.y);
    });

    canvas.addEventListener('mousedown', (e) => {
      const coords = getCanvasCoordinates(e);
      wasm_on_mouse_move(coords.x, coords.y);
      wasm_on_mouse_down(e.button);
    });

    canvas.addEventListener('mouseup', (e) => {
      wasm_on_mouse_up(e.button);
    });

    canvas.addEventListener('contextmenu', (e) => {
      e.preventDefault();
    });

    const triggerKey = (code) => {
      if (KEY_MAP.hasOwnProperty(code)) {
        wasm_on_key_down(KEY_MAP[code]);
        setTimeout(() => wasm_on_key_up(KEY_MAP[code]), 60);
      }
    };

    // Boutons rapides du HUD et de la barre d'outils
    const bindBtn = (id, fn) => {
      const el = document.getElementById(id);
      if (el) el.addEventListener('click', fn);
    };

    bindBtn('btn-toggle-editor', () => {
      const isEdit = wasm_toggle_editor_mode();
      modeEl.textContent = isEdit ? "ÉDITION" : "JEU";
      modeEl.className = isEdit ? "metric-value text-amber" : "metric-value text-purple";
    });

    bindBtn('btn-toggle-radar', () => wasm_toggle_radar());
    bindBtn('btn-toggle-grid', () => wasm_toggle_spatial_grid());
    bindBtn('btn-toggle-audio', () => triggerKey('KeyM'));
    bindBtn('btn-reload', () => triggerKey('KeyR'));
    bindBtn('btn-tool-select', () => triggerKey('KeyV'));
    bindBtn('btn-tool-brush', () => triggerKey('KeyB'));
    bindBtn('btn-tool-eraser', () => triggerKey('KeyX'));
    bindBtn('btn-tool-tile', () => triggerKey('KeyT'));
    bindBtn('btn-tool-spawn', () => triggerKey('KeyP'));
    bindBtn('btn-tool-help', () => triggerKey('KeyH'));
    bindBtn('btn-tool-genre', () => triggerKey('F4'));
    bindBtn('btn-tool-use', () => triggerKey('KeyU'));
    bindBtn('btn-slot-1', () => triggerKey('Digit1'));
    bindBtn('btn-slot-2', () => triggerKey('Digit2'));
    bindBtn('btn-slot-3', () => triggerKey('Digit3'));
    bindBtn('btn-slot-4', () => triggerKey('Digit4'));
    bindBtn('btn-slot-5', () => triggerKey('Digit5'));

    // ==========================================
    // BOUCLE DE RENDU 60 FPS (requestAnimationFrame)
    // ==========================================
    let lastTime = performance.now();
    let frameCounter = 0;
    let lastMetricUpdate = performance.now();

    function renderLoop(now) {
      const dt = Math.min((now - lastTime) / 1000.0, 0.1);
      lastTime = now;

      // 1. Mise à jour de la physique et rendu dans le buffer RGBA
      wasm_render_frame(dt);

      // 2. Copie directe sans allocation vers le Canvas HTML5
      const ptr = wasm_get_framebuffer_ptr();
      if (ptr !== 0) {
        const pixels = new Uint8ClampedArray(wasm.memory.buffer, ptr, width * height * 4);
        const imageData = new ImageData(pixels, width, height);
        ctx.putImageData(imageData, 0, 0);
      }

      // 3. Rafraîchissement des métriques HUD à 10 Hz
      frameCounter++;
      if (now - lastMetricUpdate >= 100) {
        lastMetricUpdate = now;

        const fps = wasm_get_fps();
        const cpuUs = wasm_get_cpu_us();
        const spatialReq = wasm_get_spatial_queries();
        const score = wasm_get_score();
        const statusMsg = wasm_get_status_message();

        fpsEl.textContent = `${fps.toFixed(1)} FPS`;
        cpuEl.textContent = `${cpuUs} µs`;
        
        if (cpuUs < 16666) {
          budgetEl.textContent = "RESPECTÉ (<16.6ms)";
          budgetEl.className = "metric-value text-emerald";
        } else {
          budgetEl.textContent = "DÉPASSÉ !";
          budgetEl.className = "metric-value text-rose";
        }

        spatialEl.textContent = `${spatialReq} req/frame`;
        scoreEl.textContent = `${score} PTS`;
        if (statusMsg) {
          statusEl.textContent = statusMsg;
        }
      }

      requestAnimationFrame(renderLoop);
    }

    requestAnimationFrame(renderLoop);

  } catch (err) {
    console.error("Erreur d'initialisation WebAssembly:", err);
    loadingText.textContent = `Erreur: ${err.message}`;
    loadingText.style.color = "#ef4444";
  }
}

main();
