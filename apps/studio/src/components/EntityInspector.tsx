import { useEffect, useState } from 'react';
import { Sliders, Move, Sparkles, Trash2, Copy } from 'lucide-react';
import type { StudioEntitySummary } from '../services/engineBridge';

interface EntityInspectorProps {
  selectedEntity: StudioEntitySummary | null;
  onUpdateTransform: (id: number, x: number, y: number, w: number, h: number) => void;
  onDuplicate: (id: number) => void;
  onDelete: (id: number) => void;
}

const TAG_COLORS: Record<string, string> = {
  player: '#38bdf8',
  npc: '#34d399',
  enemy: '#f59e0b',
  boss: '#ef4444',
  collectible: '#f59e0b',
  item: '#34d399',
  door: '#38bdf8',
  hazard: '#f43f5e',
  crate: '#f4af6a',
  plate: '#34d399',
  platform: '#94a3b8',
  wall: '#64748b',
  portal: '#a855f7',
};

/**
 * Inspecteur ECS connecté : lit l'état natif, écrit la transformation réelle
 * (commit au blur / Entrée pour un seul point d'annulation par édition).
 */
export const EntityInspector: React.FC<EntityInspectorProps> = ({
  selectedEntity,
  onUpdateTransform,
  onDuplicate,
  onDelete,
}) => {
  const [x, setX] = useState(0);
  const [y, setY] = useState(0);
  const [w, setW] = useState(0);
  const [h, setH] = useState(0);

  useEffect(() => {
    if (selectedEntity) {
      setX(selectedEntity.x);
      setY(selectedEntity.y);
      setW(selectedEntity.w);
      setH(selectedEntity.h);
    }
  }, [selectedEntity?.id]);

  if (!selectedEntity) {
    return (
      <aside className="w-72 flex flex-col items-center justify-center h-full bg-[#0d0819] border-l border-[#2d1b4e] p-6 text-center text-xs text-[#64748b] select-none">
        <Sliders size={28} className="text-[#2d1b4e] mb-2" />
        <span className="font-semibold text-[#94a3b8] mb-1">Aucune Entité Sélectionnée</span>
        <p className="text-[11px] leading-relaxed">
          Cliquez sur un objet dans la scène ou dans la Hiérarchie pour l'inspecter et le modifier.
        </p>
      </aside>
    );
  }

  const commit = () => {
    onUpdateTransform(selectedEntity.id, x, y, w, h);
  };

  const tagColor = TAG_COLORS[selectedEntity.tag] ?? '#94a3b8';

  return (
    <aside className="w-72 flex flex-col h-full bg-[#0d0819] border-l border-[#2d1b4e] overflow-y-auto select-none">
      <div className="flex items-center justify-between px-3 py-2.5 bg-[#150d27] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2 truncate">
          <Sliders size={14} className="text-[#a855f7]" />
          <span className="font-bold text-xs text-[#f8fafc] truncate">
            {selectedEntity.name}
          </span>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <button
            onClick={() => onDuplicate(selectedEntity.id)}
            className="p-1 hover:bg-[#1e1338] text-[#c084fc] rounded transition"
            title="Dupliquer l'entité [Ctrl+D]"
          >
            <Copy size={13} />
          </button>
          <button
            onClick={() => onDelete(selectedEntity.id)}
            className="p-1 hover:bg-[#881337] text-[#f43f5e] rounded transition"
            title="Supprimer l'entité [Suppr]"
          >
            <Trash2 size={13} />
          </button>
        </div>
      </div>

      <div className="p-3 space-y-4 text-xs">
        <div className="flex items-center justify-between px-1">
          <span className="badge-tag" style={{ backgroundColor: `${tagColor}1a`, color: tagColor }}>
            {selectedEntity.tag}
          </span>
          <span className="text-[11px] text-[#64748b] font-mono">ID #{selectedEntity.id}</span>
        </div>

        {/* Transform (éditable, commit au blur / Entrée) */}
        <div className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
          <div className="flex items-center justify-between font-bold text-[11px] text-[#c084fc] uppercase tracking-wider">
            <span className="flex items-center gap-1.5">
              <Move size={12} />
              Transform
            </span>
            <span className="text-[9px] text-[#64748b] font-normal normal-case">Entrée pour valider</span>
          </div>
          <div className="grid grid-cols-2 gap-2">
            {[
              { id: 'x', label: 'X', value: x, set: setX },
              { id: 'y', label: 'Y', value: y, set: setY },
              { id: 'w', label: 'Larg.', value: w, set: setW },
              { id: 'h', label: 'Haut.', value: h, set: setH },
            ].map(field => (
              <div key={field.id}>
                <label htmlFor={`transform-${field.id}`} className="text-[10px] text-[#94a3b8] block mb-0.5">{field.label} :</label>
                <input
                  id={`transform-${field.id}`}
                  type="number"
                  value={Math.round(field.value)}
                  onChange={e => field.set(parseFloat(e.target.value) || 0)}
                  onBlur={commit}
                  onKeyDown={e => {
                    if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                  }}
                  className="w-full text-xs font-mono py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded focus:border-[#a855f7]"
                />
              </div>
            ))}
          </div>
        </div>

        {/* Propriétés (lecture seule) */}
        <div className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
          <div className="flex items-center justify-between font-bold text-[11px] text-[#38bdf8] uppercase tracking-wider">
            <span className="flex items-center gap-1.5">
              <Sparkles size={12} />
              Propriétés
            </span>
          </div>

          <div className="space-y-1.5 text-[11px] text-[#cbd5e1]">
            <div className="flex items-center justify-between">
              <span className="text-[#94a3b8]">Nom</span>
              <span className="font-semibold text-[#f8fafc] truncate max-w-[55%]">{selectedEntity.name}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-[#94a3b8]">Type</span>
              <span className="badge-tag" style={{ backgroundColor: `${tagColor}1a`, color: tagColor }}>{selectedEntity.tag}</span>
            </div>

            {selectedEntity.is_player_controlled && (
              <div className="flex items-center justify-between">
                <span className="text-[#94a3b8]">Rôle</span>
                <span className="badge-tag bg-[#0c4a6e] text-[#38bdf8]">Joueur contrôlé</span>
              </div>
            )}

            <div className="flex items-center justify-between">
              <span className="text-[#94a3b8]">Collision</span>
              <span className={`badge-tag ${selectedEntity.is_solid ? 'bg-[#064e3b] text-[#34d399]' : selectedEntity.is_trigger ? 'bg-[#2e1065] text-[#c084fc]' : 'bg-[#150d27] text-[#64748b]'}`}>
                {selectedEntity.is_solid ? 'Solide' : selectedEntity.is_trigger ? 'Déclencheur' : 'Aucune'}
              </span>
            </div>

            {selectedEntity.is_pushable && (
              <div className="flex items-center justify-between">
                <span className="text-[#94a3b8]">Poussable</span>
                <span className="badge-tag bg-[#7c2d12] text-[#f4af6a]">Oui</span>
              </div>
            )}

            {selectedEntity.health !== undefined && (
              <div className="flex items-center justify-between">
                <span className="text-[#94a3b8]">Points de vie</span>
                <span className="font-mono font-bold text-[#f8fafc]">
                  {selectedEntity.health}
                  {selectedEntity.max_health !== undefined && ` / ${selectedEntity.max_health}`}
                </span>
              </div>
            )}

            {selectedEntity.has_movement && (
              <div className="flex items-center justify-between">
                <span className="text-[#94a3b8]">Déplacement</span>
                <span className="badge-tag bg-[#150d27] text-[#94a3b8]">
                  {selectedEntity.movement_mode ?? 'Actif'}
                  {selectedEntity.speed !== undefined && ` · ${Math.round(selectedEntity.speed)} u/s`}
                </span>
              </div>
            )}

            {selectedEntity.has_vision && (
              <div className="flex items-center justify-between">
                <span className="text-[#94a3b8]">Vision</span>
                <span className="font-mono">{selectedEntity.vision_range ?? 0} px</span>
              </div>
            )}

            {selectedEntity.script_path && (
              <div className="flex items-center justify-between">
                <span className="text-[#94a3b8]">Script KVL</span>
                <span className="badge-tag bg-[#064e3b] text-[#34d399] truncate max-w-[55%]">{selectedEntity.script_path}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </aside>
  );
};
