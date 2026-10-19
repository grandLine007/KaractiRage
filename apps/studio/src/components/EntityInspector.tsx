import { Sliders, Heart, Move, Eye, Sparkles, Trash2 } from 'lucide-react';
import type { EntityItem } from './EntityHierarchy';

interface EntityInspectorProps {
  selectedEntity: EntityItem | null;
  onUpdateEntity?: (updated: EntityItem) => void;
  onDeleteEntity?: (id: string) => void;
}

export const EntityInspector: React.FC<EntityInspectorProps> = ({
  selectedEntity,
  onUpdateEntity,
  onDeleteEntity,
}) => {
  if (!selectedEntity) {
    return (
      <aside className="w-72 flex flex-col items-center justify-center h-full bg-[#0d0819] border-l border-[#2d1b4e] p-6 text-center text-xs text-[#64748b] select-none">
        <Sliders size={28} className="text-[#2d1b4e] mb-2" />
        <span className="font-semibold text-[#94a3b8] mb-1">Aucune Entité Sélectionnée</span>
        <p className="text-[11px] leading-relaxed">
          Cliquez sur un sprite dans le Viewport ou dans la Hiérarchie pour inspecter et modifier ses composants.
        </p>
      </aside>
    );
  }

  const handleChange = (field: keyof EntityItem, value: any) => {
    if (onUpdateEntity) {
      onUpdateEntity({
        ...selectedEntity,
        [field]: value
      });
    }
  };

  return (
    <aside className="w-72 flex flex-col h-full bg-[#0d0819] border-l border-[#2d1b4e] overflow-y-auto select-none">
      {/* En-tête de l'Inspecteur */}
      <div className="flex items-center justify-between px-3 py-2.5 bg-[#150d27] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2 truncate">
          <Sliders size={14} className="text-[#a855f7]" />
          <span className="font-bold text-xs text-[#f8fafc] truncate">
            {selectedEntity.name}
          </span>
        </div>
        {onDeleteEntity && (
          <button
            onClick={() => onDeleteEntity(selectedEntity.id)}
            className="p-1 hover:bg-[#881337] text-[#f43f5e] rounded transition"
            title="Supprimer l'entité"
          >
            <Trash2 size={13} />
          </button>
        )}
      </div>

      <div className="p-3 space-y-4 text-xs">
        {/* 1. Composant Transform */}
        <div className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
          <div className="flex items-center justify-between font-bold text-[11px] text-[#c084fc] uppercase tracking-wider">
            <span className="flex items-center gap-1.5">
              <Move size={12} />
              Transform (Position)
            </span>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="text-[10px] text-[#94a3b8] block mb-0.5">Position X :</label>
              <input
                type="number"
                value={Math.round(selectedEntity.x)}
                onChange={e => handleChange('x', parseFloat(e.target.value) || 0)}
                className="w-full text-xs font-mono py-1 px-1.5"
              />
            </div>
            <div>
              <label className="text-[10px] text-[#94a3b8] block mb-0.5">Position Y :</label>
              <input
                type="number"
                value={Math.round(selectedEntity.y)}
                onChange={e => handleChange('y', parseFloat(e.target.value) || 0)}
                className="w-full text-xs font-mono py-1 px-1.5"
              />
            </div>
          </div>
        </div>

        {/* 2. Composant Sprite & Visuel */}
        <div className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
          <div className="flex items-center justify-between font-bold text-[11px] text-[#38bdf8] uppercase tracking-wider">
            <span className="flex items-center gap-1.5">
              <Sparkles size={12} />
              Sprite & Visuel
            </span>
          </div>
          <div>
            <label className="text-[10px] text-[#94a3b8] block mb-0.5">Archétype Sprite :</label>
            <select
              value={selectedEntity.spriteArchetype}
              onChange={e => handleChange('spriteArchetype', e.target.value)}
              className="w-full text-xs py-1 px-1.5 bg-[#07040d]"
            >
              <option value="ninja">Héros Ninja</option>
              <option value="knight">Chevalier Errant</option>
              <option value="boss">Boss Démon</option>
              <option value="slime">Slime Toxique</option>
              <option value="drone">Drone A*</option>
              <option value="chest">Coffre au Trésor</option>
              <option value="key">Clé Dorée</option>
              <option value="potion">Potion de Vie</option>
              <option value="portal">Portail Quantique</option>
              <option value="spikes">Piège de Pointes</option>
            </select>
          </div>
        </div>

        {/* 3. Composant Santé / PV */}
        <div className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
          <div className="flex items-center justify-between font-bold text-[11px] text-[#f43f5e] uppercase tracking-wider">
            <span className="flex items-center gap-1.5">
              <Heart size={12} />
              Santé & Combat
            </span>
          </div>
          <div>
            <label className="text-[10px] text-[#94a3b8] block mb-0.5">Points de Vie (PV) :</label>
            <input
              type="number"
              value={selectedEntity.health ?? 5}
              onChange={e => handleChange('health', parseFloat(e.target.value) || 1)}
              className="w-full text-xs font-mono py-1 px-1.5"
            />
          </div>
        </div>

        {/* 4. IA & Perception conique */}
        <div className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
          <div className="flex items-center justify-between font-bold text-[11px] text-[#f59e0b] uppercase tracking-wider">
            <span className="flex items-center gap-1.5">
              <Eye size={12} />
              Perception & Navigation
            </span>
          </div>
          <div className="flex items-center justify-between text-[11px] text-[#cbd5e1] py-1">
            <span>Vision Conique A*</span>
            <span className="badge-tag bg-[#064e3b] text-[#34d399]">Actif (180 px)</span>
          </div>
          <div className="flex items-center justify-between text-[11px] text-[#cbd5e1] py-1">
            <span>Évitement d'obstacles</span>
            <span className="badge-tag bg-[#2e1065] text-[#c084fc]">Déterministe</span>
          </div>
        </div>
      </div>
    </aside>
  );
};
