import { useEffect, useRef, useState, useCallback } from 'react';
import { Play, Pause, Crosshair, Grid3X3 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface GameViewportProps {
  onTelemetryUpdate?: (metrics: { fps: number; cpuUs: number; score: number; status: string }) => void;
  onSelectedEntityChange?: (entityName: string | null) => void;
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

  const animFrameIdRef = useRef<number | null>(null);
  const lastTimeRef = useRef<number>(performance.now());
  const canvasWidthRef = useRef<number>(1200);
  const canvasHeightRef = useRef<number>(800);
  // Position souris en attente d'envoi (coalescé : 1 IPC par frame)
  const pendingMouseRef = useRef<{ x: number; y: number } | null>(null);

  // Initialisation du viewport (le moteur natif est déjà démarré côté backend Tauri)
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
      } catch (err: any) {
        console.error("Erreur d'initialisation du viewport KaractiRage:", err);
        if (active) {
          setLoadError(err?.message || "Impossible de joindre le moteur natif Tauri");
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

  // Boucle de rendu 60 FPS (pixel-streaming depuis le moteur natif)
  useEffect(() => {
    if (!isLoaded || isPaused) return;

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
  }, [isLoaded, isPaused, onTelemetryUpdate]);

  // Gestion des entrées clavier
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (['input', 'textarea'].includes((e.target as HTMLElement)?.tagName?.toLowerCase())) {
        return;
      }

      const keyVal = KEY_MAP[e.code];
      if (keyVal !== undefined) {
        void invoke('engine_on_key_down', { key: keyVal });
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

  // Gestion des coordonnées souris (coalescée : stocke la position, envoyée par la boucle de rendu)
  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    pendingMouseRef.current = {
      x: (e.clientX - rect.left) * scaleX,
      y: (e.clientY - rect.top) * scaleY,
    };
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = (e.clientX - rect.left) * scaleX;
    const y = (e.clientY - rect.top) * scaleY;
    void invoke('engine_on_mouse_move', { x, y });
    const button = e.button === 2 ? 2 : 0;
    void invoke('engine_on_mouse_down', { button });
  }, []);

  const handleMouseUp = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const button = e.button === 2 ? 2 : 0;
    void invoke('engine_on_mouse_up', { button });
  }, []);

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
