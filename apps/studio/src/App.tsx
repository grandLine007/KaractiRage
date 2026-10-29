import { useState, useEffect, useCallback, useRef } from 'react';
import { HeaderBar } from './components/HeaderBar';
import type { StudioMode } from './components/HeaderBar';
import { GameViewport } from './components/GameViewport';
import { EntityHierarchy } from './components/EntityHierarchy';
import { EntityInspector } from './components/EntityInspector';
import { StudioToolbar } from './components/StudioToolbar';
import { SpawnPalette } from './components/SpawnPalette';
import { SuggestionPanel } from './components/SuggestionPanel';
import { ToastCenter, type Toast } from './components/ToastCenter';
import { AiAssistantModal } from './components/AiAssistantModal';
import { ProjectHubModal } from './components/ProjectHubModal';
import { ArchetypesModal } from './components/ArchetypesModal';
import { RecipeWizardModal } from './components/RecipeWizardModal';
import { KvlGraphEditor } from './components/KvlGraphEditor';
import { PxlArtWorkspace } from './components/PxlArtWorkspace';
import { VectorWorkspace } from './components/VectorWorkspace';
import { useStudioState } from './hooks/useStudioState';
import {
  launchArchetypeGame,
  launchArchetypeGame3D,
  downloadStandaloneGame,
  applyAiPayload,
  acceptAiPatch,
  rejectAiPatch,
  setActiveTool,
  cycleBrushTile,
  setBrushTile,
  selectEntity,
  updateEntityTransform,
  duplicateSelectedEntity,
  deleteSelectedEntity,
  undo,
  redo,
  saveScene,
  spawnEntityAtScreen,
  spawn3dPreset,
  setEditorMode,
  getEngineStatus,
  type StudioEntitySummary,
} from './services/engineBridge';
import { Check, X, HelpCircle } from 'lucide-react';
import type { ProjectEntry } from './services/projectBridge';
import { prewarm3D } from './render3d/prewarm';

