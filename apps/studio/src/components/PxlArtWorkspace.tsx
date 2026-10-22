import { useCallback, useEffect, useState } from 'react';
import { Hammer, Play, Square, FilePlus } from 'lucide-react';
import {
  pxlAvailable,
  pxlBuild,
  pxlListSources,
  pxlWriteSource,
  pxlServeStart,
  pxlServeStop,
  pxlServeStatus,
  type PxlBuildResult,
} from '../services/engineBridge';
import { newArtSource } from '../services/pxlSource';
import { PixelEditor } from './PixelEditor';

const PORT = 4321;

/**
 * Atelier Pixel Art (pxl) : éditeur de sprites + build des PNG + viewer live.
 * L'édition se fait sur assets/art/<source>.px.js ; « Enregistrer & Build »
 * régénère les PNG dans assets/generated.
 */
export const PxlArtWorkspace: React.FC = () => {
  const [available, setAvailable] = useState<boolean | null>(null);
  const [sources, setSources] = useState<string[]>([]);
  const [source, setSource] = useState<string>('game');
  const [serveUrl, setServeUrl] = useState<string | null>(null);
  const [build, setBuild] = useState<PxlBuildResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshSources = useCallback(async () => {
    setSources(await pxlListSources());
  }, []);

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
    void refreshSources();
    void refreshStatus();
    const timer = window.setInterval(() => void refreshStatus(), 1500);
    return () => window.clearInterval(timer);
  }, [refreshSources, refreshStatus]);

  const handleBuild = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await pxlBuild(source);
      setBuild(r);
      if (!r.ok) setError(r.stderr.trim() || r.stdout.trim() || 'Build en échec.');
    } catch (e) {
      setError((e as Error)?.message ?? 'Erreur de build');
    } finally {
      setBusy(false);
    }
  }, [source]);

  const handleServe = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const url = await pxlServeStart(source, PORT);
      setServeUrl(url);
    } catch (e) {
      setError((e as Error)?.message ?? 'Erreur de serveur');
    } finally {
      setBusy(false);
    }
  }, [source]);

  const handleStop = useCallback(async () => {
    await pxlServeStop();
    setServeUrl(null);
  }, []);

  const handleNew = useCallback(async () => {
    const name = window.prompt("Nom de la nouvelle source d'art :", 'perso');
    if (!name || !/^[A-Za-z0-9_-]+$/.test(name)) return;
    setBusy(true);
    setError(null);
    try {
      await pxlWriteSource(name, newArtSource(name));
      await refreshSources();
      setSource(name);
    } catch (e) {
      setError((e as Error)?.message ?? 'Erreur de création');
    } finally {
      setBusy(false);
    }
  }, [refreshSources]);

  const disabled = busy || available === false;

  return (
    <div className="flex flex-col flex-1 h-full bg-[#07040d] select-none">
      {/* Barre d'outils */}
      <div className="flex items-center justify-between px-4 py-2 bg-[#0d0819] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2">
          <span className="badge-tag bg-[#2e1065] text-[#c084fc] border border-[#a855f7]">
            ATELIER PIXEL ART
          </span>
          {available === false && (
            <span className="text-[11px] text-[#94a3b8]">
              ⚠️ Node introuvable — installe Node ≥ 22.14 pour Build/Serve
            </span>
          )}
          <select
            value={source}
            onChange={e => setSource(e.target.value)}
            className="bg-[#150d27] border border-[#2d1b4e] rounded px-1.5 py-1 text-[11px] text-[#cbd5e1]"
          >
            {sources.map(s => <option key={s} value={s}>{s}</option>)}
          </select>
          <button
            onClick={handleNew}
            disabled={busy}
            title="Nouvelle source d'art"
            className="flex items-center gap-1 px-2 py-1 bg-[#1e1338] hover:bg-[#2d1b4e] disabled:opacity-40 text-[#f8fafc] border border-[#3b0764] rounded-md text-[11px] font-medium transition"
          >
            <FilePlus size={13} />
          </button>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleBuild}
            disabled={disabled}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] disabled:opacity-40 text-[#f8fafc] border border-[#3b0764] rounded-md text-xs font-medium transition"
            title={`Régénérer les PNG depuis assets/art/${source}.px.js`}
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

      {/* Éditeur pixel-art (sprites de la source sélectionnée) */}
      <PixelEditor sourceName={source} onSaved={() => void refreshStatus()} />

      {/* Viewer live (overlay bas quand pxl serve tourne) */}
      {serveUrl && (
        <div className="relative h-40 shrink-0 border-t border-[#2d1b4e]">
          <iframe src={serveUrl} title="pxl live viewer" className="absolute inset-0 w-full h-full border-0 bg-white" />
        </div>
      )}
    </div>
  );
};
