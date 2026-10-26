import { useCallback, useEffect, useState } from 'react';
import { Coins, FlaskConical, KeyRound, Package, DoorOpen, Square, TriangleAlert, Box, CircleDot, Skull, BrickWall, RefreshCw, Boxes } from 'lucide-react';
import {
  listSpriteAssets,
  listAnimAssets,
  listSheetAssets,
  listMeshAssets,
  getSpritePngUrl,
  getSheetPngUrl,
  type SpriteAsset,
  type AnimAsset,
  type SheetAsset,
  type MeshAssetSummary,
} from '../services/engineBridge';

interface SpawnPreset {
  id: string;
  label: string;
  desc: string;
  color: string;
  icon: React.ReactNode;
}

const PRESETS: SpawnPreset[] = [
  { id: 'coin', label: 'Pièce', desc: '+10 points au contact', color: '#f59e0b', icon: <Coins size={16} /> },
  { id: 'potion', label: 'Potion', desc: 'Rend de la vie', color: '#ec4899', icon: <FlaskConical size={16} /> },
  { id: 'key', label: 'Clé', desc: 'Ouvre les portes', color: '#f4af6a', icon: <KeyRound size={16} /> },
  { id: 'chest', label: 'Coffre', desc: 'Récompense à débloquer', color: '#d97706', icon: <Package size={16} /> },
  { id: 'door', label: 'Porte', desc: 'S\'ouvre avec une clé', color: '#38bdf8', icon: <DoorOpen size={16} /> },
  { id: 'platform', label: 'Plateforme', desc: 'Sol solide', color: '#94a3b8', icon: <Square size={16} /> },
  { id: 'spikes', label: 'Pièges', desc: 'Dégâts au contact', color: '#ef4444', icon: <TriangleAlert size={16} /> },
  { id: 'crate', label: 'Caisse', desc: 'Bloc poussable', color: '#b45309', icon: <Box size={16} /> },
  { id: 'plate', label: 'Dalle', desc: 'Déclenche des portes', color: '#34d399', icon: <CircleDot size={16} /> },
  { id: 'boss', label: 'Boss', desc: 'Ennemi final puissant', color: '#f43f5e', icon: <Skull size={16} /> },
  { id: 'wall', label: 'Mur', desc: 'Bloc solide', color: '#64748b', icon: <BrickWall size={16} /> },
];

export const SPAWN_PRESET_IDS = PRESETS.map(p => p.id);

interface SpawnPaletteProps {
  onSpawnAtCenter: (preset: string) => void;
  onSpawn3d: (preset: string) => void;
}

/**
 * Palette d'ajout rapide : clic = placement au centre du viewport,
 * glisser-déposer = placement à la position exacte du curseur.
 */
