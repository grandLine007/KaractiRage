import { useCallback, useEffect, useRef, useState } from 'react';
import { Upload, Wand2, Save, Box, RefreshCw, MousePointer2, X } from 'lucide-react';
import {
  listVectorAssets, vectorLoad, vectorSave, vectorSpawn, vectorizeRaster, fileToPngBase64,
  DEFAULT_TRACE_OPTIONS, type VectorModel, type VectorAssetSummary, type TraceOptions,
} from '../services/vectorBridge';

interface NodeRef {
  li: number;
  pi: number;
  ni: number;
}

const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));
const totalPaths = (m: VectorModel) => m.layers.reduce((a, l) => a + l.paths.length, 0);
const round1 = (n: number) => Math.round(n * 10) / 10;

export const VectorWorkspace: React.FC = () => {
  const fileRef = useRef<HTMLInputElement | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const dragRef = useRef<NodeRef | null>(null);

  const [vectors, setVectors] = useState<VectorAssetSummary[]>([]);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [model, setModel] = useState<VectorModel | null>(null);
  const [sourceB64, setSourceB64] = useState<string | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [opts, setOpts] = useState<TraceOptions>(DEFAULT_TRACE_OPTIONS);
  const [editNodes, setEditNodes] = useState(false);
  const [idInput, setIdInput] = useState('import_01');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setVectors(await listVectorAssets());
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const loadVector = useCallback(async (id: string) => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const m = await vectorLoad(id);
      setCurrentId(id);
      setModel(m);
      setSourceB64(null);
      setPreviewUrl(null);
    } catch (e) {
      setError((e as Error)?.message ?? String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const doVectorize = useCallback(async (id: string, b64: string) => {
    setBusy(true);
    setError(null);
    try {
      const raw = await vectorizeRaster(id, b64, opts);
      const m = JSON.parse(raw) as VectorModel;
      setModel(m);
      setCurrentId(id);
      setMessage(`Vectorisé : ${m.width}×${m.height}, ${m.layers.length} couche(s), ${totalPaths(m)} chemin(s).`);
      await refresh();
    } catch (e) {
      setError((e as Error)?.message ?? String(e));
    } finally {
      setBusy(false);
    }
  }, [opts, refresh]);

  const onFile = useCallback(async (file: File) => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      if (file.type.includes('svg') || file.name.toLowerCase().endsWith('.svg')) {
        setError('SVG : importez un PNG/JPEG/WebP/GIF pour le vectoriser (un SVG est déjà vectoriel).');
        return;
      }
      const b64 = await fileToPngBase64(file);
      setSourceB64(b64);
      setPreviewUrl(URL.createObjectURL(file));
      await doVectorize(idInput, b64);
    } catch (e) {
      setError((e as Error)?.message ?? String(e));
    } finally {
      setBusy(false);
    }
  }, [doVectorize, idInput]);

  const save = useCallback(async () => {
    if (!model || !currentId) return;
    setBusy(true);
    setError(null);
    try {
      await vectorSave(currentId, model);
      setMessage(`Enregistré : assets/sprites/${currentId}.svg + assets/vectors/${currentId}.vec.json`);
    } catch (e) {
      setError((e as Error)?.message ?? String(e));
    } finally {
      setBusy(false);
    }
  }, [model, currentId]);

  const spawn = useCallback(async () => {
    if (!currentId) return;
    setBusy(true);
    setError(null);
    try {
      const id = await vectorSpawn(currentId);
      setMessage(`Entité #${id} posée dans la scène (SvgRef "${currentId}").`);
    } catch (e) {
      setError((e as Error)?.message ?? String(e));
    } finally {
      setBusy(false);
    }
  }, [currentId]);

  const onNodeDown = (li: number, pi: number, ni: number) => (e: React.PointerEvent) => {
    e.preventDefault();
    (e.target as Element).setPointerCapture(e.pointerId);
    dragRef.current = { li, pi, ni };
  };

  const onNodeMove = (e: React.PointerEvent) => {
    const d = dragRef.current;
    if (!d || !model || !svgRef.current) return;
    const svg = svgRef.current;
    const pt = svg.createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const ctm = svg.getScreenCTM();
    if (!ctm) return;
    const p = pt.matrixTransform(ctm.inverse());
    const next = clone(model);
    next.layers[d.li].paths[d.pi].points[d.ni] = [round1(p.x), round1(p.y)];
    setModel(next);
  };

  const onNodeUp = () => {
    dragRef.current = null;
  };
  return (
    <div className="flex flex-1 h-full min-h-0 bg-[#07040d] select-none">
      {/* Panneau gauche : import + réglages + bibliothèque */}
      <aside className="w-72 shrink-0 flex flex-col border-r border-[#2d1b4e] bg-[#0a0614]">
        <div className="flex items-center gap-2 px-3 py-2 border-b border-[#2d1b4e]">
          <span className="badge-tag bg-[#2e1065] text-[#c084fc] border border-[#a855f7]">VECTORISATION</span>
          <button onClick={() => void refresh()} title="Recharger" className="text-[#94a3b8] hover:text-[#f8fafc] transition"><RefreshCw size={13} /></button>
        </div>

        <div className="flex-1 overflow-y-auto p-3 flex flex-col gap-3">
          {/* Zone d'import */}
          <div
            onClick={() => fileRef.current?.click()}
            onDragOver={e => e.preventDefault()}
            onDrop={e => { e.preventDefault(); const f = e.dataTransfer.files?.[0]; if (f) void onFile(f); }}
            className="flex flex-col items-center justify-center gap-2 p-4 border-2 border-dashed border-[#3b0764] hover:border-[#a855f7] rounded-lg cursor-pointer transition bg-[#0d0819]"
          >
            <Upload size={20} className="text-[#a855f7]" />
            <span className="text-[11px] text-[#cbd5e1] text-center">Glisser une image (PNG/JPEG/WebP/GIF) ou cliquer pour importer</span>
            <input ref={fileRef} type="file" accept="image/png,image/jpeg,image/webp,image/gif" className="hidden" onChange={e => { const f = e.target.files?.[0]; if (f) void onFile(f); e.target.value = ''; }} />
          </div>

          <div className="flex gap-2">
            <input
              value={idInput}
              onChange={e => setIdInput(e.target.value)}
              placeholder="id (ex. import_01)"
              className="flex-1 bg-[#150d27] border border-[#2d1b4e] rounded px-2 py-1.5 text-[11px] text-[#cbd5e1]"
            />
            <button
              onClick={() => sourceB64 && void doVectorize(idInput, sourceB64)}
              disabled={!sourceB64 || busy}
              title="Re-vectoriser avec ces réglages"
              className="flex items-center gap-1 px-2.5 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] disabled:opacity-40 text-[#f8fafc] border border-[#3b0764] rounded-md text-[11px] font-medium transition"
            >
              <Wand2 size={13} className="text-[#38bdf8]" /> Re-vectoriser
            </button>
          </div>

          {/* Réglages de trace */}
          <div className="flex flex-col gap-2 bg-[#0d0819] border border-[#2d1b4e] rounded-lg p-2.5">
            <span className="text-[10px] font-bold uppercase tracking-wider text-[#94a3b8]">Réglages de trace</span>
            {[
              { k: 'colors' as const, label: 'Couleurs', min: 2, max: 16, step: 1 },
              { k: 'alpha_threshold' as const, label: 'Seuil alpha', min: 0, max: 255, step: 1 },
              { k: 'downscale_max' as const, label: 'Taille max (px)', min: 0, max: 512, step: 16 },
              { k: 'simplify' as const, label: 'Simplification (px)', min: 0, max: 8, step: 0.5 },
            ].map(f => (
              <label key={f.k} className="flex flex-col gap-1">
                <span className="flex justify-between text-[10px] text-[#94a3b8]">
                  <span>{f.label}</span>
                  <span className="font-mono text-[#cbd5e1]">{opts[f.k]}</span>
                </span>
                <input
                  type="range"
                  min={f.min} max={f.max} step={f.step}
                  value={opts[f.k]}
                  onChange={e => setOpts(o => ({ ...o, [f.k]: Number(e.target.value) }))}
                  className="accent-purple-500"
                />
              </label>
            ))}
          </div>

          {/* Bibliothèque */}
          <div className="flex flex-col gap-1.5">
            <span className="text-[10px] font-bold uppercase tracking-wider text-[#94a3b8]">Vecteurs ({vectors.length})</span>
            {vectors.length === 0 ? (
              <span className="text-[10px] text-[#94a3b8]">Aucun vecteur — importez une image.</span>
            ) : (
              vectors.map(v => (
                <button
                  key={v.id}
                  onClick={() => void loadVector(v.id)}
                  className={`flex items-center justify-between px-2 py-1.5 rounded text-left text-[11px] transition ${v.id === currentId ? 'bg-[#3b0764] text-[#f3e8ff]' : 'text-[#cbd5e1] hover:bg-[#1e1338]'}`}
                >
                  <span className="truncate">{v.id}</span>
                  <span className="text-[8px] text-[#94a3b8]">vec</span>
                </button>
              ))
            )}
          </div>
        </div>
      </aside>
      {/* Zone centrale : aperçu + édition */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="flex items-center gap-2 px-3 py-2 border-b border-[#2d1b4e] bg-[#0d0819]">
          <button
            onClick={() => setEditNodes(v => !v)}
            className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium border transition ${editNodes ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7]' : 'bg-[#150d27] text-[#94a3b8] border-[#2d1b4e] hover:text-[#f8fafc]'}`}
            title="Afficher et déplacer les nœuds des chemins"
          >
            <MousePointer2 size={13} className={editNodes ? 'text-[#a855f7]' : ''} />
            {editNodes ? 'Nœuds : ON' : 'Éditer les nœuds'}
          </button>
          <div className="flex-1" />
          {message && <span className="text-[10px] text-[#34d399] truncate max-w-[40%]">{message}</span>}
          {error && (
            <span className="flex items-center gap-1 text-[10px] text-[#f87171]">
              <span className="truncate max-w-[40vw]">{error}</span>
              <button onClick={() => setError(null)} className="hover:text-white"><X size={12} /></button>
            </span>
          )}
          <button
            onClick={() => void save()}
            disabled={!model || !currentId || busy}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] disabled:opacity-40 text-[#f8fafc] border border-[#3b0764] rounded-md text-xs font-medium transition"
          >
            <Save size={13} className="text-[#38bdf8]" /> Enregistrer
          </button>
          <button
            onClick={() => void spawn()}
            disabled={!currentId || busy}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] disabled:opacity-40 text-white rounded-md text-xs font-semibold transition"
          >
            <Box size={13} /> Poser dans la scène
          </button>
        </div>

        <div
          className="flex-1 overflow-auto p-4 flex items-start justify-center"
          style={{ backgroundImage: 'radial-gradient(circle at 1px 1px, #2d1b4e 1px, transparent 0)', backgroundSize: '24px 24px' }}
        >
          {model ? (
            <div className="relative">
              <svg
                ref={svgRef}
                viewBox={`0 0 ${model.width} ${model.height}`}
                style={{ width: model.width * 2, height: model.height * 2, imageRendering: 'pixelated' }}
                className="border border-[#3b0764] bg-[#0d0819] shadow-2xl shadow-purple-950/50"
                onPointerMove={onNodeMove}
                onPointerUp={onNodeUp}
                onPointerLeave={onNodeUp}
              >
                {model.layers.map((l, li) => {
                  const d = l.paths
                    .filter(p => p.points.length >= 2)
                    .map(p => `M ${p.points[0][0]} ${p.points[0][1]} ` + p.points.slice(1).map(([x, y]) => `L ${x} ${y}`).join(' ') + (p.closed ? ' Z' : ''))
                    .join(' ');
                  return d ? <path key={li} d={d} fill={l.color} fillRule="evenodd" stroke="#0d0819" strokeWidth="0.4" /> : null;
                })}
                {editNodes && model.layers.map((l, li) =>
                  l.paths.map((p, pi) =>
                    p.points.map(([x, y], ni) => (
                      <circle
                        key={`${li}-${pi}-${ni}`}
                        cx={x}
                        cy={y}
                        r={2.5}
                        fill="#f8fafc"
                        stroke="#a855f7"
                        strokeWidth={1}
                        className="cursor-move"
                        onPointerDown={onNodeDown(li, pi, ni)}
                      />
                    )),
                  ),
                )}
              </svg>
              {previewUrl && (
                <img
                  src={previewUrl}
                  alt="source"
                  className="absolute top-0 left-0 opacity-40 pointer-events-none"
                  style={{ width: model.width * 2, height: model.height * 2, imageRendering: 'pixelated' }}
                />
              )}
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center gap-2 text-[#94a3b8] py-20">
              <Wand2 size={28} className="opacity-50" />
              <p className="text-sm">Importez une image pour la vectoriser.</p>
              <p className="text-[11px]">Le tracé combine quantification, contours et simplification.</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};