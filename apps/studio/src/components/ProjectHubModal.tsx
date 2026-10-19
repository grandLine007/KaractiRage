import { useState } from 'react';
import { Sparkles, Gamepad2, FolderOpen, Bot, ArrowRight, X } from 'lucide-react';

interface ProjectHubModalProps {
  isOpen: boolean;
  onClose: () => void;
  onLaunchArchetype: (archetypeId: string) => void;
  onOpenWizard: () => void;
  onOpenAiTab: () => void;
}

const ARCHETYPES = [
  {
    id: 'rpg',
    name: 'RPG DONJON',
    genre: 'Top-Down 2D',
    color: '#10b981',
    borderColor: '#059669',
    features: ['Combat à l’épée', 'Inventaire 5 slots', 'Portes & clés', 'Coffres au trésor']
  },
  {
    id: 'platformer',
    name: 'PLATFORMER',
    genre: 'Vue Latérale 2D',
    color: '#8b5cf6',
    borderColor: '#7c3aed',
    features: ['Gravité & Sauts', 'Moteurs mobiles', 'Dalles réactives', 'Dash cinétique']
  },
  {
    id: 'shmup',
    name: 'SHMUP SPATIAL',
    genre: 'Action Espace',
    color: '#ec4899',
    borderColor: '#db2777',
    features: ['Inertie spatiale', 'Lasers balistiques', 'Vagues d’ennemis', 'Score réactif KVL']
  },
  {
    id: 'sokoban',
    name: 'PUZZLE SOKOBAN',
    genre: 'Grille Tactique',
    color: '#f59e0b',
    borderColor: '#d97706',
    features: ['Poussée de blocs', 'Dalles de sol', 'Énigmes logiques', 'Déplacement case']
  },
  {
    id: 'boss',
    name: 'TITAN BOSS',
    genre: 'Combat Arène',
    color: '#ef4444',
    borderColor: '#dc2626',
    features: ['Boss géant KVL', 'Télégraphie frappe', 'Barres de santé', 'Esquives & Dash']
  }
];

