import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Pencil, Eraser, PaintBucket, Pipette, Undo2, Redo2, ZoomIn, ZoomOut,
  Save, Plus, Copy, Trash2, Slash, Square, PencilRuler,
} from 'lucide-react';
import { pxlReadSource, pxlWriteSource, pxlBuild } from '../services/engineBridge';
import { parseArt, buildSource, type PixelPalette, type SpriteDef } from '../services/pxlSource';

type Tool = 'pencil' | 'eraser' | 'fill' | 'picker' | 'line' | 'rect';

interface Props {
  sourceName: string;
  onSaved?: () => void;
}

const TOOLS: { id: Tool; icon: React.ReactNode; title: string }[] = [
  { id: 'pencil', icon: <Pencil size={14} />, title: 'Crayon (P)' },
  { id: 'eraser', icon: <Eraser size={14} />, title: 'Gomme (E)' },
  { id: 'fill', icon: <PaintBucket size={14} />, title: 'Remplissage (F)' },
  { id: 'picker', icon: <Pipette size={14} />, title: 'Pipette (I)' },
  { id: 'line', icon: <Slash size={14} />, title: 'Ligne (L)' },
  { id: 'rect', icon: <Square size={14} />, title: 'Rectangle (R)' },
];

export const PixelEditor: React.FC<Props> = ({ sourceName, onSaved }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const dragRef = useRef<{ x0: number; y0: number; snapshot: string[] } | null>(null);
  const originalRef = useRef<string>('');

  const [palettes, setPalettes] = useState<PixelPalette[]>([]);
  const [sprites, setSprites] = useState<SpriteDef[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [grid, setGrid] = useState<string[]>([]);
  const [tool, setTool] = useState<Tool>('pencil');
  const [color, setColor] = useState('k');
  const [zoom, setZoom] = useState(16);
  const [undoStack, setUndoStack] = useState<string[][]>([]);
  const [redoStack, setRedoStack] = useState<string[][]>([]);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setBusy(true);
    setMessage(null);
    pxlReadSource(sourceName)
      .then(src => {
        if (!alive) return;
        originalRef.current = src;
        const art = parseArt(src);
        setPalettes(art.palettes);
        setSprites(art.sprites);
        const first = art.sprites.find(s => s.kind === 'grid');
        if (first) {
          setSelectedId(first.id);
          setGrid(first.grid.slice());
        } else {
          setSelectedId(null);
          setGrid([]);
        }
        setUndoStack([]);
        setRedoStack([]);
        setDirty(false);
      })
      .catch(e => { if (alive) setMessage(`Erreur de lecture : ${e}`); })
      .finally(() => { if (alive) setBusy(false); });
    return () => { alive = false; };
  }, [sourceName]);

  const selected = useMemo(() => sprites.find(s => s.id === selectedId) ?? null, [sprites, selectedId]);
  const colorHex = useMemo(() => palettes.find(p => p.key === color)?.hex ?? '#888888', [palettes, color]);

  useEffect(() => {
    const cv = canvasRef.current;
    if (!cv) return;
    const h = grid.length;
    const w = grid[0]?.length ?? 0;
    cv.width = Math.max(1, w) * zoom;
    cv.height = Math.max(1, h) * zoom;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        ctx.fillStyle = (x + y) % 2 === 0 ? '#171027' : '#221738';
        ctx.fillRect(x * zoom, y * zoom, zoom, zoom);
        const key = grid[y]?.[x] ?? '.';
        if (key === '.' || key === ' ') continue;
        const hex = palettes.find(p => p.key === key)?.hex;
        if (!hex) continue;
        ctx.fillStyle = hex;
        ctx.fillRect(x * zoom, y * zoom, zoom, zoom);
      }
    }
    if (zoom >= 8) {
      ctx.strokeStyle = 'rgba(148,163,184,0.12)';
      ctx.lineWidth = 1;
      for (let x = 0; x <= w; x++) {
        ctx.beginPath(); ctx.moveTo(x * zoom + 0.5, 0); ctx.lineTo(x * zoom + 0.5, h * zoom); ctx.stroke();
      }
      for (let y = 0; y <= h; y++) {
        ctx.beginPath(); ctx.moveTo(0, y * zoom + 0.5); ctx.lineTo(w * zoom, y * zoom + 0.5); ctx.stroke();
      }
    }
  }, [grid, palettes, zoom]);
  const applyGrid = useCallback((next: string[], withUndo = true) => {
    if (withUndo) {
      setUndoStack(s => [...s.slice(-49), grid]);
      setRedoStack([]);
    }
    setGrid(next);
    setDirty(true);
  }, [grid]);

  const getCell = (g: string[], x: number, y: number) => g[y]?.[x] ?? '.';

  const setCell = (g: string[], x: number, y: number, key: string) => {
    if (y < 0 || y >= g.length) return g;
    g[y] = g[y].slice(0, x) + key + g[y].slice(x + 1);
    return g;
  };

  const floodFill = (g: string[], x: number, y: number, key: string) => {
    const target = getCell(g, x, y);
    if (target === key) return g;
    const h = g.length, w = g[0]?.length ?? 0;
    const stack = [[x, y]];
    while (stack.length) {
      const [cx, cy] = stack.pop()!;
      if (cx < 0 || cy < 0 || cx >= w || cy >= h) continue;
      if (getCell(g, cx, cy) !== target) continue;
      g = setCell(g, cx, cy, key);
      stack.push([cx + 1, cy], [cx - 1, cy], [cx, cy + 1], [cx, cy - 1]);
    }
    return g;
  };

  const drawLine = (g: string[], x0: number, y0: number, x1: number, y1: number, key: string) => {
    const dx = Math.abs(x1 - x0), dy = Math.abs(y1 - y0);
    const sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1;
    let err = dx - dy, x = x0, y = y0;
    for (;;) {
      g = setCell(g, x, y, key);
      if (x === x1 && y === y1) break;
      const e2 = 2 * err;
      if (e2 > -dy) { err -= dy; x += sx; }
      if (e2 < dx) { err += dx; y += sy; }
    }
    return g;
  };

  const drawRect = (g: string[], x0: number, y0: number, x1: number, y1: number, key: string) => {
    const [xa, xb] = x0 <= x1 ? [x0, x1] : [x1, x0];
    const [ya, yb] = y0 <= y1 ? [y0, y1] : [y1, y0];
    for (let x = xa; x <= xb; x++) { g = setCell(g, x, ya, key); g = setCell(g, x, yb, key); }
    for (let y = ya; y <= yb; y++) { g = setCell(g, xa, y, key); g = setCell(g, xb, y, key); }
    return g;
  };

  const cellFromEvent = (e: React.MouseEvent) => {
    const rect = canvasRef.current!.getBoundingClientRect();
    return {
      x: Math.floor((e.clientX - rect.left) / zoom),
      y: Math.floor((e.clientY - rect.top) / zoom),
    };
  };

  const onPointerDown = (e: React.MouseEvent) => {
    e.preventDefault();
    const { x, y } = cellFromEvent(e);
    if (tool === 'line' || tool === 'rect') {
      dragRef.current = { x0: x, y0: y, snapshot: grid.map(r => r) };
      return;
    }
    if (tool === 'picker') {
      const k = getCell(grid, x, y);
      if (k !== '.' && k !== ' ') setColor(k);
      return;
    }
    const key = tool === 'eraser' ? '.' : color;
    if (tool === 'fill') {
      applyGrid(floodFill(grid.map(r => r), x, y, key));
    } else {
      if (getCell(grid, x, y) === key) return;
      applyGrid(setCell(grid.map(r => r), x, y, key));
    }
  };

  const onPointerMove = (e: React.MouseEvent) => {
    if (e.buttons !== 1) return;
    const { x, y } = cellFromEvent(e);
    if (dragRef.current) {
      const d = dragRef.current;
      const key = tool === 'eraser' ? '.' : color;
      setGrid(tool === 'line'
        ? drawLine(d.snapshot.map(r => r), d.x0, d.y0, x, y, key)
        : drawRect(d.snapshot.map(r => r), d.x0, d.y0, x, y, key));
      setDirty(true);
      return;
    }
    if (tool === 'picker' || tool === 'fill') return;
    const key = tool === 'eraser' ? '.' : color;
    if (getCell(grid, x, y) === key) return;
    setGrid(setCell(grid.map(r => r), x, y, key));
    setDirty(true);
  };

  const onPointerUp = () => {
    if (dragRef.current) {
      setUndoStack(s => [...s.slice(-49), dragRef.current!.snapshot]);
      setRedoStack([]);
      setDirty(true);
      dragRef.current = null;
    }
  };
  const undo = () => {
    setUndoStack(s => {
      if (!s.length) return s;
      setRedoStack(r => [...r, grid]);
      setGrid(s[s.length - 1]);
      setDirty(true);
      return s.slice(0, -1);
    });
  };

  const redo = () => {
    setRedoStack(r => {
      if (!r.length) return r;
      setUndoStack(s => [...s, grid]);
      setGrid(r[r.length - 1]);
      setDirty(true);
      return r.slice(0, -1);
    });
  };

  const selectSprite = (id: string) => {
    const s = sprites.find(x => x.id === id);
    if (!s || s.kind !== 'grid') return;
    setSelectedId(id);
    setGrid(s.grid.slice());
    setUndoStack([]);
    setRedoStack([]);
  };

  const addSprite = () => {
    const base = selectedId ?? 'sprite_01';
    let id = base, n = 2;
    while (sprites.some(s => s.id === id)) id = `${base}_${n++}`;
    const g = Array.from({ length: 16 }, () => '.'.repeat(16));
    const def: SpriteDef = { id, kind: 'grid', opts: '{ outline: "k" }', grid: g, raw: '', start: -1, end: -1 };
    setSprites(s => [...s, def]);
    setSelectedId(id);
    setGrid(g.slice());
    setUndoStack([]);
    setRedoStack([]);
    setDirty(true);
  };

  const duplicateSprite = () => {
    const src = sprites.find(s => s.id === selectedId);
    if (!src) return;
    let id = `${src.id}_copy`, n = 2;
    while (sprites.some(s => s.id === id)) id = `${src.id}_copy${n++}`;
    const def: SpriteDef = { id, kind: src.kind, opts: src.opts, grid: src.grid.map(r => r), raw: '', start: -1, end: -1 };
    setSprites(s => [...s, def]);
    setSelectedId(id);
    setGrid(def.grid.slice());
    setUndoStack([]);
    setRedoStack([]);
    setDirty(true);
  };

  const renameSprite = () => {
    const name = window.prompt('Nouveau nom du sprite :', selectedId ?? '');
    if (!name || !/^[A-Za-z0-9_]+$/.test(name)) return;
    if (sprites.some(s => s.id === name)) { setMessage('Nom déjà utilisé.'); return; }
    setSprites(s => s.map(x => x.id === selectedId ? { ...x, id: name } : x));
    setSelectedId(name);
    setDirty(true);
  };

  const deleteSprite = () => {
    if (!selectedId) return;
    const remaining = sprites.filter(x => x.id !== selectedId);
    setSprites(remaining);
    const next = remaining.find(x => x.kind === 'grid');
    if (next) { setSelectedId(next.id); setGrid(next.grid.slice()); }
    else { setSelectedId(null); setGrid([]); }
    setUndoStack([]);
    setRedoStack([]);
    setDirty(true);
  };

  const save = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const merged = sprites.map(s => s.id === selectedId ? { ...s, grid: grid.slice() } : s);
      setSprites(merged);
      const src = buildSource(originalRef.current, palettes, merged);
      await pxlWriteSource(sourceName, src);
      originalRef.current = src;
      const r = await pxlBuild(sourceName);
      setMessage(r.ok ? `Build OK (${r.duration_ms}ms)` : (r.stderr.trim() || r.stdout.trim() || 'Build en échec.'));
      setDirty(false);
      onSaved?.();
    } catch (err) {
      setMessage(`Erreur : ${(err as Error)?.message ?? String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const editColorHex = (hex: string) => {
    setPalettes(p => p.map(x => x.key === color ? { ...x, hex } : x));
    setDirty(true);
  };

  const addColor = () => {
    const hex = window.prompt('Couleur hex (#rrggbb) :', '#ffffff');
    if (!hex || !/^#[0-9a-fA-F]{6}$/.test(hex)) return;
    const keys = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    let key = '';
    for (const c of keys) { if (!palettes.some(p => p.key === c) && !'._ #'.includes(c)) { key = c; break; } }
    if (!key) { setMessage('Plus de clé libre.'); return; }
    setPalettes(p => [...p, { key, hex }]);
    setColor(key);
    setDirty(true);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
      const map: Record<string, Tool> = { p: 'pencil', e: 'eraser', f: 'fill', i: 'picker', l: 'line', r: 'rect' };
      const k = e.key.toLowerCase();
      if (map[k]) { setTool(map[k]); return; }
      if ((e.ctrlKey || e.metaKey) && k === 'z') { e.preventDefault(); if (e.shiftKey) redo(); else undo(); return; }
      if ((e.ctrlKey || e.metaKey) && k === 's') { e.preventDefault(); void save(); return; }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });
  if (!selected) {
    return (
      <div className="flex-1 flex items-center justify-center text-[#64748b] text-sm">
        {busy ? 'Chargement…' : 'Aucun sprite éditable dans cette source.'}
      </div>
    );
  }

  return (
    <div className="flex flex-1 h-full min-h-0">
      {/* Liste des sprites */}
      <aside className="w-44 shrink-0 flex flex-col border-r border-[#2d1b4e] bg-[#0a0614]">
        <div className="flex items-center justify-between px-2 py-1.5 border-b border-[#2d1b4e]">
          <span className="text-[10px] font-bold uppercase tracking-wider text-[#64748b]">Sprites</span>
          <div className="flex gap-1">
            <button onClick={addSprite} title="Nouveau sprite" className="text-[#94a3b8] hover:text-[#f8fafc]"><Plus size={13} /></button>
            <button onClick={duplicateSprite} title="Dupliquer" className="text-[#94a3b8] hover:text-[#f8fafc]"><Copy size={13} /></button>
            <button onClick={renameSprite} title="Renommer" className="text-[#94a3b8] hover:text-[#f8fafc]"><PencilRuler size={13} /></button>
            <button onClick={deleteSprite} title="Supprimer" className="text-[#94a3b8] hover:text-[#f87171]"><Trash2 size={13} /></button>
          </div>
        </div>
        <div className="flex-1 overflow-y-auto p-1.5 flex flex-col gap-0.5">
          {sprites.map(s => (
            <button
              key={s.id}
              onClick={() => selectSprite(s.id)}
              className={`flex items-center justify-between px-2 py-1 rounded text-left text-[11px] transition ${
                s.id === selectedId ? 'bg-[#3b0764] text-[#f3e8ff]' : 'text-[#cbd5e1] hover:bg-[#1e1338]'
              }`}
            >
              <span className="truncate">{s.id}</span>
              <span className={`text-[8px] uppercase ${s.kind === 'grid' ? 'text-[#a855f7]' : 'text-[#64748b]'}`}>
                {s.kind === 'grid' ? `${s.grid[0]?.length ?? 0}×${s.grid.length}` : 'proc'}
              </span>
            </button>
          ))}
        </div>
      </aside>

      {/* Zone de dessin */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="flex items-center gap-1 px-2 py-1.5 border-b border-[#2d1b4e] bg-[#0d0819]">
          {TOOLS.map(t => (
            <button
              key={t.id}
              onClick={() => setTool(t.id)}
              title={t.title}
              className={`p-1.5 rounded-md border transition ${tool === t.id ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7]' : 'text-[#94a3b8] border-transparent hover:bg-[#1e1338]'}`}
            >
              {t.icon}
            </button>
          ))}
          <span className="mx-1 h-4 w-px bg-[#2d1b4e]" />
          <span className="w-6 h-6 rounded border border-[#2d1b4e] cursor-pointer" style={{ backgroundColor: colorHex }} title={`Couleur courante : ${color}`} />
          <span className="text-[11px] font-mono text-[#cbd5e1]">{color}</span>
          <span className="mx-1 h-4 w-px bg-[#2d1b4e]" />
          <button onClick={undo} disabled={!undoStack.length} title="Annuler (Ctrl+Z)" className="p-1.5 text-[#94a3b8] disabled:opacity-30 hover:text-[#f8fafc]"><Undo2 size={14} /></button>
          <button onClick={redo} disabled={!redoStack.length} title="Rétablir (Ctrl+Shift+Z)" className="p-1.5 text-[#94a3b8] disabled:opacity-30 hover:text-[#f8fafc]"><Redo2 size={14} /></button>
          <span className="mx-1 h-4 w-px bg-[#2d1b4e]" />
          <button onClick={() => setZoom(z => Math.max(8, z / 2))} title="Zoom -" className="p-1.5 text-[#94a3b8] hover:text-[#f8fafc]"><ZoomOut size={14} /></button>
          <span className="text-[10px] font-mono text-[#64748b] w-10 text-center">{zoom}×</span>
          <button onClick={() => setZoom(z => Math.min(64, z * 2))} title="Zoom +" className="p-1.5 text-[#94a3b8] hover:text-[#f8fafc]"><ZoomIn size={14} /></button>
          <div className="flex-1" />
          {dirty && <span className="text-[10px] text-[#f59e0b]">non enregistré</span>}
          <button
            onClick={() => void save()}
            disabled={busy}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] disabled:opacity-40 text-white rounded-md text-xs font-semibold transition"
          >
            <Save size={14} />
            {busy ? 'Enregistrement…' : 'Enregistrer & Build'}
          </button>
        </div>

        {message && <div className="px-3 py-1 text-[11px] font-mono text-[#94a3b8] bg-[#0d0819] border-b border-[#2d1b4e]">{message}</div>}

        <div className="flex-1 overflow-auto p-4 flex items-start justify-center bg-[#0a0614]" style={{ backgroundImage: 'radial-gradient(circle at 1px 1px, #2d1b4e 1px, transparent 0)', backgroundSize: '24px 24px' }}>
          <canvas
            ref={canvasRef}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerLeave={onPointerUp}
            className="cursor-crosshair border border-[#3b0764] shadow-2xl shadow-purple-950/50"
            style={{ touchAction: 'none' }}
          />
        </div>
      </div>
      {/* Palette */}
      <aside className="w-36 shrink-0 flex flex-col border-l border-[#2d1b4e] bg-[#0a0614]">
        <div className="flex items-center justify-between px-2 py-1.5 border-b border-[#2d1b4e]">
          <span className="text-[10px] font-bold uppercase tracking-wider text-[#64748b]">Palette</span>
          <button onClick={addColor} title="Ajouter une couleur" className="text-[#94a3b8] hover:text-[#f8fafc]"><Plus size={13} /></button>
        </div>
        <div className="flex-1 overflow-y-auto p-1.5 grid grid-cols-2 gap-1 content-start">
          {palettes.map(p => (
            <button
              key={p.key}
              onClick={() => setColor(p.key)}
              title={`${p.key} = ${p.hex}`}
              className={`flex flex-col items-center gap-0.5 p-1 rounded-md border ${color === p.key ? 'border-[#a855f7] bg-[#1e1338]' : 'border-[#2d1b4e]'}`}
            >
              <span className="w-5 h-5 rounded-sm border border-black/40" style={{ backgroundColor: p.hex }} />
              <span className="text-[9px] font-mono text-[#cbd5e1]">{p.key}</span>
            </button>
          ))}
        </div>
        <div className="border-t border-[#2d1b4e] p-1.5">
          <div className="flex items-center gap-1">
            <input
              type="color"
              value={/^#[0-9a-fA-F]{6}$/.test(colorHex) ? colorHex : '#888888'}
              onChange={e => editColorHex(e.target.value)}
              className="w-7 h-7 rounded bg-transparent border border-[#2d1b4e] cursor-pointer"
              title="Modifier la couleur sélectionnée"
            />
            <input
              type="text"
              value={colorHex}
              onChange={e => editColorHex(e.target.value)}
              className="flex-1 bg-[#150d27] border border-[#2d1b4e] rounded px-1.5 py-1 text-[10px] font-mono text-[#cbd5e1]"
            />
          </div>
        </div>
      </aside>
    </div>
  );
};