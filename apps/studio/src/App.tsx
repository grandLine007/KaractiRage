import { useState, useEffect, useCallback } from 'react';
import { HeaderBar } from './components/HeaderBar';
import type { StudioMode } from './components/HeaderBar';
import { GameViewport } from './components/GameViewport';
import { EntityHierarchy } from './components/EntityHierarchy';
import type { EntityItem } from './components/EntityHierarchy';
import { EntityInspector } from './components/EntityInspector';
import { AiAssistantModal } from './components/AiAssistantModal';
import { ProjectHubModal } from './components/ProjectHubModal';
import { RecipeWizardModal } from './components/RecipeWizardModal';
import { KvlGraphEditor } from './components/KvlGraphEditor';
import {
  applyArchetype,
  toggleEditorMode,
  downloadStandaloneGame,
  applyAiPayload,
  acceptAiPatch,
  rejectAiPatch,
  setActiveTool,
  cycleBrushTile,
} from './services/engineBridge';
import { Check, X } from 'lucide-react';

const INITIAL_ENTITIES: EntityItem[] = [
  { id: '1', name: 'Héros Ninja', kind: 'player', x: 200, y: 350, health: 5, spriteArchetype: 'ninja' },
  { id: '2', name: 'Boss Démon Titan', kind: 'boss', x: 850, y: 320, health: 120, spriteArchetype: 'boss' },
  { id: '3', name: 'Garde Patrouilleur A*', kind: 'enemy', x: 500, y: 360, health: 15, spriteArchetype: 'drone' },
  { id: '4', name: 'Coffre au Trésor Scellé', kind: 'item', x: 700, y: 370, spriteArchetype: 'chest' },
  { id: '5', name: 'Clé Dorée de Donjon', kind: 'item', x: 380, y: 370, spriteArchetype: 'key' },
  { id: '6', name: 'Zone de Lave Ardente', kind: 'hazard', x: 550, y: 384, spriteArchetype: 'spikes' },
  { id: '7', name: 'Portail Dimensionnel', kind: 'portal', x: 1050, y: 340, spriteArchetype: 'portal' },
  { id: '8', name: 'Sol de Pierre Principal', kind: 'platform', x: 0, y: 400, spriteArchetype: 'platform' },
];

