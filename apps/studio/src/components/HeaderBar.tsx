import { Play, PenTool, GitFork, Brush, Bot, FolderOpen, Download, Cpu, Activity, Zap, Wand2 } from 'lucide-react';

export type StudioMode = 'play' | 'edit' | 'kvl' | 'pixel' | 'vector';

interface HeaderBarProps {
  currentMode: StudioMode;
  onModeChange: (mode: StudioMode) => void;
  onOpenHub: () => void;
  onOpenAiAssistant: () => void;
  onExportStandalone: () => void;
  metrics: {
    fps: number;
    cpuUs: number;
    score: number;
    status: string;
  };
}

export const HeaderBar: React.FC<HeaderBarProps> = ({
  currentMode,
  onModeChange,
  onOpenHub,
  onOpenAiAssistant,
  onExportStandalone,
  metrics,
}) => {
  return (
    <header data-testid="header" className="flex items-center justify-between px-4 py-2.5 bg-[#0d0819] border-b border-[#2d1b4e] select-none">
      {/* Marque et Logo */}
      <div className="flex items-center gap-3">
        <div className="flex items-center justify-center w-8 h-8 rounded bg-gradient-to-br from-[#7e22ce] to-[#2e1065] border border-[#a855f7] shadow-lg shadow-purple-950/50">
          <span className="font-extrabold text-sm text-[#f8fafc] tracking-wider">KR</span>
        </div>
        <div>
          <div className="flex items-center gap-2">
            <h1 className="font-bold text-sm tracking-wide text-[#f8fafc]">KARACTIRAGE STUDIO</h1>
            <span className="badge-tag bg-[#2e1065] text-[#c084fc] border border-[#a855f7]/50">
              TAURI v2 • REACT
            </span>
          </div>
          <p className="text-[11px] text-[#94a3b8]">Atelier de création 2D multi-genres • Logique Visuelle KVL</p>
        </div>
      </div>

      {/* Sélecteur de Mode Segmenté */}
      <div data-testid="header-modes" className="flex items-center p-1 bg-[#07040d] border border-[#2d1b4e] rounded-lg">
        <button
          onClick={() => onModeChange('play')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-semibold transition ${
            currentMode === 'play'
              ? 'bg-[#3b0764] text-[#f3e8ff] border border-[#a855f7] shadow-md shadow-purple-950/40'
              : 'text-[#94a3b8] hover:text-[#f8fafc]'
          }`}
        >
          <Play size={14} className={currentMode === 'play' ? 'text-[#38bdf8]' : ''} />
          JOUER (PLAY)
        </button>

        <button
          onClick={() => onModeChange('edit')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-semibold transition ${
            currentMode === 'edit'
              ? 'bg-[#3b0764] text-[#f3e8ff] border border-[#a855f7] shadow-md shadow-purple-950/40'
              : 'text-[#94a3b8] hover:text-[#f8fafc]'
          }`}
        >
          <PenTool size={14} className={currentMode === 'edit' ? 'text-[#a855f7]' : ''} />
          ATELIER STUDIO
        </button>

        <button
          onClick={() => onModeChange('kvl')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-semibold transition ${
            currentMode === 'kvl'
              ? 'bg-[#3b0764] text-[#f3e8ff] border border-[#a855f7] shadow-md shadow-purple-950/40'
              : 'text-[#94a3b8] hover:text-[#f8fafc]'
          }`}
        >
          <GitFork size={14} className={currentMode === 'kvl' ? 'text-[#34d399]' : ''} />
          LOGIQUE KVL
        </button>

        <button
          onClick={() => onModeChange('pixel')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-semibold transition ${
            currentMode === 'pixel'
              ? 'bg-[#3b0764] text-[#f3e8ff] border border-[#a855f7] shadow-md shadow-purple-950/40'
              : 'text-[#94a3b8] hover:text-[#f8fafc]'
          }`}
        >
          <Brush size={14} className={currentMode === 'pixel' ? 'text-[#a855f7]' : ''} />
          PIXEL ART
        </button>

        <button
          onClick={() => onModeChange('vector')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-semibold transition ${
            currentMode === 'vector'
              ? 'bg-[#3b0764] text-[#f3e8ff] border border-[#a855f7] shadow-md shadow-purple-950/40'
              : 'text-[#94a3b8] hover:text-[#f8fafc]'
          }`}
        >
          <Wand2 size={14} className={currentMode === 'vector' ? 'text-[#38bdf8]' : ''} />
          VECTORISATION
        </button>
      </div>

      {/* Télémétrie en temps réel */}
      <div className="hidden lg:flex items-center gap-2">
        <div className="flex items-center gap-1.5 px-2.5 py-1 bg-[#150d27] border border-[#2d1b4e] rounded text-xs">
          <Activity size={13} className={metrics.fps >= 55 ? 'text-[#34d399]' : 'text-[#f59e0b]'} />
          <span className="font-mono font-bold text-[#f8fafc]">{metrics.fps || 60}</span>
          <span className="text-[10px] text-[#94a3b8]">FPS</span>
        </div>

        <div className="flex items-center gap-1.5 px-2.5 py-1 bg-[#150d27] border border-[#2d1b4e] rounded text-xs">
          <Cpu size={13} className="text-[#38bdf8]" />
          <span className="font-mono font-bold text-[#f8fafc]">{metrics.cpuUs || 320}</span>
          <span className="text-[10px] text-[#94a3b8]">µs CPU</span>
        </div>

        <div className="flex items-center gap-1.5 px-2.5 py-1 bg-[#150d27] border border-[#2d1b4e] rounded text-xs">
          <Zap size={13} className="text-[#f59e0b]" />
          <span className="font-mono font-bold text-[#f8fafc]">{metrics.score || 0}</span>
          <span className="text-[10px] text-[#94a3b8]">SCORE</span>
        </div>
      </div>

      {/* Actions Principales */}
      <div className="flex items-center gap-2">
        <button
          onClick={onOpenHub}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] border border-[#3b0764] rounded-md text-xs font-medium transition"
          title="Hub des Projets & Accueil [H]"
        >
          <FolderOpen size={14} className="text-[#38bdf8]" />
          Hub [H]
        </button>

        <button
          onClick={onOpenAiAssistant}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] text-white rounded-md text-xs font-semibold shadow-md shadow-purple-950/50 transition"
          title="Assistant Intention IA [I]"
        >
          <Bot size={14} className="text-[#f3e8ff]" />
          Intention IA [I]
        </button>

        <button
          onClick={onExportStandalone}
          className="flex items-center gap-1.5 px-2.5 py-1.5 bg-[#150d27] hover:bg-[#1e1338] text-[#94a3b8] hover:text-[#f8fafc] border border-[#2d1b4e] rounded-md text-xs transition"
          title="Exporter le jeu standalone HTML5"
        >
          <Download size={14} />
          Export
        </button>
      </div>
    </header>
  );
};
