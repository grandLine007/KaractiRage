import { useState } from 'react';
import { User, Skull, Package, Flame, Zap, Layers, Plus, Search } from 'lucide-react';

export interface EntityItem {
  id: string;
  name: string;
  kind: 'player' | 'enemy' | 'boss' | 'item' | 'hazard' | 'portal' | 'platform';
  x: number;
  y: number;
  health?: number;
  spriteArchetype: string;
}

interface EntityHierarchyProps {
  entities: EntityItem[];
  selectedEntityId: string | null;
  onSelectEntity: (id: string) => void;
  onAddEntity?: () => void;
}

export const EntityHierarchy: React.FC<EntityHierarchyProps> = ({
  entities,
  selectedEntityId,
  onSelectEntity,
  onAddEntity,
}) => {
  const [filterQuery, setFilterQuery] = useState('');

  const filtered = entities.filter(e =>
    e.name.toLowerCase().includes(filterQuery.toLowerCase()) ||
    e.kind.toLowerCase().includes(filterQuery.toLowerCase())
  );

  const getEntityIcon = (kind: EntityItem['kind']) => {
    switch (kind) {
      case 'player': return <User size={13} className="text-[#38bdf8]" />;
      case 'enemy': return <Skull size={13} className="text-[#f59e0b]" />;
      case 'boss': return <Skull size={13} className="text-[#ef4444]" />;
      case 'item': return <Package size={13} className="text-[#34d399]" />;
      case 'hazard': return <Flame size={13} className="text-[#f43f5e]" />;
      case 'portal': return <Zap size={13} className="text-[#a855f7]" />;
      case 'platform': return <Layers size={13} className="text-[#94a3b8]" />;
    }
  };

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
        {onAddEntity && (
          <button
            onClick={onAddEntity}
            className="p-1 hover:bg-[#1e1338] text-[#c084fc] rounded transition"
            title="Ajouter une entité"
          >
            <Plus size={15} />
          </button>
        )}
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
                  : 'hover:bg-[#150d27] text-[#94a3b8] hover:text-[#f8fafc]'
              }`}
            >
              <div className="flex items-center gap-2 truncate">
                {getEntityIcon(entity.kind)}
                <span className="truncate">{entity.name}</span>
              </div>
              <span className="text-[10px] text-[#64748b] font-mono shrink-0 uppercase">
                {entity.kind}
              </span>
            </div>
          );
        })}

        {filtered.length === 0 && (
          <div className="p-4 text-center text-xs text-[#64748b]">
            Aucune entité trouvée
          </div>
        )}
      </div>
    </aside>
  );
};
