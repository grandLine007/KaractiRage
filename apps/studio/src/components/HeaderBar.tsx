import { Play, PenTool, GitFork, Brush, Bot, FolderOpen, Download, Wand2 } from 'lucide-react';

export type StudioMode = 'play' | 'edit' | 'kvl' | 'pixel' | 'vector';

interface HeaderBarProps {
  currentMode: StudioMode;
  onModeChange: (mode: StudioMode) => void;
  onOpenHub: () => void;
  onOpenAiAssistant: () => void;
  onExportStandalone: () => void;
}

const MODES: { id: StudioMode; label: string; full: string; icon: React.ReactNode }[] = [
  { id: 'play', label: 'Jouer', full: 'Jouer (Play)', icon: <Play size={15} /> },
  { id: 'edit', label: 'Créer', full: 'Créer — Atelier Studio', icon: <PenTool size={15} /> },
  { id: 'kvl', label: 'Logique', full: 'Logique KVL', icon: <GitFork size={15} /> },
  { id: 'pixel', label: 'Pixel', full: 'Pixel Art', icon: <Brush size={15} /> },
  { id: 'vector', label: 'Vectoriel', full: 'Vectorisation', icon: <Wand2 size={15} /> },
];

export const HeaderBar: React.FC<HeaderBarProps> = ({
  currentMode,
  onModeChange,
  onOpenHub,
  onOpenAiAssistant,
  onExportStandalone,
}) => {
  return (
    <header className="flex items-center justify-between gap-3 px-3 h-14 min-h-[56px] bg-[#0d0819] border-b border-[#2d1b4e] select-none shrink-0">
      {/* Zone gauche : marque discrète */}
      <div className="flex items-center gap-2 shrink-0">
        <div className="flex items-center justify-center w-8 h-8 rounded bg-gradient-to-br from-[#7e22ce] to-[#2e1065] border border-[#a855f7] shadow-md shadow-purple-950/50">
          <span className="font-extrabold text-sm text-[#f8fafc] tracking-wider">KR</span>
        </div>
        <h1 className="font-bold text-sm tracking-wide text-[#f8fafc] whitespace-nowrap">KARACTIRAGE STUDIO</h1>
      </div>

      {/* Zone centre : modes (libellés courts + ARIA) */}
      <nav className="flex items-center gap-0.5 p-0.5 bg-[#07040d] border border-[#2d1b4e] rounded-lg shrink-0" aria-label="Modes de l'atelier">
        {MODES.map((m) => {
          const active = currentMode === m.id;
          return (
            <button
              key={m.id}
              onClick={() => onModeChange(m.id)}
              aria-pressed={active}
              title={m.full}
              className={`flex items-center gap-1.5 h-8 px-3 rounded-md text-xs font-semibold whitespace-nowrap transition ${
                active
                  ? 'bg-[#3b0764] text-[#f3e8ff] border border-[#a855f7] shadow-md shadow-purple-950/40'
                  : 'text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#150d27]'
              }`}
            >
              {m.icon}
              {m.label}
            </button>
          );
        })}
      </nav>

      {/* Zone droite : actions secondaires (cibles ≥ 32 px) */}
      <div className="flex items-center gap-1 shrink-0">
        <button
          onClick={onOpenHub}
          className="flex items-center justify-center w-8 h-8 rounded-md text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] transition"
          title="Hub des Projets & Accueil [H]"
          aria-label="Hub des Projets & Accueil [H]"
        >
          <FolderOpen size={16} />
        </button>
        <button
          onClick={onOpenAiAssistant}
          className="flex items-center justify-center w-8 h-8 rounded-md bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] text-white shadow-md shadow-purple-950/50 transition"
          title="Assistant Intention IA [I]"
          aria-label="Assistant Intention IA [I]"
        >
          <Bot size={16} />
        </button>
        <button
          onClick={onExportStandalone}
          className="flex items-center justify-center w-8 h-8 rounded-md text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] transition"
          title="Exporter le jeu standalone HTML5"
          aria-label="Exporter le jeu standalone HTML5"
        >
          <Download size={16} />
        </button>
      </div>
    </header>
  );
};
