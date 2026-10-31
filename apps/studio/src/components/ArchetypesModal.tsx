import { useEffect, useState } from 'react';
import { ArrowRight, Gamepad2, X } from 'lucide-react';
import { ARCHETYPES } from '../services/archetypes';

interface ArchetypesModalProps {
  isOpen: boolean;
  onClose: () => void;
  onLaunchArchetype: (archetypeId: string) => Promise<void>;
  onLaunchArchetype3D: (archetypeId: string) => Promise<void>;
}

/** Galerie plein écran des 5 archétypes de jeu « en 1 clic ». */
export const ArchetypesModal: React.FC<ArchetypesModalProps> = ({ isOpen, onClose, onLaunchArchetype, onLaunchArchetype3D }) => {
  const [launching, setLaunching] = useState<{ id: string; dim: '2d' | '3d' } | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handleLaunch = async (id: string, dim: '2d' | '3d') => {
    setLaunching({ id, dim });
    try {
      if (dim === '2d') await onLaunchArchetype(id);
      else await onLaunchArchetype3D(id);
    } finally {
      setLaunching(null);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md select-none"
      onClick={onClose}
    >
      <div
        className="relative flex flex-col w-full max-w-6xl max-h-[92vh] bg-[#0d0819] border border-[#a855f7] rounded-xl shadow-2xl shadow-purple-950/90 overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        {/* En-tête */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#150d27] border-b border-[#2d1b4e]">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-8 h-8 rounded bg-gradient-to-br from-[#7e22ce] to-[#2e1065] border border-[#a855f7]">
              <Gamepad2 size={16} className="text-[#f8fafc]" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-[#f8fafc] tracking-wide">5 ARCHÉTYPES DE JEU EN 1 CLIC</h2>
              <p className="text-xs text-[#94a3b8]">
                Chaque archétype lance son jeu de référence complet — scène officielle ciselée, prête à jouer.
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] rounded-lg transition"
            aria-label="Fermer"
          >
            <X size={18} />
          </button>
        </div>

        {/* Cartes */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
            {ARCHETYPES.map(a => (
              <div
                key={a.id}
                className="flex flex-col rounded-lg overflow-hidden border bg-[#150d27] transition hover:shadow-xl"
                style={{ borderColor: a.borderColor }}
              >
                {/* En-tête de carte */}
                <div
                  className="px-4 py-3 flex items-center gap-3"
                  style={{ background: `linear-gradient(135deg, ${a.borderColor}33, transparent)` }}
                >
                  <span className="text-2xl leading-none">{a.emoji}</span>
                  <div className="flex-1 min-w-0">
                    <div className="font-bold text-sm" style={{ color: a.color }}>{a.name}</div>
                    <div className="text-[11px] text-[#94a3b8]">{a.genre}</div>
                  </div>
                </div>

                <div className="flex-1 px-4 py-3 space-y-3">
                  <p className="text-xs text-[#cbd5e1]">{a.description}</p>
                  <div className="text-[11px] font-semibold text-[#38bdf8]">🎮 {a.defaultGame}</div>

                  <div>
                    <div className="text-[10px] uppercase tracking-wider text-[#94a3b8] mb-1">Contenus</div>
                    <ul className="space-y-1 text-[11px] text-[#cbd5e1]">
                      {a.features.map((f, i) => (
                        <li key={i} className="flex items-center gap-1.5">
                          <span className="w-1 h-1 rounded-full bg-[#a855f7]" />{f}
                        </li>
                      ))}
                    </ul>
                  </div>

                  <div>
                    <div className="text-[10px] uppercase tracking-wider text-[#94a3b8] mb-1">Contrôles</div>
                    <ul className="space-y-1 text-[11px] text-[#94a3b8]">
                      {a.controls.map((c, i) => (
                        <li key={i} className="flex items-center gap-1.5">⌨️ {c}</li>
                      ))}
                    </ul>
                  </div>
                </div>

                {/* Pied de carte */}
                <div className="p-3 border-t border-[#2d1b4e] space-y-2">
                  <button
                    onClick={() => handleLaunch(a.id, '2d')}
                    disabled={launching !== null}
                    className="w-full py-2 text-center rounded text-xs font-bold text-white transition hover:opacity-90 disabled:opacity-60 flex items-center justify-center gap-1"
                    style={{ backgroundColor: a.borderColor }}
                  >
                    {launching?.id === a.id && launching.dim === '2d' ? 'Lancement…' : <>Démo 2D <ArrowRight size={12} /></>}
                  </button>
                  <button
                    onClick={() => handleLaunch(a.id, '3d')}
                    disabled={launching !== null}
                    className="w-full py-2 text-center rounded text-xs font-bold transition disabled:opacity-60 flex items-center justify-center gap-1 border"
                    style={{ color: a.color, borderColor: a.borderColor }}
                  >
                    {launching?.id === a.id && launching.dim === '3d' ? 'Lancement…' : <>Démo 3D <ArrowRight size={12} /></>}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Barre inférieure */}
        <div className="flex items-center justify-between px-6 py-3.5 bg-[#150d27] border-t border-[#2d1b4e]">
          <span className="text-xs text-[#94a3b8]">Échap pour fermer — le jeu se lance en mode Jeu (Play).</span>
          <button
            onClick={onClose}
            className="px-4 py-2 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] rounded-lg text-xs font-medium transition"
          >
            Fermer [Échap]
          </button>
        </div>
      </div>
    </div>
  );
};
