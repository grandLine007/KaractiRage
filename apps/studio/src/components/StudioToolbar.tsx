import { MousePointer2, Paintbrush, Eraser, Undo2, Redo2, Save } from 'lucide-react';
import type { StudioEditorState } from '../services/engineBridge';

interface StudioToolbarProps {
  tool: StudioEditorState['tool'];
  tile: string;
  canUndo: boolean;
  canRedo: boolean;
  onSelectTool: (tool: StudioEditorState['tool']) => void;
  onSelectTile: (tileId: string) => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
}

const TILES = [
  { id: 'wall', label: 'Mur', color: '#94a3b8' },
  { id: 'hazard', label: 'Piège', color: '#ef4444' },
  { id: 'collectible', label: 'Pièce', color: '#f59e0b' },
  { id: 'heart', label: 'Cœur', color: '#ec4899' },
  { id: 'crate', label: 'Caisse', color: '#f4af6a' },
  { id: 'plate', label: 'Dalle', color: '#34d399' },
];

/**
 * Barre d'outils de l'Atelier : sélection, pinceau, gomme, tuiles et historique.
 * Reflète fidèlement les raccourcis natifs V / B / X / T et Ctrl+Z / Ctrl+Y.
 */
export const StudioToolbar: React.FC<StudioToolbarProps> = ({
  tool,
  tile,
  canUndo,
  canRedo,
  onSelectTool,
  onSelectTile,
  onUndo,
  onRedo,
  onSave,
}) => {
  return (
    <div className="flex items-center gap-3 px-3 py-2 bg-[#0d0819] border-b border-[#2d1b4e] select-none overflow-x-auto">
      {/* Outils */}
      <div className="flex items-center gap-1">
        <button
          onClick={() => onSelectTool('select')}
          title="Outil Sélection / Inspection [V]"
          className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-semibold transition border ${
            tool === 'select'
              ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7]'
              : 'text-[#94a3b8] border-transparent hover:bg-[#1e1338] hover:text-[#f8fafc]'
          }`}
        >
          <MousePointer2 size={14} />
          <span className="hidden sm:inline">Sélection</span>
          <kbd className="ml-0.5 text-[9px] font-mono text-[#94a3b8]">V</kbd>
        </button>

        <button
          onClick={() => onSelectTool('brush')}
          title="Outil Pinceau [B]"
          className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-semibold transition border ${
            tool === 'brush'
              ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7]'
              : 'text-[#94a3b8] border-transparent hover:bg-[#1e1338] hover:text-[#f8fafc]'
          }`}
        >
          <Paintbrush size={14} />
          <span className="hidden sm:inline">Pinceau</span>
          <kbd className="ml-0.5 text-[9px] font-mono text-[#94a3b8]">B</kbd>
        </button>

        <button
          onClick={() => onSelectTool('eraser')}
          title="Outil Gomme [X]"
          className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-semibold transition border ${
            tool === 'eraser'
              ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7]'
              : 'text-[#94a3b8] border-transparent hover:bg-[#1e1338] hover:text-[#f8fafc]'
          }`}
        >
          <Eraser size={14} />
          <span className="hidden sm:inline">Gomme</span>
          <kbd className="ml-0.5 text-[9px] font-mono text-[#94a3b8]">X</kbd>
        </button>
      </div>

      <div className="w-px h-5 bg-[#2d1b4e]" />

      {/* Tuiles du pinceau */}
      <div className="flex items-center gap-1" title="Tuile active du pinceau [T]">
        {TILES.map(t => (
          <button
            key={t.id}
            onClick={() => onSelectTile(t.id)}
            title={t.label}
            className={`flex items-center gap-1.5 px-2 py-1.5 rounded-md text-[11px] font-medium transition border ${
              tile === t.id
                ? 'bg-[#150d27] text-[#f8fafc] border-[#a855f7]'
                : 'text-[#94a3b8] border-transparent hover:bg-[#1e1338] hover:text-[#cbd5e1]'
            }`}
          >
            <span
              className="w-2.5 h-2.5 rounded-sm shrink-0"
              style={{ backgroundColor: t.color }}
            />
            {t.label}
          </button>
        ))}
      </div>

      <div className="w-px h-5 bg-[#2d1b4e]" />

      {/* Historique + sauvegarde */}
      <div className="flex items-center gap-1 ml-auto">
        <button
          onClick={onUndo}
          disabled={!canUndo}
          title="Annuler [Ctrl+Z]"
          className={`p-1.5 rounded-md transition ${canUndo ? 'text-[#cbd5e1] hover:bg-[#1e1338] hover:text-[#f8fafc]' : 'text-[#2d1b4e] cursor-not-allowed'}`}
        >
          <Undo2 size={15} />
        </button>
        <button
          onClick={onRedo}
          disabled={!canRedo}
          title="Rétablir [Ctrl+Y]"
          className={`p-1.5 rounded-md transition ${canRedo ? 'text-[#cbd5e1] hover:bg-[#1e1338] hover:text-[#f8fafc]' : 'text-[#2d1b4e] cursor-not-allowed'}`}
        >
          <Redo2 size={15} />
        </button>
        <button
          onClick={onSave}
          title="Enregistrer la scène et les scripts [Ctrl+S]"
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] border border-[#3b0764] rounded-md text-xs font-medium transition"
        >
          <Save size={13} className="text-[#34d399]" />
          <span className="hidden sm:inline">Enregistrer</span>
        </button>
      </div>
    </div>
  );
};
