import { useCallback, useEffect, useState } from 'react';
import { Hammer, Play, Square, RefreshCw } from 'lucide-react';
import {
  pxlAvailable,
  pxlBuild,
  pxlServeStart,
  pxlServeStop,
  pxlServeStatus,
  type PxlBuildResult,
} from '../services/engineBridge';

const ART_NAME = 'game';
const PORT = 4321;

/**
 * Atelier Pixel Art (pxl) : build des PNG + viewer live (`pxl serve`) en iframe.
 * Le viewer rejoue le dessin coup par coup à chaque sauvegarde de assets/art/game.px.js.
 */
export const PxlArtWorkspace: React.FC = () => {
  const [available, setAvailable] = useState<boolean | null>(null);
  const [serveUrl, setServeUrl] = useState<string | null>(null);
  const [build, setBuild] = useState<PxlBuildResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await pxlServeStatus(PORT);
      setServeUrl(s.running ? s.url : null);
    } catch {
      // moteur non prêt
    }
  }, []);

  useEffect(() => {
    pxlAvailable().then(setAvailable).catch(() => setAvailable(false));
    void refreshStatus();
    const timer = window.setInterval(() => void refreshStatus(), 1500);
    return () => window.clearInterval(timer);
  }, [refreshStatus]);

  const handleBuild = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await pxlBuild(ART_NAME);
      setBuild(r);
      if (!r.ok) setError(r.stderr.trim() || r.stdout.trim() || 'Build en échec.');
    } catch (e) {
      setError((e as Error)?.message ?? 'Erreur de build');
    } finally {
      setBusy(false);
    }
  }, []);

  const handleServe = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const url = await pxlServeStart(ART_NAME, PORT);
      setServeUrl(url);
    } catch (e) {
      setError((e as Error)?.message ?? 'Erreur de serveur');
    } finally {
      setBusy(false);
    }
  }, []);

  const handleStop = useCallback(async () => {
    await pxlServeStop();
    setServeUrl(null);
  }, []);

  const disabled = busy || available === false;

  return (
    <div className="flex flex-col flex-1 h-full bg-[#07040d] select-none">
      {/* Barre d'outils */}
      <div className="flex items-center justify-between px-4 py-2 bg-[#0d0819] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2">
          <span className="badge-tag bg-[#2e1065] text-[#c084fc] border border-[#a855f7]">
            ATELIER PIXEL ART
          </span>
          <span className="text-[11px] text-[#94a3b8]">
            {available === false
              ? '⚠️ Node introuvable — installe Node ≥ 22.14 pour Build/Serve'
              : `source : assets/art/${ART_NAME}.px.js`}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleBuild}
            disabled={disabled}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] disabled:opacity-40 text-[#f8fafc] border border-[#3b0764] rounded-md text-xs font-medium transition"
            title="Régénérer les PNG depuis assets/art/game.px.js"
          >
            <Hammer size={14} className="text-[#38bdf8]" />
            {busy ? 'Build…' : 'Build'}
          </button>

          {serveUrl ? (
            <button
              onClick={handleStop}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-[#881337] hover:bg-[#9f1239] text-[#fecdd3] rounded-md text-xs font-semibold transition"
              title="Arrêter le viewer live"
            >
              <Square size={14} />
              Stop
            </button>
          ) : (
            <button
              onClick={handleServe}
              disabled={disabled}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] disabled:opacity-40 text-white rounded-md text-xs font-semibold transition"
              title="Lancer le viewer live (http://127.0.0.1:4321)"
            >
              <Play size={14} className="text-[#f3e8ff]" />
              Serve (viewer live)
            </button>
          )}
        </div>
      </div>

      {/* Sortie du build */}
      {build && (
        <div className="px-4 py-1.5 bg-[#0d0819] border-b border-[#2d1b4e] text-[11px] font-mono text-[#94a3b8] whitespace-pre-wrap max-h-28 overflow-auto">
          {build.stdout}
          {build.stderr}
        </div>
      )}
      {error && (
        <div className="px-4 py-1.5 bg-[#2a0f0f] border-b border-[#7f1d1d] text-[11px] text-[#fca5a5] whitespace-pre-wrap">
          {error}
        </div>
      )}

      {/* Zone du viewer */}
      <div className="flex-1 relative">
        {serveUrl ? (
          <iframe
            src={serveUrl}
            title="pxl live viewer"
            className="absolute inset-0 w-full h-full border-0 bg-white"
          />
        ) : (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-[#64748b]">
            <RefreshCw size={28} className="opacity-50" />
            <p className="text-sm">Lancez « Serve (viewer live) » pour regarder pxl peindre en direct.</p>
            <p className="text-[11px]">Le viewer rejoue chaque asset coup par coup à chaque sauvegarde.</p>
          </div>
        )}
      </div>
    </div>
  );
};