export function App() {
  const [currentMode, setCurrentMode] = useState<StudioMode>(() => {
    const saved = localStorage.getItem('karactirage.lastMode');
    return saved === 'edit' || saved === 'kvl' || saved === 'pixel' || saved === 'vector' ? saved : 'play';
  });
  // Le Hub des Projets est l'écran d'accueil : il s'affiche au lancement (le moteur
  // boote en arrière-plan derrière l'overlay, la scène n'est pas lancée d'emblée).
  const [isHubOpen, setIsHubOpen] = useState(true);
  const [isAiModalOpen, setIsAiModalOpen] = useState(false);
  const [isWizardOpen, setIsWizardOpen] = useState(false);
  const [isArchetypesOpen, setIsArchetypesOpen] = useState(false);
  const [view3d, setView3d] = useState(false);
  const [pendingAiPatch, setPendingAiPatch] = useState<string | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [isHelpOpen, setIsHelpOpen] = useState(false);

  const [metrics, setMetrics] = useState({
    fps: 60,
    cpuUs: 310,
    score: 0,
    status: 'Ouverture de KaractiRage Studio…'
  });

  // État d'édition natif (polling suspendu hors mode Studio).
  const { state: editorState, refresh } = useStudioState(currentMode === 'edit');

  // Références stables pour l'autosauvegarde (UX-204) sans recréer les callbacks.
  const currentModeRef = useRef(currentMode);
  const editorStateRef = useRef<{ can_undo?: boolean } | null>(null);
  useEffect(() => {
    currentModeRef.current = currentMode;
    localStorage.setItem('karactirage.lastMode', currentMode);
  }, [currentMode]);
  useEffect(() => {
    editorStateRef.current = editorState;
  }, [editorState]);

  const toastIdRef = useRef(0);
  const pushToast = useCallback((message: string, kind: Toast['kind'] = 'info') => {
    const id = ++toastIdRef.current;
    setToasts(prev => [...prev, { id, message, kind }]);
    window.setTimeout(() => setToasts(prev => prev.filter(t => t.id !== id)), 4200);
  }, []);
  const dismissToast = useCallback((id: number) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  // Préchauffe la 3D (chunk three.js + sonde GPU) en tâche de fond : n'affecte pas
  // l'ouverture, mais rend la première bascule 3D quasi immédiate.
  useEffect(() => {
    prewarm3D();
  }, []);

  // Enrichit le statut avec le temps d'init du moteur (boot non bloquant). Au
  // montage, le boot est le plus souvent encore `booting` : on interroge jusqu'à
  // un état terminal (ready/failed) pour ne jamais afficher « Prêt » à tort.
  useEffect(() => {
    let active = true;
    let timer: number | null = null;

    const poll = async () => {
      try {
        const s = await getEngineStatus();
        if (!active) return;
        if (s.state === 'ready') {
          setMetrics(prev => ({
            ...prev,
            status: `Moteur KaractiRage Prêt (${s.init_ms} ms) • 60 FPS Déterministe`,
          }));
          return; // état terminal
        }
        if (s.state === 'failed') {
          setMetrics(prev => ({
            ...prev,
            status: `Moteur indisponible : ${s.error ?? 'erreur inconnue'}`,
          }));
          return; // état terminal
        }
        setMetrics(prev => ({ ...prev, status: 'Préparation du moteur KaractiRage…' }));
      } catch {
        /* indisponible temporaire : le prochain tick retentera */
      }
      if (active) timer = window.setTimeout(poll, 250);
    };

    void poll();
    return () => {
      active = false;
      if (timer !== null) window.clearTimeout(timer);
    };
  }, []);

  const entities: StudioEntitySummary[] = editorState?.entities ?? [];
  const selectedEntityId = editorState?.selected_id ?? null;
  const selectedEntity: StudioEntitySummary | null =
    entities.find(e => e.id === selectedEntityId) ?? null;

  // Bascule instantanée TAB (Jeu <-> Studio), avec moteur synchronisé.
  const handleToggleTab = useCallback(() => {
    const prev = currentModeRef.current;
    const next = prev === 'play' ? 'edit' : 'play';
    // Autosauvegarde avant de quitter l'édition (UX-204)
    if (prev === 'edit' && editorStateRef.current?.can_undo) {
      void saveScene().then(() => setMetrics((m) => ({ ...m, status: "💾 Scène autosauvegardée avant de quitter l'atelier" })));
    }
    void setEditorMode(next);
    setCurrentMode(next);
  }, []);

  const applyMode = useCallback((mode: StudioMode) => {
    const prev = currentModeRef.current;
    if (prev === mode) return;
    // Autosauvegarde avant de quitter l'édition si des modifications sont en attente (UX-204)
    if (prev === 'edit' && editorStateRef.current?.can_undo) {
      void saveScene().then(() => setMetrics((m) => ({ ...m, status: "💾 Scène autosauvegardée avant de quitter l'atelier" })));
    }
    void setEditorMode(mode === 'play' ? 'play' : 'edit');
    setCurrentMode(mode);
  }, []);

  // Validation ou Rollback du Patch IA
  const handleAcceptAi = useCallback(() => {
    void acceptAiPatch();
    setPendingAiPatch(null);
    setMetrics(prev => ({
      ...prev,
      status: "✨ Patch IA validé et scellé dans le projet !"
    }));
  }, []);

  const handleRejectAi = useCallback(() => {
    void rejectAiPatch();
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

      // 1bis. Raccourcis de mode Ctrl+1..5 / F6 (UX-202)
      if ((e.ctrlKey || e.metaKey) && ['1', '2', '3', '4', '5'].includes(e.key)) {
        e.preventDefault();
        const order: StudioMode[] = ['play', 'edit', 'kvl', 'pixel', 'vector'];
        applyMode(order[Number(e.key) - 1]);
        return;
      }
      if (e.key === 'F6') {
        e.preventDefault();
        const order: StudioMode[] = ['play', 'edit', 'kvl', 'pixel', 'vector'];
        const idx = order.indexOf(currentMode);
        applyMode(order[(idx + 1) % order.length]);
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
          setIsHelpOpen(false);
        }
      } else if (e.key === 'Enter' && pendingAiPatch) {
        handleAcceptAi();
      }

      // 3. Raccourcis Outils Atelier Viewport (V, B, X, T)
      if (currentMode === 'edit') {
        if (e.key === 'v' || e.key === 'V') {
          void setActiveTool('select').then(() => refresh());
        } else if (e.key === 'b' || e.key === 'B') {
          void setActiveTool('brush').then(() => refresh());
        } else if (e.key === 'x' || e.key === 'X') {
          void setActiveTool('eraser').then(() => refresh());
        } else if (e.key === 't' || e.key === 'T') {
          void cycleBrushTile().then(tile => {
            if (tile) {
              refresh();
              setMetrics(prev => ({ ...prev, status: `Tuile du pinceau : ${tile} (T)` }));
            }
          });
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [currentMode, pendingAiPatch, handleToggleTab, handleAcceptAi, handleRejectAi, refresh, applyMode]);

  // --- Handlers d'édition connectés au moteur natif -------------------------

  const handleSelectEntity = useCallback((id: number) => {
    void selectEntity(id).then(() => refresh());
  }, [refresh]);

  const handleUpdateTransform = useCallback((id: number, x: number, y: number, w: number, h: number) => {
    void updateEntityTransform(id, x, y, w, h, true).then(() => {
      refresh();
      pushToast('Position enregistrée (Ctrl+Z pour annuler).', 'success');
    });
  }, [refresh, pushToast]);

  const handleDuplicate = useCallback((id: number) => {
    void selectEntity(id)
      .then(() => duplicateSelectedEntity())
      .then(() => refresh());
  }, [refresh]);

  const handleDelete = useCallback((id: number) => {
    void selectEntity(id)
      .then(() => deleteSelectedEntity())
      .then(ok => {
        refresh();
        pushToast(
          ok ? 'Entité supprimée (Ctrl+Z pour annuler).' : 'Suppression refusée : joueur protégé.',
          ok ? 'info' : 'error'
        );
      });
  }, [refresh, pushToast]);

  const handleTool = useCallback((tool: 'select' | 'brush' | 'eraser') => {
    void setActiveTool(tool).then(() => refresh());
  }, [refresh]);

  const handleTile = useCallback((tileId: string) => {
    void setBrushTile(tileId).then(() => refresh());
  }, [refresh]);

  const handleUndo = useCallback(() => { void undo().then(() => refresh()); }, [refresh]);
  const handleRedo = useCallback(() => { void redo().then(() => refresh()); }, [refresh]);

  const handleSave = useCallback(() => {
    void saveScene().then(ok => {
      pushToast(
        ok ? 'Scène ECS et scripts KVL enregistrés.' : "Échec de l'enregistrement sur disque.",
        ok ? 'success' : 'error'
      );
    });
  }, [pushToast]);

  const handleSpawnAtCenter = useCallback((preset: string) => {
    void spawnEntityAtScreen(preset, 600, 400).then(() => refresh());
  }, [refresh]);

  const handleSpawn3d = useCallback((preset: string) => {
    void spawn3dPreset(preset, 0, 1, 0).then(() => {
      refresh();
      setView3d(true);
    });
  }, [refresh]);

  const handleSpawnAtScreen = useCallback((preset: string, sx: number, sy: number) => {
    void spawnEntityAtScreen(preset, sx, sy).then(() => refresh());
  }, [refresh]);

  // Lancement d'un des 5 jeux de référence (Archétypes) en 1 clic
  const handleLaunchArchetype = async (archetypeId: string) => {
    try {
      const sceneName = await launchArchetypeGame(archetypeId);
      setIsHubOpen(false);
      setIsArchetypesOpen(false);
      setView3d(false);
      setCurrentMode('play');
      setMetrics(prev => ({
        ...prev,
        status: `🎮 ${archetypeId.toUpperCase()} — Jeu par défaut lancé : '${sceneName}' • Prêt à jouer !`
      }));
    } catch (err: any) {
      const message = err?.message ?? String(err);
      console.error(`[KaractiRage] Échec du lancement de l'archétype '${archetypeId}' :`, err);
      pushToast(`Échec du lancement de l'archétype '${archetypeId}' : ${message}`, 'error');
    }
  };

  // Lancement de la démo 3D thématique d'un archétype (scène procédurale dédiée).
  const handleLaunchArchetype3D = async (archetypeId: string) => {
    try {
      await launchArchetypeGame3D(archetypeId);
      setIsHubOpen(false);
      setIsArchetypesOpen(false);
      setCurrentMode('play');
      setView3d(true);
      setMetrics(prev => ({
        ...prev,
        status: `🎮 ${archetypeId.toUpperCase()} — Démo 3D lancée • Vue RenderPacket`,
      }));
    } catch (err: any) {
      const message = err?.message ?? String(err);
      console.error(`[KaractiRage] Échec du lancement 3D de l'archétype '${archetypeId}' :`, err);
      pushToast(`Échec du lancement 3D de l'archétype '${archetypeId}' : ${message}`, 'error');
    }
  };

  // Ouverture d'un projet/scène depuis le Hub (registre disque)
  const handleProjectOpened = (entry: ProjectEntry, mode: 'play' | 'edit') => {
    setIsHubOpen(false);
    setView3d(false);
    setCurrentMode(mode);
    void setEditorMode(mode);
    setMetrics(prev => ({
      ...prev,
      status: `Projet '${entry.title}' ouvert • Mode ${mode === 'edit' ? 'Studio' : 'Jeu'} 60 FPS`
    }));
  };

  // Application du Payload généré par l'IA dans la scène native
  const handleApplyGeneratedLevel = async (payloadJson: string) => {
    try {
      const parsed = JSON.parse(payloadJson);
      const code = await applyAiPayload(payloadJson, parsed.title || "Intention IA");
      if (code === 0) {
        setPendingAiPatch(parsed.title || "Patch IA");
        setIsAiModalOpen(false);
        setCurrentMode('play');
        setMetrics(prev => ({
          ...prev,
          status: `✨ Niveau IA '${parsed.title}' injecté dans le Viewport (Examinez le halo violet)`
        }));
      } else {
        pushToast(`Erreur d'application du payload IA (code ${code}).`, 'error');
      }
    } catch (err: any) {
      pushToast(`JSON de niveau invalide : ${err?.message}`, 'error');
    }
  };

  // Export Standalone HTML5 Universel (Commandement 9)
  const handleExportStandalone = () => {
    void downloadStandaloneGame("MonJeuKaractiRage", "Créateur KaractiRage");
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
        onModeChange={applyMode}
        onOpenHub={() => setIsHubOpen(true)}
        onOpenAiAssistant={() => setIsAiModalOpen(true)}
        onExportStandalone={handleExportStandalone}
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
        {/* Panneau Gauche : Palette + Hiérarchie (Visible en mode Studio) */}
        {currentMode === 'edit' && (
          <div className="flex flex-col w-64 h-full border-r border-[#2d1b4e]">
            <SpawnPalette onSpawnAtCenter={handleSpawnAtCenter} onSpawn3d={handleSpawn3d} />
            <EntityHierarchy
              entities={entities}
              selectedEntityId={selectedEntityId}
              onSelectEntity={handleSelectEntity}
            />
            <SuggestionPanel
              entities={entities}
              selectedEntityId={selectedEntityId}
              onSpawn={handleSpawnAtCenter}
              onOpenKvl={() => applyMode('kvl')}
              onTestGame={() => applyMode('play')}
            />
          </div>
        )}

        {/* Zone Centrale : Viewport 2D, Éditeur Nodal KVL ou Atelier Pixel Art */}
        <main className="flex-1 flex flex-col h-full overflow-hidden relative">
          {currentMode === 'kvl' ? (
            <KvlGraphEditor />
          ) : currentMode === 'pixel' ? (
            <PxlArtWorkspace />
          ) : currentMode === 'vector' ? (
            <VectorWorkspace />
          ) : (
            <>
              {currentMode === 'edit' && (
                <StudioToolbar
                  tool={editorState?.tool ?? 'select'}
                  tile={editorState?.tile ?? 'wall'}
                  canUndo={editorState?.can_undo ?? false}
                  canRedo={editorState?.can_redo ?? false}
                  onSelectTool={handleTool}
                  onSelectTile={handleTile}
                  onUndo={handleUndo}
                  onRedo={handleRedo}
                  onSave={handleSave}
                />
              )}
              <GameViewport
                onTelemetryUpdate={setMetrics}
                onSceneChanged={refresh}
                onSpawnAtScreen={handleSpawnAtScreen}
                isEditMode={currentMode === 'edit'}
                view3d={view3d}
                onView3dChange={setView3d}
              />
            </>
          )}
        </main>

        {/* Panneau Droit : Inspecteur (Visible en mode Studio) */}
        {currentMode === 'edit' && (
          <EntityInspector
            selectedEntity={selectedEntity}
            onUpdateTransform={handleUpdateTransform}
            onDuplicate={handleDuplicate}
            onDelete={handleDelete}
          />
        )}
      </div>

      {/* 4. Barre de Statut Inférieure */}
      <footer className="flex items-center justify-between gap-3 px-4 py-1.5 bg-[#0d0819] border-t border-[#2d1b4e] text-xs text-[#94a3b8] shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <span className="w-2 h-2 rounded-full bg-[#34d399] animate-pulse shrink-0" />
          <span className="font-medium text-[#cbd5e1] truncate">{metrics.status}</span>
        </div>
        <div className="flex items-center gap-3 shrink-0">
          <button
            onClick={() => setIsHelpOpen(true)}
            className="flex items-center gap-1 px-2 py-1 rounded-md text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] transition"
            title="Aide clavier (raccourcis)"
            aria-label="Aide clavier (raccourcis)"
          >
            <HelpCircle size={13} />
            Aide clavier
          </button>
          <span className="text-[#a855f7] font-semibold">KaractiRage Studio v0.2.0</span>
        </div>
      </footer>

      {/* 4bis. Aide clavier (popover) */}
      {isHelpOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm select-none"
          onClick={() => setIsHelpOpen(false)}
        >
          <div
            className="w-full max-w-md bg-[#0d0819] border border-[#2d1b4e] rounded-xl shadow-2xl shadow-purple-950/90 overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between px-4 py-3 bg-[#150d27] border-b border-[#2d1b4e]">
              <h2 className="font-bold text-sm text-[#f8fafc]">Aide clavier</h2>
              <button onClick={() => setIsHelpOpen(false)} className="p-1 text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] rounded" aria-label="Fermer">
                <X size={16} />
              </button>
            </div>
            <div className="p-4 text-xs text-[#94a3b8] font-mono space-y-1.5">
              <div className="flex justify-between"><span>Bascule Jouer ↔ Studio</span><kbd>Tab</kbd></div>
              <div className="flex justify-between"><span>Changer de mode</span><kbd>Ctrl+1…5</kbd></div>
              <div className="flex justify-between"><span>Mode suivant</span><kbd>F6</kbd></div>
              <div className="flex justify-between"><span>Hub / IA / Recette</span><kbd>H / I / W</kbd></div>
              <div className="flex justify-between"><span>Outils sélection / pinceau / gomme</span><kbd>V / B / X</kbd></div>
              <div className="flex justify-between"><span>Cycle tuile</span><kbd>T</kbd></div>
              <div className="flex justify-between"><span>Enregistrer / annuler / rétablir</span><kbd>Ctrl+S / Z / Y</kbd></div>
              <div className="flex justify-between"><span>Effacer la sélection</span><kbd>Suppr</kbd></div>
              <div className="flex justify-between"><span>Saut / interaction (jeu)</span><kbd>Espace / E</kbd></div>
              <div className="flex justify-between"><span>Accepter / rejeter un patch IA</span><kbd>Entrée / Échap</kbd></div>
            </div>
          </div>
        </div>
      )}

      {/* 5. Modales Flottantes */}
      <ProjectHubModal
        isOpen={isHubOpen}
        onClose={() => setIsHubOpen(false)}
        onOpenArchetypes={() => {
          setIsHubOpen(false);
          setIsArchetypesOpen(true);
        }}
        onOpenWizard={() => {
          setIsHubOpen(false);
          setIsWizardOpen(true);
        }}
        onOpenAiTab={() => {
          setIsHubOpen(false);
          setIsAiModalOpen(true);
        }}
        onProjectOpened={handleProjectOpened}
      />

      <ArchetypesModal
        isOpen={isArchetypesOpen}
        onClose={() => setIsArchetypesOpen(false)}
        onLaunchArchetype={handleLaunchArchetype}
        onLaunchArchetype3D={handleLaunchArchetype3D}
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

      {/* 6. Notifications non bloquantes */}
      <ToastCenter toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}

export default App;