export function App() {
  const [currentMode, setCurrentMode] = useState<StudioMode>('play');
  const [isHubOpen, setIsHubOpen] = useState(false);
  const [isAiModalOpen, setIsAiModalOpen] = useState(false);
  const [isWizardOpen, setIsWizardOpen] = useState(false);
  const [selectedEntityId, setSelectedEntityId] = useState<string | null>('1');
  const [entities, setEntities] = useState<EntityItem[]>(INITIAL_ENTITIES);
  const [pendingAiPatch, setPendingAiPatch] = useState<string | null>(null);

  const [metrics, setMetrics] = useState({
    fps: 60,
    cpuUs: 310,
    score: 0,
    status: 'Moteur KaractiRage Prêt • 60 FPS Déterministe'
  });

  // Bascule instantanée TAB sacrée (Commandement 7)
  const handleToggleTab = useCallback(() => {
    setCurrentMode(prev => {
      const next = prev === 'play' ? 'edit' : 'play';
      toggleEditorMode();
      return next;
    });
  }, []);

  // Validation ou Rollback du Patch IA
  const handleAcceptAi = useCallback(() => {
    acceptAiPatch();
    setPendingAiPatch(null);
    setMetrics(prev => ({
      ...prev,
      status: "✨ Patch IA validé et scellé dans le projet !"
    }));
  }, []);

  const handleRejectAi = useCallback(() => {
    rejectAiPatch();
    setPendingAiPatch(null);
    setMetrics(prev => ({
      ...prev,
      status: "✕ Patch IA rejeté — Scène restaurée à son état antérieur."
    }));
  }, []);

  // Gestion des raccourcis clavier globaux conformes aux spécifications
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (['input', 'textarea'].includes((e.target as HTMLElement)?.tagName?.toLowerCase())) {
        return;
      }

      // 1. Bascule Sacrée TAB (16ms)
      if (e.key === 'Tab') {
        e.preventDefault();
        handleToggleTab();
        return;
      }

      // 2. Gestion des modales
      if (e.key === 'h' || e.key === 'H') {
        setIsHubOpen(prev => !prev);
      } else if (e.key === 'i' || e.key === 'I') {
        setIsAiModalOpen(prev => !prev);
      } else if (e.key === 'w' || e.key === 'W') {
        setIsWizardOpen(prev => !prev);
      } else if (e.key === 'Escape') {
        if (pendingAiPatch) {
          handleRejectAi();
        } else {
          setIsHubOpen(false);
          setIsAiModalOpen(false);
          setIsWizardOpen(false);
        }
      } else if (e.key === 'Enter' && pendingAiPatch) {
        handleAcceptAi();
      }

      // 3. Raccourcis Outils Atelier Viewport (V, B, X, T)
      if (currentMode === 'edit') {
        if (e.key === 'v' || e.key === 'V') {
          setActiveTool('select');
        } else if (e.key === 'b' || e.key === 'B') {
          setActiveTool('brush');
        } else if (e.key === 'x' || e.key === 'X') {
          setActiveTool('eraser');
        } else if (e.key === 't' || e.key === 'T') {
          const tile = cycleBrushTile();
          if (tile) {
            setMetrics(prev => ({ ...prev, status: `Tuile du pinceau : ${tile} (T)` }));
          }
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [currentMode, pendingAiPatch, handleToggleTab, handleAcceptAi, handleRejectAi]);

  const selectedEntity = entities.find(e => e.id === selectedEntityId) || null;

  const handleUpdateEntity = (updated: EntityItem) => {
    setEntities(prev => prev.map(e => (e.id === updated.id ? updated : e)));
  };

  const handleDeleteEntity = (id: string) => {
    setEntities(prev => prev.filter(e => e.id !== id));
    if (selectedEntityId === id) {
      setSelectedEntityId(null);
    }
  };

  const handleAddEntity = () => {
    const newId = (entities.length + 1).toString();
    const newEnt: EntityItem = {
      id: newId,
      name: `Nouvelle Entité ${newId}`,
      kind: 'enemy',
      x: 300,
      y: 350,
      health: 10,
      spriteArchetype: 'slime'
    };
    setEntities(prev => [...prev, newEnt]);
    setSelectedEntityId(newId);
  };

  // Instanciation d'un des 5 Archétypes de Recette
  const handleLaunchArchetype = (archetypeId: string) => {
    const success = applyArchetype(archetypeId);
    setIsHubOpen(false);
    setCurrentMode('play');
    setMetrics(prev => ({
      ...prev,
      status: success
        ? `Archétype '${archetypeId.toUpperCase()}' généré avec succès • Prêt à jouer !`
        : `Échec du lancement de l'archétype '${archetypeId}'`
    }));
  };

  // Application du Payload généré par l'IA dans la scène Wasm
  const handleApplyGeneratedLevel = (payloadJson: string) => {
    try {
      const parsed = JSON.parse(payloadJson);
      const code = applyAiPayload(payloadJson, parsed.title || "Intention IA");
      if (code === 0) {
        setPendingAiPatch(parsed.title || "Patch IA");
        setIsAiModalOpen(false);
        setCurrentMode('play');
        setMetrics(prev => ({
          ...prev,
          status: `✨ Niveau IA '${parsed.title}' injecté dans le Viewport (Examinez le halo violet)`
        }));
      } else {
        alert(`Erreur d'application du payload IA (code ${code}).`);
      }
    } catch (err: any) {
      alert(`JSON de niveau invalide : ${err?.message}`);
    }
  };

  // Export Standalone HTML5 Universel (Commandement 9)
  const handleExportStandalone = () => {
    downloadStandaloneGame("MonJeuKaractiRage", "Créateur KaractiRage");
    setMetrics(prev => ({
      ...prev,
      status: "📦 Fichier HTML5 Standalone téléchargé avec succès ! Jouable sans serveur ni compte."
    }));
  };

  return (
    <div className="flex flex-col w-screen h-screen bg-[#07040d] text-[#f8fafc] overflow-hidden select-none font-sans">
      {/* 1. Barre Supérieure */}
      <HeaderBar
        currentMode={currentMode}
        onModeChange={mode => {
          if (mode !== currentMode) {
            toggleEditorMode();
            setCurrentMode(mode);
          }
        }}
        onOpenHub={() => setIsHubOpen(true)}
        onOpenAiAssistant={() => setIsAiModalOpen(true)}
        onExportStandalone={handleExportStandalone}
        metrics={metrics}
      />

      {/* 2. Bandeau Flottant de Révision IA (Boucle de Confiance & Rollback en 1 clic) */}
      {pendingAiPatch && (
        <div className="absolute top-14 left-1/2 -translate-x-1/2 z-40 flex items-center gap-3 px-4 py-2 bg-[#17082e]/95 border border-[#a855f7] rounded-full shadow-2xl shadow-purple-950/90 backdrop-blur-md animate-bounce">
          <span className="w-2.5 h-2.5 rounded-full bg-[#c084fc] animate-pulse" />
          <span className="text-xs font-bold text-[#f8fafc]">
            Patch IA en révision : <span className="text-[#38bdf8]">{pendingAiPatch}</span>
          </span>
          <div className="flex items-center gap-1.5 ml-2">
            <button
              onClick={handleAcceptAi}
              className="flex items-center gap-1 px-3 py-1 bg-[#059669] hover:bg-[#047857] text-white text-xs font-bold rounded-full transition shadow"
              title="Valider définitivement (Entrée)"
            >
              <Check size={13} />
              Valider [Entrée]
            </button>
            <button
              onClick={handleRejectAi}
              className="flex items-center gap-1 px-3 py-1 bg-[#881337] hover:bg-[#9f1239] text-[#fecdd3] text-xs font-bold rounded-full transition shadow"
              title="Annuler et restaurer l'état antérieur (Échap)"
            >
              <X size={13} />
              Rollback [Échap]
            </button>
          </div>
        </div>
      )}

      {/* 3. Espace de Travail Central Multi-Panneaux */}
      <div className="flex flex-1 overflow-hidden relative">
        {/* Panneau Gauche : Hiérarchie (Visible en mode Studio) */}
        {currentMode === 'edit' && (
          <EntityHierarchy
            entities={entities}
            selectedEntityId={selectedEntityId}
            onSelectEntity={setSelectedEntityId}
            onAddEntity={handleAddEntity}
          />
        )}

        {/* Zone Centrale : Viewport 2D ou Éditeur Nodal KVL */}
        <main className="flex-1 flex flex-col h-full overflow-hidden relative">
          {currentMode === 'kvl' ? (
            <KvlGraphEditor />
          ) : (
            <GameViewport
              onTelemetryUpdate={setMetrics}
              onSelectedEntityChange={name => {
                const found = entities.find(e => e.name === name);
                if (found) setSelectedEntityId(found.id);
              }}
            />
          )}
        </main>

        {/* Panneau Droit : Inspecteur (Visible en mode Studio) */}
        {currentMode === 'edit' && (
          <EntityInspector
            selectedEntity={selectedEntity}
            onUpdateEntity={handleUpdateEntity}
            onDeleteEntity={handleDeleteEntity}
          />
        )}
      </div>

      {/* 4. Barre de Statut Inférieure */}
      <footer className="flex items-center justify-between px-4 py-1.5 bg-[#0d0819] border-t border-[#2d1b4e] text-xs text-[#94a3b8]">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-[#34d399] animate-pulse" />
          <span className="font-medium text-[#cbd5e1]">{metrics.status}</span>
        </div>
        <div className="flex items-center gap-4 text-[11px] text-[#64748b]">
          <span>Touches : [Tab] Jouer/Studio • [W] Assistant 6 Étapes • [H] Hub • [I] IA • [Espace] Saut</span>
          <span className="text-[#a855f7] font-semibold">KaractiRage Studio v0.1.0</span>
        </div>
      </footer>

      {/* 5. Modales Flottantes */}
      <ProjectHubModal
        isOpen={isHubOpen}
        onClose={() => setIsHubOpen(false)}
        onLaunchArchetype={handleLaunchArchetype}
        onOpenWizard={() => {
          setIsHubOpen(false);
          setIsWizardOpen(true);
        }}
        onOpenAiTab={() => {
          setIsHubOpen(false);
          setIsAiModalOpen(true);
        }}
      />

      <RecipeWizardModal
        isOpen={isWizardOpen}
        onClose={() => setIsWizardOpen(false)}
        onProjectCreated={projTitle => {
          setCurrentMode('play');
          setMetrics(prev => ({
            ...prev,
            status: `Projet '${projTitle}' généré et prêt à jouer à 60 FPS !`
          }));
        }}
      />

      <AiAssistantModal
        isOpen={isAiModalOpen}
        onClose={() => setIsAiModalOpen(false)}
        onApplyGeneratedLevel={handleApplyGeneratedLevel}
      />
    </div>
  );
}

export default App;