export const SpawnPalette: React.FC<SpawnPaletteProps> = ({ onSpawnAtCenter, onSpawn3d }) => {
  const [tab, setTab] = useState<'objets' | 'sprites' | 'anims' | 'atlas' | 'mesh'>('objets');
  const [sprites, setSprites] = useState<SpriteAsset[]>([]);
  const [anims, setAnims] = useState<AnimAsset[]>([]);
  const [sheets, setSheets] = useState<SheetAsset[]>([]);
  const [meshes, setMeshes] = useState<MeshAssetSummary[]>([]);
  const [thumbnails, setThumbnails] = useState<Record<string, string>>({});

  const refresh = useCallback(async () => {
    const [s, a, sh, me] = await Promise.all([listSpriteAssets(), listAnimAssets(), listSheetAssets(), listMeshAssets()]);
    setSprites(s);
    setAnims(a);
    setSheets(sh);
    setMeshes(me);
    for (const it of s) {
      getSpritePngUrl(it.id).then(u => setThumbnails(prev => ({ ...prev, [it.id]: u }))).catch(() => {});
    }
    for (const it of a) {
      getSheetPngUrl(it.id).then(u => setThumbnails(prev => ({ ...prev, [it.id]: u }))).catch(() => {});
    }
    for (const it of sh) {
      getSheetPngUrl(it.id).then(u => setThumbnails(prev => ({ ...prev, [it.id]: u }))).catch(() => {});
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const drag = (payload: string) => (e: React.DragEvent) => {
    e.dataTransfer.setData('application/x-karacti-preset', payload);
    e.dataTransfer.effectAllowed = 'copy';
  };

  const TABS: { id: 'objets' | 'sprites' | 'anims' | 'atlas' | 'mesh'; label: string }[] = [
    { id: 'objets', label: 'Objets' },
    { id: 'sprites', label: 'Sprites' },
    { id: 'anims', label: 'Anims' },
    { id: 'atlas', label: 'Atlas' },
    { id: 'mesh', label: 'Mesh 3D' },
  ];

  return (
    <div className="flex flex-col gap-2 p-2.5 bg-[#0d0819] border-b border-[#2d1b4e]">
      <div className="flex items-center justify-between px-0.5">
        <span className="text-[10px] font-bold uppercase tracking-wider text-[#64748b]">
          Ajouter un objet
        </span>
        <span className="text-[10px] text-[#64748b]">
          clic = centre · glisser = libre
        </span>
      </div>

      <div className="flex gap-1">
        {TABS.map(t => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`flex-1 px-1 py-1 rounded-md text-[10px] font-semibold transition border ${
              tab === t.id
                ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7]'
                : 'bg-[#150d27] text-[#94a3b8] border-[#2d1b4e] hover:text-[#f8fafc]'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'objets' && (
        <div className="grid grid-cols-4 gap-1.5">
          {PRESETS.map(p => (
            <button
              key={p.id}
              draggable
              onDragStart={drag(p.id)}
              onClick={() => onSpawnAtCenter(p.id)}
              title={`${p.label} — ${p.desc}`}
              className="flex flex-col items-center gap-1 p-2 bg-[#150d27] hover:bg-[#1e1338] border border-[#2d1b4e] hover:border-[#a855f7] rounded-lg transition group cursor-grab"
            >
              <span style={{ color: p.color }}>{p.icon}</span>
              <span className="text-[10px] font-semibold text-[#cbd5e1] group-hover:text-[#f8fafc]">
                {p.label}
              </span>
            </button>
          ))}
        </div>
      )}

      {tab === 'sprites' && (
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between px-0.5">
            <span className="text-[10px] font-bold uppercase tracking-wider text-[#a855f7]">
              Mes sprites (pxl)
            </span>
            <button
              onClick={() => void refresh()}
              title="Recharger"
              className="text-[#64748b] hover:text-[#f8fafc] transition"
            >
              <RefreshCw size={12} />
            </button>
          </div>
          <div className="grid grid-cols-4 gap-1.5">
            {sprites.map(s => (
              <button
                key={s.id}
                draggable
                onDragStart={drag(`sprite:${s.id}`)}
                onClick={() => onSpawnAtCenter(`sprite:${s.id}`)}
                title={`${s.id} — ${s.w}×${s.h} px`}
                className="flex flex-col items-center gap-1 p-1.5 bg-[#150d27] hover:bg-[#1e1338] border border-[#2d1b4e] hover:border-[#a855f7] rounded-lg transition group cursor-grab"
              >
                {thumbnails[s.id] ? (
                  <img
                    src={thumbnails[s.id]}
                    alt={s.id}
                    className="w-8 h-8 object-contain"
                    style={{ imageRendering: 'pixelated' }}
                  />
                ) : (
                  <span className="w-8 h-8 flex items-center justify-center text-[#64748b] text-[8px]">…</span>
                )}
                <span className="text-[9px] font-semibold text-[#cbd5e1] group-hover:text-[#f8fafc] truncate max-w-full">
                  {s.id}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}

      {tab === 'anims' && (
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between px-0.5">
            <span className="text-[10px] font-bold uppercase tracking-wider text-[#a855f7]">
              Animations (pxl)
            </span>
            <button
              onClick={() => void refresh()}
              title="Recharger"
              className="text-[#64748b] hover:text-[#f8fafc] transition"
            >
              <RefreshCw size={12} />
            </button>
          </div>
          {anims.length === 0 ? (
            <span className="text-[10px] text-[#64748b] px-1">Aucune animation.</span>
          ) : (
            <div className="grid grid-cols-4 gap-1.5">
              {anims.map(a => (
                <button
                  key={a.id}
                  draggable
                  onDragStart={drag(`anim:${a.id}`)}
                  onClick={() => onSpawnAtCenter(`anim:${a.id}`)}
                  title={`${a.id} — ${a.frames} frames`}
                  className="flex flex-col items-center gap-1 p-1.5 bg-[#150d27] hover:bg-[#1e1338] border border-[#2d1b4e] hover:border-[#a855f7] rounded-lg transition group cursor-grab"
                >
                  {thumbnails[a.id] ? (
                    <img
                      src={thumbnails[a.id]}
                      alt={a.id}
                      className="w-8 h-8 object-contain"
                      style={{ imageRendering: 'pixelated' }}
                    />
                  ) : (
                    <span className="w-8 h-8 flex items-center justify-center text-[#64748b] text-[8px]">…</span>
                  )}
                  <span className="text-[9px] font-semibold text-[#cbd5e1] group-hover:text-[#f8fafc] truncate max-w-full">
                    {a.id}
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {tab === 'atlas' && (
        <div className="flex flex-col gap-2">
          {sheets.length === 0 ? (
            <span className="text-[10px] text-[#64748b] px-1">Aucun atlas.</span>
          ) : (
            sheets.map(sh => (
              <div key={sh.id} className="flex flex-col gap-1 bg-[#150d27] border border-[#2d1b4e] rounded-lg p-1.5">
                <div className="flex items-center gap-2">
                  {thumbnails[sh.id] ? (
                    <img
                      src={thumbnails[sh.id]}
                      alt={sh.id}
                      className="w-8 h-8 object-contain"
                      style={{ imageRendering: 'pixelated' }}
                    />
                  ) : (
                    <span className="w-8 h-8 flex items-center justify-center text-[#64748b] text-[8px]">…</span>
                  )}
                  <span className="text-[10px] font-bold text-[#cbd5e1]">{sh.id}</span>
                </div>
                <div className="flex flex-wrap gap-1">
                  {sh.frames.map(f => (
                    <button
                      key={f}
                      draggable
                      onDragStart={drag(`sheet:${sh.id}:${f}`)}
                      onClick={() => onSpawnAtCenter(`sheet:${sh.id}:${f}`)}
                      title={`${sh.id}/${f}`}
                      className="px-1.5 py-0.5 bg-[#1e1338] hover:bg-[#2d1b4e] border border-[#3b0764] hover:border-[#a855f7] rounded text-[9px] text-[#cbd5e1] cursor-grab transition"
                    >
                      {f}
                    </button>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>
      )}

      {tab === 'mesh' && (
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between px-0.5">
            <span className="text-[10px] font-bold uppercase tracking-wider text-[#38bdf8]">Meshes 3D</span>
            <button onClick={() => void refresh()} title="Recharger" className="text-[#64748b] hover:text-[#f8fafc] transition"><RefreshCw size={12} /></button>
          </div>
          {meshes.length === 0 ? (
            <span className="text-[10px] text-[#64748b] px-1">Aucun mesh.</span>
          ) : (
            <div className="grid grid-cols-3 gap-1.5">
              {meshes.map(m => (
                <button
                  key={m.id}
                  onClick={() => onSpawn3d(m.id)}
                  title={`${m.id} — ${m.parts} partie(s)`}
                  className="flex flex-col items-center gap-1 p-1.5 bg-[#150d27] hover:bg-[#1e1338] border border-[#2d1b4e] hover:border-[#38bdf8] rounded-lg transition group"
                >
                  <Boxes size={16} className="text-[#38bdf8]" />
                  <span className="text-[9px] font-semibold text-[#cbd5e1] group-hover:text-[#f8fafc] truncate max-w-full">{m.id}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
