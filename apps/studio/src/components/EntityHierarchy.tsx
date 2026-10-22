import { useState } from 'react';
import {
  User, Skull, Package, Flame, Zap, Layers, Search, Coins, DoorOpen, Box, CircleDot, BrickWall, Sparkles,
} from 'lucide-react';
import type { StudioEntitySummary } from '../services/engineBridge';

interface EntityHierarchyProps {
  entities: StudioEntitySummary[];
  selectedEntityId: number | null;
  onSelectEntity: (id: number) => void;
}

const TAG_LABELS: Record<string, string> = {
  player: 'Joueur',
  npc: 'PNJ',
  enemy: 'Ennemi',
  boss: 'Boss',
  collectible: 'Pièce',
  item: 'Objet',
  door: 'Porte',
  hazard: 'Piège',
  crate: 'Caisse',
  plate: 'Dalle',
  platform: 'Sol',
  wall: 'Mur',
  portal: 'Portail',
  torch: 'Torche',
};

const getEntityIcon = (tag: string) => {
  switch (tag) {
    case 'player': return <User size={13} className="text-[#38bdf8]" />;
    case 'npc': return <Sparkles size={13} className="text-[#34d399]" />;
    case 'enemy': return <Skull size={13} className="text-[#f59e0b]" />;
    case 'boss': return <Skull size={13} className="text-[#ef4444]" />;
    case 'collectible': return <Coins size={13} className="text-[#f59e0b]" />;
    case 'item': return <Package size={13} className="text-[#34d399]" />;
    case 'door': return <DoorOpen size={13} className="text-[#38bdf8]" />;
    case 'hazard': return <Flame size={13} className="text-[#f43f5e]" />;
    case 'crate': return <Box size={13} className="text-[#f4af6a]" />;
    case 'plate': return <CircleDot size={13} className="text-[#34d399]" />;
    case 'platform': return <Layers size={13} className="text-[#94a3b8]" />;
    case 'wall': return <BrickWall size={13} className="text-[#64748b]" />;
    case 'portal': return <Zap size={13} className="text-[#a855f7]" />;
    default: return <Layers size={13} className="text-[#94a3b8]" />;
  }
};

/**
 * Panneau de hiérarchie connecté au moteur natif : la liste est l'état réel.
 */
export const EntityHierarchy: React.FC<EntityHierarchyProps> = ({
  entities,
  selectedEntityId,
  onSelectEntity,
}) => {
  const [filterQuery, setFilterQuery] = useState('');

  const filtered = entities.filter(e => {
    const q = filterQuery.toLowerCase();
    if (!q) return true;
    const tagLabel = TAG_LABELS[e.tag] ?? e.tag;
    return (
      e.name.toLowerCase().includes(q) ||
      e.tag.toLowerCase().includes(q) ||
      tagLabel.toLowerCase().includes(q)
    );
  });

  const counts = entities.reduce<Record<string, number>>((acc, e) => {
    acc[e.tag] = (acc[e.tag] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <aside className="w-64 flex flex-col h-full bg-[#0d0819] border-r border-[#2d1b4e] select-none">
      {/* En-tête */}
      <div className="flex items-center justify-between px-3 py-2.5 bg-[#150d27] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-1.5">
          <Layers size={14} className="text-[#a855f7]" />
          <span className="font-bold text-xs text-[#f8fafc] tracking-wide uppercase">
            Hiérarchie ({entities.length})
          </span>
        </div>
      </div>

      {/* Barre de Recherche */}
      <div className="p-2 border-b border-[#2d1b4e]">
        <div className="flex items-center gap-1.5 px-2 py-1 bg-[#07040d] border border-[#2d1b4e] rounded">
          <Search size={12} className="text-[#64748b]" />
          <input
            type="text"
            value={filterQuery}
            onChange={e => setFilterQuery(e.target.value)}
            placeholder="Filtrer les entités..."
            className="w-full text-xs bg-transparent border-none p-0 focus:ring-0 text-[#f8fafc] placeholder-[#64748b]"
          />
        </div>
      </div>

      {/* Liste des entités */}
      <div className="flex-1 overflow-y-auto p-1.5 space-y-0.5">
        {filtered.map(entity => {
          const isSelected = selectedEntityId === entity.id;
          return (
            <div
              key={entity.id}
              onClick={() => onSelectEntity(entity.id)}
              className={`flex items-center justify-between px-2.5 py-1.5 rounded cursor-pointer transition text-xs ${
                isSelected
                  ? 'bg-[#3b0764] text-[#f3e8ff] font-bold border border-[#a855f7]'
                  : 'hover:bg-[#150d27] text-[#94a3b8] hover:text-[#f8fafc] border border-transparent'
              }`}
            >
              <div className="flex items-center gap-2 truncate">
                {getEntityIcon(entity.tag)}
                <span className="truncate">{entity.name}</span>
              </div>
              <span className="text-[10px] text-[#64748b] font-mono shrink-0 uppercase">
                {TAG_LABELS[entity.tag] ?? entity.tag}
              </span>
            </div>
          );
        })}

        {filtered.length === 0 && (
          <div className="p-4 text-center text-xs text-[#64748b] leading-relaxed">
            {entities.length === 0
              ? 'Scène vide — ajoutez un objet avec la palette ci-dessus.'
              : 'Aucune entité ne correspond au filtre.'}
          </div>
        )}
      </div>

      {/* Résumé par type */}
      {Object.keys(counts).length > 0 && (
        <div className="px-3 py-2 border-t border-[#2d1b4e] text-[10px] text-[#64748b] flex flex-wrap gap-1.5">
          {Object.entries(counts).map(([tag, n]) => (
            <span key={tag} className="badge-tag bg-[#150d27] text-[#94a3b8]">
              {TAG_LABELS[tag] ?? tag} · {n}
            </span>
          ))}
        </div>
      )}
    </aside>
  );
};

