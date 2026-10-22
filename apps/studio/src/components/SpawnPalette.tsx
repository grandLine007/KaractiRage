import { useCallback, useEffect, useState } from 'react';
import { Coins, FlaskConical, KeyRound, Package, DoorOpen, Square, TriangleAlert, Box, CircleDot, Skull, BrickWall, RefreshCw } from 'lucide-react';
import { listSpriteAssets, getSpritePngUrl, type SpriteAsset } from '../services/engineBridge';

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
}

/**
 * Palette d'ajout rapide : clic = placement au centre du viewport,
 * glisser-déposer = placement à la position exacte du curseur.
 */
export const SpawnPalette: React.FC<SpawnPaletteProps> = ({ onSpawnAtCenter }) => {
  const [sprites, setSprites] = useState<SpriteAsset[]>([]);
  const [thumbnails, setThumbnails] = useState<Record<string, string>>({});

  const refreshSprites = useCallback(async () => {
    const list = await listSpriteAssets();
    setSprites(list);
    // Précharge les miniatures sans bloquer l'affichage de la grille.
    for (const s of list) {
      getSpritePngUrl(s.id)
        .then(url => setThumbnails(prev => ({ ...prev, [s.id]: url })))
        .catch(() => {});
    }
  }, []);

  useEffect(() => {
    void refreshSprites();
  }, [refreshSprites]);

  return (
    <div className="flex flex-col gap-2 p-2.5 bg-[#0d0819] border-b border-[#2d1b4e]">
      <div className="flex items-center justify-between px-0.5">
        <span className="text-[10px] font-bold uppercase tracking-wider text-[#64748b]">
          Ajouter un objet
        </span>
        <span className="text-[10px] text-[#64748b]">
          clic = au centre · glisser = position libre
        </span>
      </div>

      {sprites.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between px-0.5">
            <span className="text-[10px] font-bold uppercase tracking-wider text-[#a855f7]">
              Mes sprites (pxl)
            </span>
            <button
              onClick={() => void refreshSprites()}
              title="Recharger les sprites"
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
                onDragStart={e => {
                  e.dataTransfer.setData('application/x-karacti-preset', `sprite:${s.id}`);
                  e.dataTransfer.effectAllowed = 'copy';
                }}
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

      <div className="grid grid-cols-4 gap-1.5">
        {PRESETS.map(p => (
          <button
            key={p.id}
            draggable
            onDragStart={e => {
              e.dataTransfer.setData('application/x-karacti-preset', p.id);
              e.dataTransfer.effectAllowed = 'copy';
            }}
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
    </div>
  );
};