export const ProjectHubModal: React.FC<ProjectHubModalProps> = ({
  isOpen,
  onClose,
  onLaunchArchetype,
  onOpenWizard,
  onOpenAiTab,
}) => {
  const [activeTab, setActiveTab] = useState<0 | 1 | 2>(0);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md select-none">
      <div className="relative flex flex-col w-full max-w-4xl max-h-[90vh] bg-[#0d0819] border border-[#a855f7] rounded-xl shadow-2xl shadow-purple-950/90 overflow-hidden">
        {/* En-tête du Hub */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#150d27] border-b border-[#2d1b4e]">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-8 h-8 rounded bg-gradient-to-br from-[#7e22ce] to-[#2e1065] border border-[#a855f7]">
              <span className="font-extrabold text-sm text-[#f8fafc]">KR</span>
            </div>
            <div>
              <h2 className="text-sm font-bold text-[#f8fafc] tracking-wide">
                KARACTIRAGE ATELIER — HUB DE PROJETS UNIFIÉ
              </h2>
              <p className="text-xs text-[#94a3b8]">
                Atelier de création 2D multi-genres • Logique Visuelle KVL • Déterministe 60 FPS
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] rounded-lg transition"
          >
            <X size={18} />
          </button>
        </div>

        {/* Barre d'Onglets 1.1 / 1.2 / 1.3 */}
        <div className="flex items-center px-6 pt-3 bg-[#0d0819] border-b border-[#2d1b4e] gap-2">
          <button
            onClick={() => setActiveTab(0)}
            className={`flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-t-lg border-t border-x transition ${
              activeTab === 0
                ? 'bg-[#150d27] text-[#c084fc] border-[#a855f7]'
                : 'text-[#94a3b8] border-transparent hover:text-[#f8fafc]'
            }`}
          >
            <Sparkles size={14} />
            1.1 NOUVEAU PROJET / 6 ÉTAPES
          </button>

          <button
            onClick={() => setActiveTab(1)}
            className={`flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-t-lg border-t border-x transition ${
              activeTab === 1
                ? 'bg-[#150d27] text-[#c084fc] border-[#a855f7]'
                : 'text-[#94a3b8] border-transparent hover:text-[#f8fafc]'
            }`}
          >
            <FolderOpen size={14} />
            1.2 PROJETS SUR DISQUE
          </button>

          <button
            onClick={() => {
              setActiveTab(2);
              onOpenAiTab();
            }}
            className={`flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-t-lg border-t border-x transition ${
              activeTab === 2
                ? 'bg-[#150d27] text-[#c084fc] border-[#a855f7]'
                : 'text-[#94a3b8] border-transparent hover:text-[#f8fafc]'
            }`}
          >
            <Bot size={14} />
            1.3 INTENTION IA & CONFIG LLM [I]
          </button>
        </div>

        {/* Corps de l'onglet actif */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {activeTab === 0 && (
            <>
              {/* Bannière Assistant 6 Étapes */}
              <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between p-4 bg-[#170f28] border border-[#a855f7] rounded-lg shadow-inner gap-4">
                <div className="flex items-center gap-3">
                  <div className="px-2.5 py-1 bg-[#2e1065] text-[#c084fc] font-bold text-xs rounded border border-[#c084fc]/40">
                    1.1
                  </div>
                  <div>
                    <h3 className="font-bold text-sm text-[#f8fafc]">
                      ASSISTANT DE CRÉATION GUIDÉ (6 ÉTAPES)
                    </h3>
                    <p className="text-xs text-[#94a3b8]">
                      Configurez genres, héros, physique et gameplay 2D pas à pas.
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <button
                    onClick={onOpenAiTab}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-[#2e1065] hover:bg-[#3b0764] text-[#c084fc] border border-[#a855f7] rounded text-xs font-semibold transition"
                  >
                    <Bot size={13} />
                    Intention IA [I]
                  </button>
                  <button
                    onClick={onOpenWizard}
                    className="flex items-center gap-1.5 px-3.5 py-1.5 bg-[#7e22ce] hover:bg-[#9333ea] text-white rounded text-xs font-bold shadow transition"
                  >
                    Assistant [W]
                    <ArrowRight size={13} />
                  </button>
                </div>
              </div>

              {/* Grille des 5 Archétypes en 1 clic */}
              <div>
                <h4 className="text-xs font-bold text-[#38bdf8] uppercase tracking-wider mb-3 flex items-center gap-2">
                  <Gamepad2 size={15} />
                  Ou démarrage immédiat : 5 archétypes de jeu en 1 clic
                </h4>
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
                  {ARCHETYPES.map(a => (
                    <div
                      key={a.id}
                      className="flex flex-col justify-between p-3.5 bg-[#150d27] border rounded-lg hover:shadow-lg transition group"
                      style={{ borderColor: a.borderColor }}
                    >
                      <div>
                        <div className="flex items-center justify-between mb-1.5">
                          <span className="font-bold text-xs" style={{ color: a.color }}>
                            {a.name}
                          </span>
                        </div>
                        <span className="text-[11px] font-medium text-[#94a3b8] block mb-2.5">
                          {a.genre}
                        </span>
                        <ul className="space-y-1 mb-4 text-[11px] text-[#cbd5e1]">
                          {a.features.map((f, i) => (
                            <li key={i} className="flex items-center gap-1.5">
                              <span className="w-1 h-1 rounded-full bg-[#a855f7]" />
                              {f}
                            </li>
                          ))}
                        </ul>
                      </div>

                      <button
                        onClick={() => onLaunchArchetype(a.id)}
                        className="w-full py-1.5 text-center rounded text-xs font-bold text-white transition hover:opacity-90 flex items-center justify-center gap-1"
                        style={{ backgroundColor: a.borderColor }}
                      >
                        Lancer
                        <ArrowRight size={12} />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            </>
          )}

          {activeTab === 1 && (
            <div className="p-8 text-center bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-3">
              <FolderOpen size={32} className="mx-auto text-[#a855f7]" />
              <h3 className="font-bold text-sm text-[#f8fafc]">Projets et Scènes sur Disque</h3>
              <p className="text-xs text-[#94a3b8] max-w-md mx-auto">
                Chargez vos scènes sauvegardées au format `.karacti.json` ou parcourez les démos officielles de KaractiRage.
              </p>
              <div className="pt-2 flex justify-center gap-3">
                <button
                  onClick={() => onLaunchArchetype('rpg')}
                  className="px-4 py-2 bg-[#2e1065] text-[#c084fc] border border-[#a855f7] rounded text-xs font-semibold"
                >
                  Charger Scène Démo (Donjon)
                </button>
              </div>
            </div>
          )}

          {activeTab === 2 && (
            <div className="p-6 text-center bg-[#150d27] border border-[#a855f7] rounded-lg space-y-4">
              <Bot size={36} className="mx-auto text-[#c084fc]" />
              <h3 className="font-bold text-sm text-[#f8fafc]">Configuration & Intention IA</h3>
              <p className="text-xs text-[#94a3b8] max-w-md mx-auto">
                Connectez LM Studio, Ollama ou vos clés d’API Claude/Gemini pour générer des niveaux entiers par intention textuelle en français.
              </p>
              <button
                onClick={onOpenAiTab}
                className="px-5 py-2.5 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] text-white rounded-lg text-xs font-bold shadow-lg shadow-purple-950/60"
              >
                Ouvrir la Console Assistant IA
              </button>
            </div>
          )}
        </div>

        {/* Barre d'Action Inférieure */}
        <div className="flex items-center justify-between px-6 py-3.5 bg-[#150d27] border-t border-[#2d1b4e]">
          <span className="text-xs text-[#64748b]">Appuyez sur Échap pour fermer à tout moment</span>
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
