import { useEffect, useState } from 'react';
import { Sliders, Move, Sparkles, Trash2, Copy } from 'lucide-react';
import type { StudioEntitySummary } from '../services/engineBridge';
import { AiSparkle } from './AiSparkle';

interface EntityInspectorProps {
  selectedEntity: StudioEntitySummary | null;
  onUpdateTransform: (id: number, x: number, y: number, w: number, h: number) => void;
  /** Voie utilisateur de la transformée 3D — même fonction moteur que l'op IA SetTransform3D. */
  onUpdateTransform3d?: (
    id: number,
    position: [number, number, number],
    rotation_euler_deg: [number, number, number],
    scale: [number, number, number],
  ) => void;
  /** Voie utilisateur du matériau 3D — même état que l'opération IA SetMaterial. */
  onUpdateMaterial3d?: (id: number, color: number, roughness: number, metalness: number) => void;
  onDuplicate: (id: number) => void;
  onDelete: (id: number) => void;
  /** Ouvre la barre d'édition IA avec le champ d'origine en indice. */
  onAiEdit: (field: string) => void;
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
  onUpdateTransform3d,
  onUpdateMaterial3d,
  onDuplicate,
  onDelete,
  onAiEdit,
}) => {
  const [pos3d, setPos3d] = useState<[number, number, number]>([0, 0, 0]);
  const [rot3d, setRot3d] = useState<[number, number, number]>([0, 0, 0]);
  const [scale3d, setScale3d] = useState<[number, number, number]>([1, 1, 1]);
  const [matColor, setMatColor] = useState('#ffffff');
  const [matRough, setMatRough] = useState(0.4);
  const [matMetal, setMatMetal] = useState(0.0);
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
    const mat = selectedEntity?.material;
    if (mat) {
      setMatColor(`#${(mat.color & 0xffffff).toString(16).padStart(6, '0')}`);
      setMatRough(mat.roughness);
      setMatMetal(mat.metalness);
    }
    const t3 = selectedEntity?.transform3d;
    if (t3) {
      setPos3d([...t3.position] as [number, number, number]);
      setRot3d([...t3.rotation_euler_deg] as [number, number, number]);
      setScale3d([...t3.scale] as [number, number, number]);
    }
  }, [selectedEntity?.id]);

  if (!selectedEntity) {
    return (
      <aside data-testid="inspector" className="w-72 flex flex-col items-center justify-center h-full bg-[#0d0819] border-l border-[#2d1b4e] p-6 text-center text-xs text-[#94a3b8] select-none">
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
    <aside data-testid="inspector" className="w-72 flex flex-col h-full bg-[#0d0819] border-l border-[#2d1b4e] overflow-y-auto select-none">
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
            className="flex items-center justify-center w-8 h-8 hover:bg-[#1e1338] text-[#c084fc] rounded transition"
            title="Dupliquer l'entité [Ctrl+D]"
          >
            <Copy size={13} />
          </button>
          <button
            onClick={() => onDelete(selectedEntity.id)}
            className="flex items-center justify-center w-8 h-8 hover:bg-[#881337] text-[#f43f5e] rounded transition"
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
          <span className="text-[11px] text-[#94a3b8] font-mono">ID #{selectedEntity.id}</span>
        </div>

        {/* Transform (éditable, commit au blur / Entrée) */}
        <div className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
          <div className="flex items-center justify-between font-bold text-[11px] text-[#c084fc] uppercase tracking-wider">
            <span className="flex items-center gap-1.5">
              <Move size={12} />
              Transform
            </span>
            <span className="text-[9px] text-[#94a3b8] font-normal normal-case">Entrée pour valider</span>
          </div>
          {selectedEntity.transform3d ? (
            <div data-testid="transform-3d" className="grid grid-cols-3 gap-2">
              {(['X', 'Y', 'Z'] as const).map((axe, i) => (
                <label key={`p${axe}`} className="text-[10px] text-[#94a3b8]">
                  {axe}
                  <input
                    type="number"
                    step="0.1"
                    value={Number(pos3d[i].toFixed(2))}
                    onChange={e => {
                      const v = [...pos3d] as [number, number, number];
                      v[i] = parseFloat(e.target.value) || 0;
                      setPos3d(v);
                    }}
                    onBlur={() => onUpdateTransform3d?.(selectedEntity.id, pos3d, rot3d, scale3d)}
                    onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }}
                    className="mt-0.5 w-full text-xs font-mono py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded focus:border-[#a855f7]"
                  />
                </label>
              ))}
              {(['RX', 'RY', 'RZ'] as const).map((axe, i) => (
                <label key={`r${axe}`} className="text-[10px] text-[#94a3b8]">
                  {axe}°
                  <input
                    type="number"
                    step="1"
                    value={Number(rot3d[i].toFixed(1))}
                    onChange={e => {
                      const v = [...rot3d] as [number, number, number];
                      v[i] = parseFloat(e.target.value) || 0;
                      setRot3d(v);
                    }}
                    onBlur={() => onUpdateTransform3d?.(selectedEntity.id, pos3d, rot3d, scale3d)}
                    onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }}
                    className="mt-0.5 w-full text-xs font-mono py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded focus:border-[#a855f7]"
                  />
                </label>
              ))}
              {(['SX', 'SY', 'SZ'] as const).map((axe, i) => (
                <label key={`s${axe}`} className="text-[10px] text-[#94a3b8]">
                  {axe}
                  <input
                    type="number"
                    step="0.05"
                    min="0.01"
                    value={Number(scale3d[i].toFixed(2))}
                    onChange={e => {
                      const v = [...scale3d] as [number, number, number];
                      v[i] = parseFloat(e.target.value) || 1;
                      setScale3d(v);
                    }}
                    onBlur={() => onUpdateTransform3d?.(selectedEntity.id, pos3d, rot3d, scale3d)}
                    onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }}
                    className="mt-0.5 w-full text-xs font-mono py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded focus:border-[#a855f7]"
                  />
                </label>
              ))}
            </div>
          ) : (
          <div className="grid grid-cols-2 gap-2">
            {[
              { id: 'x', label: 'X', value: x, set: setX, aiLabel: 'la position X' },
              { id: 'y', label: 'Y', value: y, set: setY, aiLabel: 'la position Y' },
              { id: 'w', label: 'Larg.', value: w, set: setW, aiLabel: 'la largeur' },
              { id: 'h', label: 'Haut.', value: h, set: setH, aiLabel: 'la hauteur' },
            ].map(field => (
              <div key={field.id}>
                <div className="flex items-center justify-between mb-0.5">
                  <label htmlFor={`transform-${field.id}`} className="text-[10px] text-[#94a3b8]">
                    {field.label} :
                  </label>
                  <AiSparkle
                    label={field.aiLabel}
                    testId={`ai-sparkle-${field.id}`}
                    onClick={() => onAiEdit(field.id)}
                  />
                </div>
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
          )}
        </div>

        {selectedEntity.material && (
          <div data-testid="material-3d" className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
            <div className="flex items-center justify-between font-bold text-[11px] text-[#c084fc] uppercase tracking-wider">
              <span>Matériau 3D</span>
            </div>
            <div className="grid grid-cols-3 gap-2">
              <label className="text-[10px] text-[#94a3b8]">
                Couleur
                <input
                  type="color"
                  value={matColor}
                  onChange={e => setMatColor(e.target.value)}
                  onBlur={() => onUpdateMaterial3d?.(selectedEntity.id, parseInt(matColor.slice(1), 16), matRough, matMetal)}
                  className="mt-0.5 w-full h-7 bg-[#07040d] border border-[#2d1b4e] rounded"
                />
              </label>
              <label className="text-[10px] text-[#94a3b8]">
                Rugosité
                <input
                  type="number" step="0.05" min="0" max="1" value={matRough}
                  onChange={e => setMatRough(parseFloat(e.target.value) || 0)}
                  onBlur={() => onUpdateMaterial3d?.(selectedEntity.id, parseInt(matColor.slice(1), 16), matRough, matMetal)}
                  className="mt-0.5 w-full text-xs font-mono py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded"
                />
              </label>
              <label className="text-[10px] text-[#94a3b8]">
                Métal
                <input
                  type="number" step="0.05" min="0" max="1" value={matMetal}
                  onChange={e => setMatMetal(parseFloat(e.target.value) || 0)}
                  onBlur={() => onUpdateMaterial3d?.(selectedEntity.id, parseInt(matColor.slice(1), 16), matRough, matMetal)}
                  className="mt-0.5 w-full text-xs font-mono py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded"
                />
              </label>
            </div>
          </div>
        )}

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
              <span className="flex items-center gap-1 min-w-0">
                <span className="font-semibold text-[#f8fafc] truncate max-w-[45%]">{selectedEntity.name}</span>
                <AiSparkle
                  label="le nom"
                  testId="ai-sparkle-name"
                  onClick={() => onAiEdit('name')}
                />
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-[#94a3b8]">Type</span>
              <span className="badge-tag" style={{ backgroundColor: `${tagColor}1a`, color: tagColor }}>{selectedEntity.tag}</span>
            </div>

            {selectedEntity.is_player_controlled && (
              <div className="flex items-center justify-between">
                <span className="text-[#94a3b8]">Rôle</span>
                <span className="badge-tag bg-[#0c4a6e] text-[#7dd3fc]">Joueur contrôlé</span>
              </div>
            )}

            <div className="flex items-center justify-between">
              <span className="text-[#94a3b8]">Collision</span>
              <span className={`badge-tag ${selectedEntity.is_solid ? 'bg-[#064e3b] text-[#34d399]' : selectedEntity.is_trigger ? 'bg-[#2e1065] text-[#c084fc]' : 'bg-[#150d27] text-[#94a3b8]'}`}>
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
                <span className="flex items-center gap-1">
                  <span className="badge-tag bg-[#150d27] text-[#94a3b8]">
                    {selectedEntity.movement_mode ?? 'Actif'}
                    {selectedEntity.speed !== undefined && ` · ${Math.round(selectedEntity.speed)} u/s`}
                  </span>
                  <AiSparkle
                    label="la vitesse"
                    testId="ai-sparkle-speed"
                    onClick={() => onAiEdit('speed')}
                  />
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
