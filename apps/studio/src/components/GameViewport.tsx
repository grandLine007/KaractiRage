import { useEffect, useRef, useState, useCallback } from 'react';
import { Play, Pause, Crosshair, Grid3X3, PenTool, Box } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { detectRenderCapabilities, type RenderCapabilities } from '../render3d/capabilities';
import { retryEngineBoot } from '../services/engineBridge';

interface GameViewportProps {
  onTelemetryUpdate?: (metrics: { fps: number; cpuUs: number; score: number; status: string }) => void;
  onSceneChanged?: () => void;
  onSpawnAtScreen?: (preset: string, sx: number, sy: number) => void;
  isEditMode?: boolean;
  view3d: boolean;
  onView3dChange: (v: boolean) => void;
}

interface EngineTelemetry {
  fps: number;
  cpu_us: number;
  score: number;
  status: string;
  update_us: number;
  render_us: number;
  encode_us: number;
  total_us: number;
}

/** Contrôleur de cycle de vie renvoyé par `startScene3D` (structure locale). */
interface Scene3DController {
  pause: () => void;
  resume: () => void;
  dispose: () => void;
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
  'NumpadEnter': 36,
  'Delete': 37,
  'Backspace': 38,
  'KeyC': 39,
  'KeyY': 40
};

export const GameViewport: React.FC<GameViewportProps> = ({ onTelemetryUpdate, onSceneChanged, onSpawnAtScreen, isEditMode = false, view3d, onView3dChange }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [bootTick, setBootTick] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const [showRadar, setShowRadar] = useState(true);
  const [showGrid, setShowGrid] = useState(false);
  const [capabilities, setCapabilities] = useState<RenderCapabilities | null>(null);
  const [selected3dId, setSelected3dId] = useState<number | null>(null);

  const animFrameIdRef = useRef<number | null>(null);
  const lastTimeRef = useRef<number>(performance.now());
  const canvasWidthRef = useRef<number>(1200);
  const canvasHeightRef = useRef<number>(800);
  // Position souris en attente d'envoi (coalescé : 1 IPC par frame)
  const pendingMouseRef = useRef<{ x: number; y: number } | null>(null);
  const scene3dRef = useRef<HTMLDivElement | null>(null);
  const scene3dCtlRef = useRef<Scene3DController | null>(null);
  const started3dRef = useRef(false);
  const [scene3dReady, setScene3dReady] = useState(false);
  const [scene3dError, setScene3dError] = useState<string | null>(null);

  // Initialisation du viewport : la première commande `engine_canvas_width` attend la
  // fin du boot moteur (désormais lancé en arrière-plan côté Tauri), d'où le spinner.
  useEffect(() => {
    let active = true;

    async function loadEngine() {
      try {
        const w = await invoke<number>('engine_canvas_width');
        const h = await invoke<number>('engine_canvas_height');
        if (!active) return;
        canvasWidthRef.current = w || 1200;
        canvasHeightRef.current = h || 800;

        const canvas = canvasRef.current;
        if (canvas) {
          canvas.width = canvasWidthRef.current;
          canvas.height = canvasHeightRef.current;
        }

        setIsLoaded(true);
        setLoadError(null);
      } catch (err: any) {
        console.error("Erreur d'initialisation du viewport KaractiRage:", err);
        if (active) {
          setLoadError(err?.message || 'Impossible de joindre le moteur natif Tauri');
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
  }, [bootTick]);

  // Relance le boot après un échec (plus de `exit(1)` : récupération possible).
  const retryEngine = useCallback(async () => {
    setLoadError(null);
    setIsLoaded(false);
    await retryEngineBoot();
    setBootTick((t) => t + 1);
  }, []);

  // Détection des capacités 3D (badge GPU) — indépendante du rendu 2D actif.
  useEffect(() => {
    let active = true;
    detectRenderCapabilities().then((c) => {
      if (active) setCapabilities(c);
    });
    return () => {
      active = false;
    };
  }, []);

  // Boucle de rendu 60 FPS (pixel-streaming depuis le moteur natif)
  useEffect(() => {
    if (!isLoaded || isPaused || view3d) return;

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d', { alpha: false });
    if (!ctx) return;

    const width = canvasWidthRef.current;
    const height = canvasHeightRef.current;
    canvas.width = width;
    canvas.height = height;

    let frameCounter = 0;

    async function renderLoop(now: number) {
      const dt = Math.min((now - lastTimeRef.current) / 1000.0, 0.05);
      lastTimeRef.current = now;

      // Coalescence souris : envoie la dernière position connue (1 IPC max par frame)
      const pendingMouse = pendingMouseRef.current;
      if (pendingMouse) {
        pendingMouseRef.current = null;
        void invoke('engine_on_mouse_move', { x: pendingMouse.x, y: pendingMouse.y });
      }

      // 1. Avance la simulation native et récupère le framebuffer RGBA
      const buffer = await invoke<ArrayBuffer>('engine_frame', { dt });

      // 2. Transfert du framebuffer vers le Canvas HTML5
      if (ctx) {
        const pixels = new Uint8ClampedArray(buffer);
        const imageData = new ImageData(pixels, width, height);
        ctx.putImageData(imageData, 0, 0);
      }

      // 3. Télémétrie groupée (1 seul IPC toutes les 10 frames au lieu de 4)
      frameCounter++;
      if (frameCounter % 10 === 0 && onTelemetryUpdate) {
        const t = await invoke<EngineTelemetry>('engine_telemetry');
        onTelemetryUpdate({
          fps: t.fps,
          cpuUs: t.total_us,
          score: t.score,
          status: t.status,
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
  }, [isLoaded, isPaused, view3d, onTelemetryUpdate]);

  // Vue 3D (three.js) pilotée par le moteur Rust via `engine_frame_packet`.
  // Montée UNE fois (à la première bascule) puis conservée en vie : pause/resume au
  // lieu de recréer le device GPU à chaque aller-retour 2D↔3D.
  useEffect(() => {
    if (!view3d) {
      scene3dCtlRef.current?.pause();
      return;
    }
    if (started3dRef.current) {
      scene3dCtlRef.current?.resume();
      return;
    }
    started3dRef.current = true;
    const container = scene3dRef.current;
    if (!container) {
      started3dRef.current = false;
      return;
    }
    let active = true;
    import('../render3d/scene3d')
      .then((mod) => mod.startScene3D(container, () => {}, (id) => {
        if (active) setSelected3dId(id);
      }))
      .then((ctl) => {
        if (!active) {
          started3dRef.current = false;
          ctl.dispose();
          return;
        }
        scene3dCtlRef.current = ctl;
        setScene3dReady(true);
        setScene3dError(null);
      })
      .catch((err) => {
        started3dRef.current = false;
        if (active) setScene3dError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      active = false;
    };
  }, [view3d]);

  // Gestion des entrées clavier
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (['input', 'textarea'].includes((e.target as HTMLElement)?.tagName?.toLowerCase())) {
        return;
      }

      // Empêche le navigateur d'intercepter les raccourcis d'édition du Studio.
      if (
        (e.metaKey || e.ctrlKey) &&
        ['s', 'z', 'y', 'd'].includes(e.key.toLowerCase())
      ) {
        e.preventDefault();
      }

      const keyVal = KEY_MAP[e.code];
      if (keyVal !== undefined) {
        void invoke('engine_on_key_down', { key: keyVal });
        if (['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Backspace'].includes(e.code)) {
          e.preventDefault();
        }
      }
    }

    function handleKeyUp(e: KeyboardEvent) {
      if (['input', 'textarea'].includes((e.target as HTMLElement)?.tagName?.toLowerCase())) {
        return;
      }
      const keyVal = KEY_MAP[e.code];
      if (keyVal !== undefined) {
        void invoke('engine_on_key_up', { key: keyVal });
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  // Conversion client -> coordonnées internes du canvas, en tenant compte du
  // letterboxing de `object-contain` (le contenu 1200×800 est centré dans l'élément).
  const toCanvasCoords = useCallback((clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    const cw = canvas.width;
    const ch = canvas.height;
    if (rect.width === 0 || rect.height === 0) return null;
    const scale = Math.min(rect.width / cw, rect.height / ch);
    const contentW = cw * scale;
    const contentH = ch * scale;
    const offsetX = (rect.width - contentW) / 2;
    const offsetY = (rect.height - contentH) / 2;
    return {
      x: (clientX - rect.left - offsetX) / scale,
      y: (clientY - rect.top - offsetY) / scale,
    };
  }, []);

  // Gestion des coordonnées souris (coalescée : stocke la position, envoyée par la boucle de rendu)
  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const p = toCanvasCoords(e.clientX, e.clientY);
    if (p) pendingMouseRef.current = p;
  }, [toCanvasCoords]);

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const p = toCanvasCoords(e.clientX, e.clientY);
    if (p) void invoke('engine_on_mouse_move', { x: p.x, y: p.y });
    const button = e.button === 2 ? 2 : 0;
    void invoke('engine_on_mouse_down', { button });
  }, [toCanvasCoords]);

  const handleMouseUp = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const button = e.button === 2 ? 2 : 0;
    void invoke('engine_on_mouse_up', { button });
    // La sélection / le déplacement sont traités côté moteur à la frame suivante.
    window.setTimeout(() => onSceneChanged?.(), 60);
  }, [onSceneChanged]);

  // Glisser-déposer d'un préfabriqué depuis la palette vers une position exacte.
  const handleDragOver = useCallback((e: React.DragEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
  }, []);

  const handleDrop = useCallback((e: React.DragEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const preset = e.dataTransfer.getData('application/x-karacti-preset');
    if (!preset) return;
    const p = toCanvasCoords(e.clientX, e.clientY);
    if (p) onSpawnAtScreen?.(preset, p.x, p.y);
  }, [onSpawnAtScreen, toCanvasCoords]);

  // Actions de la barre d'outils du viewport
  const togglePause = () => setIsPaused(prev => !prev);
  const toggleRadarAction = () => {
    void invoke('engine_toggle_radar');
    setShowRadar(prev => !prev);
  };
  const toggleGridAction = () => {
    void invoke('engine_toggle_spatial_grid');
    setShowGrid(prev => !prev);
  };

  return (
    <div className="relative flex flex-col flex-1 h-full bg-[#07040d] overflow-hidden select-none">
      {/* Barre d'outils supérieure du Viewport */}
      <div className="flex items-center justify-between px-4 py-2 bg-[#0d0819] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2">
          <span className="badge-tag bg-[#2e1065] text-[#c084fc] border border-[#a855f7]">
            VIEWPORT {view3d ? '3D' : '2D'}
          </span>
          {view3d && selected3dId !== null && (
            <span className="badge-tag bg-[#082F49] text-[#fbbf24] border border-[#f59e0b]">
              Sélection #{selected3dId}
            </span>
          )}
          {capabilities && (
            <span
              className={`badge-tag border ${
                capabilities.tier === 'webgpu'
                  ? 'bg-[#082F49] text-[#38bdf8] border-[#0284c7]'
                  : capabilities.tier === 'webgl2'
                    ? 'bg-[#064E3B] text-[#a7f3d0] border-[#059669]'
                    : 'bg-[#3f3f46] text-[#d4d4d8] border-[#71717a]'
              }`}
              title="Capacité de rendu 3D détectée (webgpu → webgl2 → logiciel)"
            >
              GPU {capabilities.backend}
            </span>
          )}
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
          <button
            onClick={() => onView3dChange(!view3d)}
            className={`p-1.5 rounded transition ${view3d ? 'bg-[#2e1065] text-[#c084fc] border border-[#a855f7]' : 'hover:bg-[#1e1338] text-[#64748b]'}`}
            title={view3d ? "Revenir à la vue 2D" : "Basculer en vue 3D (RenderPacket)"}
          >
            <Box size={15} />
          </button>
        </div>
      </div>

      {/* Conteneur du Canvas */}
      <div className="relative flex-1 flex items-center justify-center p-2 bg-[#050308] overflow-hidden">
        {loadError && (
          <div className="absolute inset-0 z-20 flex flex-col items-center justify-center p-6 text-center bg-[#050308]/90">
            <span className="text-[#f43f5e] font-bold text-base mb-2">Erreur du moteur</span>
            <p className="text-xs text-[#94a3b8] mb-4">{loadError}</p>
            <button
              onClick={retryEngine}
              className="px-4 py-2 bg-[#f43f5e] text-white rounded font-medium text-xs hover:bg-[#e11d48]"
            >
              Réessayer
            </button>
          </div>
        )}

        {!isLoaded && !loadError && (
          <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-[#050308]/90">
            <div className="w-8 h-8 border-2 border-[#a855f7] border-t-transparent rounded-full animate-spin" />
            <span className="text-xs text-[#c084fc] font-medium tracking-wide">
              Préparation du moteur natif KaractiRage…
            </span>
          </div>
        )}

        {isEditMode && (
          <div className="absolute top-3 left-3 z-30 flex items-center gap-1.5 px-2.5 py-1 bg-[#3b0764]/90 border border-[#a855f7] rounded-full text-[10px] font-bold uppercase tracking-wide text-[#e9d5ff] pointer-events-none">
            <PenTool size={11} className="text-[#a855f7]" />
            Mode Édition
          </div>
        )}

        <div
          ref={scene3dRef}
          className={`absolute inset-0 ${view3d ? '' : 'hidden'}`}
        />
        {view3d && !scene3dReady && !scene3dError && (
          <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-[#050308]/90">
            <div className="w-8 h-8 border-2 border-[#a855f7] border-t-transparent rounded-full animate-spin" />
            <span className="text-xs text-[#c084fc] font-medium tracking-wide">
              Compilation GPU…
            </span>
          </div>
        )}
        {view3d && scene3dError && (
          <div className="absolute inset-0 z-20 flex flex-col items-center justify-center p-6 text-center bg-[#050308]/90">
            <span className="text-[#f43f5e] font-bold text-base mb-2">Vue 3D indisponible</span>
            <p className="text-xs text-[#94a3b8] mb-4 max-w-md">{scene3dError}</p>
            <button
              onClick={() => onView3dChange(false)}
              className="px-4 py-2 bg-[#1E293B] text-white rounded font-medium text-xs hover:bg-[#2d1b4e]"
            >
              Revenir à la vue 2D
            </button>
          </div>
        )}
        {!view3d && (
          <canvas
            ref={canvasRef}
            data-testid="canvas-viewport"
            width={1200}
            height={800}
            onMouseMove={handleMouseMove}
            onMouseDown={handleMouseDown}
            onMouseUp={handleMouseUp}
            onDragOver={handleDragOver}
            onDrop={handleDrop}
            onContextMenu={e => e.preventDefault()}
            tabIndex={0}
            className={`w-full h-full object-contain shadow-2xl rounded border focus:outline-none transition-colors ${isEditMode ? 'border-[#a855f7] shadow-purple-950/60' : 'border-[#2d1b4e]'}`}
            style={{ imageRendering: 'pixelated' }}
          />
        )}
      </div>
    </div>
  );
};
