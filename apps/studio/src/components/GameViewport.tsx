import { useEffect, useRef, useState, useCallback } from 'react';
import { Play, Pause, Crosshair, Grid3X3 } from 'lucide-react';
// @ts-ignore
import init, * as wasmBindings from '../wasm/karactirage_wasm.js';
// @ts-ignore
import wasmUrl from '../wasm/karactirage_wasm_bg.wasm?url';
import { setWasmBindings } from '../services/engineBridge';

interface GameViewportProps {
  onTelemetryUpdate?: (metrics: { fps: number; cpuUs: number; score: number; status: string }) => void;
  onSelectedEntityChange?: (entityName: string | null) => void;
}

// Table de correspondance des touches du navigateur vers les codes d'entrée WebKey KaractiRage
const KEY_MAP: Record<string, number> = {
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
  'Digit1': 30,
  'Digit2': 31,
  'Digit3': 32,
  'Digit4': 33,
  'Digit5': 34,
  'KeyI': 35,
  'Enter': 36,
  'NumpadEnter': 36
};

export const GameViewport: React.FC<GameViewportProps> = ({ onTelemetryUpdate }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isPaused, setIsPaused] = useState(false);
  const [showRadar, setShowRadar] = useState(true);
  const [showGrid, setShowGrid] = useState(false);

  const wasmModuleRef = useRef<any>(null);
  const wasmExportsRef = useRef<any>(null);
  const animFrameIdRef = useRef<number | null>(null);
  const lastTimeRef = useRef<number>(performance.now());

  // Initialisation du module WebAssembly
  useEffect(() => {
    let active = true;

    async function loadEngine() {
      try {
        const wasm = await init({ module_or_path: wasmUrl });

        if (!active) return;

        wasmModuleRef.current = wasm;
        wasmExportsRef.current = wasmBindings;
        setWasmBindings(wasmBindings);

        // Exposition pour l'automatisation, console navigateur et tests
        if (typeof window !== 'undefined') {
          (window as any).karactiEngine = {
            wasm: wasmBindings,
            movePlayerTo: (x: number, y: number) => (wasmBindings as any).wasm_move_player_to ? (wasmBindings as any).wasm_move_player_to(x, y) : false,
            getPlayerPos: () => ({
              x: (wasmBindings as any).wasm_get_player_pos_x ? (wasmBindings as any).wasm_get_player_pos_x() : 0,
              y: (wasmBindings as any).wasm_get_player_pos_y ? (wasmBindings as any).wasm_get_player_pos_y() : 0,
            }),
            interact: () => (wasmBindings as any).wasm_interact ? (wasmBindings as any).wasm_interact() : false,
            getScore: () => (wasmBindings as any).wasm_get_score ? (wasmBindings as any).wasm_get_score() : 0,
            getStatus: () => (wasmBindings as any).wasm_get_status_message ? (wasmBindings as any).wasm_get_status_message() : '',
          };
        }

        // Démarrage du moteur
        const initCode = wasmBindings.wasm_init_engine();
        if (initCode !== 0) {
          throw new Error(`Échec wasm_init_engine: code ${initCode}`);
        }

        if (typeof (wasmBindings as any).wasm_set_embedded_mode === 'function') {
          (wasmBindings as any).wasm_set_embedded_mode(true);
        }

        const canvas = canvasRef.current;
        if (canvas) {
          canvas.width = wasmBindings.wasm_get_canvas_width() || 1200;
          canvas.height = wasmBindings.wasm_get_canvas_height() || 800;
        }

        setIsLoaded(true);
      } catch (err: any) {
        console.error("Erreur de chargement Wasm KaractiRage:", err);
        if (active) {
          setLoadError(err?.message || "Impossible de charger karactirage_wasm_bg.wasm");
        }
      }
    }

    loadEngine();

    return () => {
      active = false;
      if (animFrameIdRef.current) {
        cancelAnimationFrame(animFrameIdRef.current);
      }
    };
  }, []);

  // Boucle de rendu 60 FPS
  useEffect(() => {
    if (!isLoaded || isPaused) return;

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d', { alpha: false });
    if (!ctx) return;

    const wasmBindings = wasmExportsRef.current;
    const wasm = wasmModuleRef.current;
    if (!wasmBindings || !wasm) return;

    const width = 1200;
    const height = 800;
    canvas.width = width;
    canvas.height = height;

    let frameCounter = 0;

    function renderLoop(now: number) {
      const dt = Math.min((now - lastTimeRef.current) / 1000.0, 0.05);
      lastTimeRef.current = now;

      // 1. Avance la simulation et génère le rendu logiciel
      wasmBindings.wasm_render_frame(dt);

      // 2. Transfert haute vitesse du buffer mémoire vers le Canvas HTML5 (stride exact 1200x800)
      const ptr = wasmBindings.wasm_get_framebuffer_ptr();
      if (ptr !== 0 && ctx) {
        const pixels = new Uint8ClampedArray(wasm.memory.buffer, ptr, width * height * 4);
        const imageData = new ImageData(pixels, width, height);
        ctx.putImageData(imageData, 0, 0);
      }

      // 3. Télémétrie périodique (toutes les 10 frames pour limiter les re-renders React)
      frameCounter++;
      if (frameCounter % 10 === 0 && onTelemetryUpdate) {
        onTelemetryUpdate({
          fps: wasmBindings.wasm_get_fps(),
          cpuUs: wasmBindings.wasm_get_cpu_us(),
          score: wasmBindings.wasm_get_score(),
          status: wasmBindings.wasm_get_status_message(),
        });
      }

      animFrameIdRef.current = requestAnimationFrame(renderLoop);
    }

    lastTimeRef.current = performance.now();
    animFrameIdRef.current = requestAnimationFrame(renderLoop);

    return () => {
      if (animFrameIdRef.current) {
        cancelAnimationFrame(animFrameIdRef.current);
      }
    };
  }, [isLoaded, isPaused, onTelemetryUpdate]);

  // Gestion des entrées clavier
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (['input', 'textarea'].includes((e.target as HTMLElement)?.tagName?.toLowerCase())) {
        return;
      }

      const keyVal = KEY_MAP[e.code];
      if (keyVal !== undefined && wasmExportsRef.current) {
        wasmExportsRef.current.wasm_on_key_down(keyVal);
        if (['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.code)) {
          e.preventDefault();
        }
      }
    }

    function handleKeyUp(e: KeyboardEvent) {
      if (['input', 'textarea'].includes((e.target as HTMLElement)?.tagName?.toLowerCase())) {
        return;
      }
      const keyVal = KEY_MAP[e.code];
      if (keyVal !== undefined && wasmExportsRef.current) {
        wasmExportsRef.current.wasm_on_key_up(keyVal);
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  // Gestion des coordonnées souris
  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !wasmExportsRef.current) return;

    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = (e.clientX - rect.left) * scaleX;
    const y = (e.clientY - rect.top) * scaleY;

    wasmExportsRef.current.wasm_on_mouse_move(x, y);
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !wasmExportsRef.current) return;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = (e.clientX - rect.left) * scaleX;
    const y = (e.clientY - rect.top) * scaleY;
    wasmExportsRef.current.wasm_on_mouse_move(x, y);
    const button = e.button === 2 ? 2 : 0;
    wasmExportsRef.current.wasm_on_mouse_down(button);
  }, []);

  const handleMouseUp = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!wasmExportsRef.current) return;
    const button = e.button === 2 ? 2 : 0;
    wasmExportsRef.current.wasm_on_mouse_up(button);
  }, []);

  // Actions de la barre d'outils du viewport
  const togglePause = () => setIsPaused(prev => !prev);
  const toggleRadarAction = () => {
    if (wasmExportsRef.current) {
      wasmExportsRef.current.wasm_toggle_radar();
      setShowRadar(prev => !prev);
    }
  };
  const toggleGridAction = () => {
    if (wasmExportsRef.current) {
      wasmExportsRef.current.wasm_toggle_spatial_grid();
      setShowGrid(prev => !prev);
    }
  };

  return (
    <div className="relative flex flex-col flex-1 h-full bg-[#07040d] overflow-hidden select-none">
      {/* Barre d'outils supérieure du Viewport */}
      <div className="flex items-center justify-between px-4 py-2 bg-[#0d0819] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2">
          <span className="badge-tag bg-[#2e1065] text-[#c084fc] border border-[#a855f7]">
            VIEWPORT 2D
          </span>
          <span className="text-xs text-[#94a3b8]">1200 × 800 • Déterministe 60 FPS</span>
        </div>

        <div className="flex items-center gap-1.5">
          <button
            onClick={togglePause}
            className={`p-1.5 rounded transition ${isPaused ? 'bg-[#f59e0b] text-black font-bold' : 'hover:bg-[#1e1338] text-[#f8fafc]'}`}
            title={isPaused ? "Reprendre (Pause active)" : "Mettre en pause"}
          >
            {isPaused ? <Play size={15} /> : <Pause size={15} />}
          </button>
          <button
            onClick={toggleRadarAction}
            className={`p-1.5 rounded transition ${showRadar ? 'bg-[#2e1065] text-[#38bdf8] border border-[#38bdf8]/40' : 'hover:bg-[#1e1338] text-[#64748b]'}`}
            title="Minicarte Radar [M]"
          >
            <Crosshair size={15} />
          </button>
          <button
            onClick={toggleGridAction}
            className={`p-1.5 rounded transition ${showGrid ? 'bg-[#2e1065] text-[#a855f7] border border-[#a855f7]/40' : 'hover:bg-[#1e1338] text-[#64748b]'}`}
            title="Grille Spatiale [G]"
          >
            <Grid3X3 size={15} />
          </button>
        </div>
      </div>

      {/* Conteneur du Canvas */}
      <div className="relative flex-1 flex items-center justify-center p-2 bg-[#050308] overflow-hidden">
        {loadError && (
          <div className="absolute inset-0 z-20 flex flex-col items-center justify-center p-6 text-center bg-[#050308]/90">
            <span className="text-[#f43f5e] font-bold text-base mb-2">Erreur WebAssembly</span>
            <p className="text-xs text-[#94a3b8] mb-4">{loadError}</p>
            <button
              onClick={() => window.location.reload()}
              className="px-4 py-2 bg-[#f43f5e] text-white rounded font-medium text-xs hover:bg-[#e11d48]"
            >
              Recharger
            </button>
          </div>
        )}

        {!isLoaded && !loadError && (
          <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-[#050308]/90">
            <div className="w-8 h-8 border-2 border-[#a855f7] border-t-transparent rounded-full animate-spin" />
            <span className="text-xs text-[#c084fc] font-medium tracking-wide">
              Initialisation du moteur Wasm KaractiRage...
            </span>
          </div>
        )}

        <canvas
          ref={canvasRef}
          width={1200}
          height={800}
          onMouseMove={handleMouseMove}
          onMouseDown={handleMouseDown}
          onMouseUp={handleMouseUp}
          onContextMenu={e => e.preventDefault()}
          tabIndex={0}
          className="w-full h-full object-contain shadow-2xl rounded border border-[#2d1b4e] focus:outline-none focus:border-[#a855f7]"
          style={{ imageRendering: 'pixelated' }}
        />
      </div>
    </div>
  );
};
