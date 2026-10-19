import React, { useState, useEffect } from 'react';
import {
  Sparkles, X, ChevronRight, ChevronLeft, Check, ShieldCheck,
  Gamepad2, Layers, Move, Globe2, Trophy, Cpu, Play,
  AlertTriangle
} from 'lucide-react';
import { validateRecipe, applyRecipe } from '../services/engineBridge';

interface RecipeWizardModalProps {
  isOpen: boolean;
  onClose: () => void;
  onProjectCreated: (title: string) => void;
}

const GENRES = [
  { id: 'adventure', label: 'Aventure', icon: '🗺️', desc: 'Exploration, découverte et mystères' },
  { id: 'action', label: 'Action', icon: '⚔️', desc: 'Réflexes, esquives et dynamisme' },
  { id: 'platformer', label: 'Plateforme', icon: '👟', desc: 'Gravité, sauts d’obstacles et corniches' },
  { id: 'rpg', label: 'RPG / Rôle', icon: '🛡️', desc: 'Statistiques, progression et équipement' },
  { id: 'shoot_em_up', label: "Shoot'em up", icon: '🚀', desc: 'Tirs balistiques, esquive de projectiles' },
  { id: 'puzzle', label: 'Puzzle & Énigmes', icon: '🧩', desc: 'Réflexion logique, blocs et interrupteurs' },
  { id: 'combat', label: 'Combat / Arène', icon: '🥊', desc: 'Duels au corps à corps, boss épiques' },
  { id: 'strategy', label: 'Stratégie', icon: '♟️', desc: 'Planification tactique et gestion de terrain' },
  { id: 'horror', label: 'Horreur & Survie', icon: '🕯️', desc: 'Tension, ressources rares et embuscades' },
  { id: 'simulation', label: 'Simulation', icon: '🏗️', desc: 'Systèmes émergents et modélisation' },
  { id: 'racing', label: 'Course', icon: '🏁', desc: 'Vitesse pure, trajectoires et chrono' },
  { id: 'educational', label: 'Éducatif', icon: '🎓', desc: 'Apprentissage ludique et vulgarisation' },
];

export const RecipeWizardModal: React.FC<RecipeWizardModalProps> = ({
  isOpen,
  onClose,
  onProjectCreated
}) => {
  const [step, setStep] = useState(1);
  const [title, setTitle] = useState("Mon Odyssée KaractiRage");
  const [author, setAuthor] = useState("Créateur Studio");

  // Étape 1 : Genres
  const [primaryGenre, setPrimaryGenre] = useState<string>('platformer');
  const [secondaryGenres, setSecondaryGenres] = useState<string[]>(['action']);

  // Étape 2 : Univers & Caméra
  const [dimension, setDimension] = useState<'two_d' | 'two_and_half_d' | 'three_d'>('two_d');
  const [camera, setCamera] = useState<'follow_player' | 'fixed' | 'auto_scroll_horizontal' | 'auto_scroll_vertical' | 'top_down_free'>('follow_player');

  // Étape 3 : Déplacement & Combat
  const [directions, setDirections] = useState<'horizontal_only' | 'four_directions' | 'eight_directions' | 'free_inertia' | 'grid'>('horizontal_only');
  const [abilities, setAbilities] = useState({
    jump: true,
    double_jump: false,
    run: true,
    crouch: true,
    dash: true,
    interact: true
  });
  const [combat, setCombat] = useState<'none' | 'melee' | 'projectiles' | 'special_abilities' | 'hybrid'>('melee');

  // Étape 4 : Monde & Éléments
  const [gravity, setGravity] = useState(true);
  const [interactiveElements, setInteractiveElements] = useState<string[]>(['collectible', 'door', 'trap', 'moving_platform']);

  // Étape 5 : Gameplay & Expérience
  const [gameplay, setGameplay] = useState({
    inventory: false,
    dialogues: false,
    quest_objectives: true,
    save_system: true,
    score_system: true,
    boss_fight: false,
    wave_spawner: false
  });

  // Étape 6 : Profil Matériel
  const [performance, setPerformance] = useState<'low_end' | 'balanced' | 'high_end' | 'mobile'>('low_end');

  // Rapport du résolveur de dépendances
  const [validationReport, setValidationReport] = useState<any>({ is_valid: true, resolved_dependencies: [], warnings: [], conflicts: [] });

  // Construction de l'objet recette
  const currentRecipe = {
    schema_version: 1,
    title,
    author,
    genres: { primary: primaryGenre, secondary: secondaryGenres },
    dimension,
    camera,
    player: {
      name: "Héros",
      directions,
      abilities: {
        jump: abilities.jump,
        double_jump: abilities.double_jump,
        run: abilities.run,
        crouch: abilities.crouch,
        dash: abilities.dash,
        climb: false,
        swim: false,
        teleport: false,
        interact: abilities.interact
      },
      combat,
      max_health: combat === 'none' ? 1 : 5,
      speed: directions === 'grid' ? 140 : 200
    },
    world: {
      gravity_enabled: gravity,
      day_night_cycle: false,
      dynamic_lighting: false,
      interactive_elements: interactiveElements
    },
    gameplay,
    performance
  };

  // Validation dynamique à chaque modification de paramètre
  useEffect(() => {
    if (!isOpen) return;
    try {
      const rep = validateRecipe(JSON.stringify(currentRecipe));
      setValidationReport(rep);
    } catch (e) {
      // Ignorer si le moteur n'est pas prêt
    }
  }, [
    isOpen, primaryGenre, secondaryGenres, dimension, camera, directions,
    abilities, combat, gravity, interactiveElements, gameplay, performance
  ]);

  if (!isOpen) return null;

  const toggleSecondaryGenre = (gId: string) => {
    if (gId === primaryGenre) return;
    setSecondaryGenres(prev =>
      prev.includes(gId) ? prev.filter(x => x !== gId) : [...prev, gId]
    );
  };

  const toggleInteractiveElement = (elem: string) => {
    setInteractiveElements(prev =>
      prev.includes(elem) ? prev.filter(x => x !== elem) : [...prev, elem]
    );
  };

  const handleCreateProject = () => {
    const jsonStr = JSON.stringify(currentRecipe);
    const res = applyRecipe(jsonStr);
    if (res === 0) {
      onProjectCreated(title);
      onClose();
    } else {
      alert(`Erreur lors de l'instanciation de la recette (code ${res}). Vérifiez les conflits.`);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md select-none font-sans">
      <div className="relative flex flex-col w-full max-w-4xl max-h-[92vh] bg-[#0d0819] border border-[#a855f7] rounded-xl shadow-2xl shadow-purple-950/90 overflow-hidden">
        {/* En-tête de l'Assistant */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#150d27] border-b border-[#2d1b4e]">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-8 h-8 rounded bg-[#2e1065] border border-[#a855f7] text-[#c084fc]">
              <Sparkles size={18} />
            </div>
            <div>
              <h2 className="text-base font-bold text-[#f8fafc] tracking-wide">
                ASSISTANT DE CRÉATION GUIDÉ (6 ÉTAPES)
              </h2>
              <p className="text-xs text-[#94a3b8]">
                Définition de la recette déclarative universelle • Résolveur de dépendances automatique
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

        {/* Fil d'Ariane des 6 Étapes */}
        <div className="flex items-center px-6 py-2.5 bg-[#0a0514] border-b border-[#2d1b4e] overflow-x-auto gap-2">
          {[
            { num: 1, label: '1. Genres' },
            { num: 2, label: '2. Univers' },
            { num: 3, label: '3. Jouabilité' },
            { num: 4, label: '4. Monde' },
            { num: 5, label: '5. Objectifs' },
            { num: 6, label: '6. Matériel & Synthèse' },
          ].map(s => (
            <button
              key={s.num}
              onClick={() => setStep(s.num)}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-bold transition whitespace-nowrap ${
                step === s.num
                  ? 'bg-[#3b0764] text-[#f3e8ff] border border-[#a855f7]'
                  : step > s.num
                  ? 'bg-[#150d27] text-[#34d399] border border-[#059669]/40'
                  : 'bg-[#150d27] text-[#64748b] border border-[#2d1b4e]'
              }`}
            >
              {step > s.num ? <Check size={12} /> : <span>{s.num}.</span>}
              {s.label.replace(/^\d+\.\s*/, '')}
            </button>
          ))}
        </div>

        {/* Corps de l'Étape Active */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {/* ÉTAPE 1 : GENRES */}
          {step === 1 && (
            <div className="space-y-4">
              <div>
                <h3 className="text-sm font-bold text-[#f8fafc] flex items-center gap-2">
                  <Gamepad2 size={16} className="text-[#a855f7]" />
                  Étape 1 — Que veux-tu créer ? (Genre principal & genres combinables)
                </h3>
                <p className="text-xs text-[#94a3b8]">
                  Sélectionnez un genre principal d'ancrage, puis enrichissez le gameplay avec des genres secondaires.
                </p>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-3">
                {GENRES.map(g => {
                  const isPrimary = primaryGenre === g.id;
                  const isSecondary = secondaryGenres.includes(g.id);
                  return (
                    <div
                      key={g.id}
                      onClick={() => setPrimaryGenre(g.id)}
                      className={`p-3.5 rounded-lg border cursor-pointer transition ${
                        isPrimary
                          ? 'bg-[#2e1065] border-[#a855f7] shadow-lg shadow-purple-950/50'
                          : isSecondary
                          ? 'bg-[#150d27] border-[#06b6d4] text-[#e0f2fe]'
                          : 'bg-[#150d27] border-[#2d1b4e] hover:border-[#64748b]'
                      }`}
                    >
                      <div className="flex items-center justify-between mb-1">
                        <span className="text-base mr-2">{g.icon}</span>
                        <span className="font-bold text-xs text-[#f8fafc] flex-1">{g.label}</span>
                        {isPrimary && (
                          <span className="text-[10px] uppercase font-extrabold px-1.5 py-0.5 rounded bg-[#7e22ce] text-white">
                            Principal
                          </span>
                        )}
                        {!isPrimary && isSecondary && (
                          <span className="text-[10px] uppercase font-extrabold px-1.5 py-0.5 rounded bg-[#0891b2] text-white">
                            Secondaire
                          </span>
                        )}
                      </div>
                      <p className="text-[11px] text-[#94a3b8] leading-relaxed mb-2">{g.desc}</p>
                      {!isPrimary && (
                        <button
                          type="button"
                          onClick={e => {
                            e.stopPropagation();
                            toggleSecondaryGenre(g.id);
                          }}
                          className={`text-[10px] font-semibold px-2 py-0.5 rounded border transition ${
                            isSecondary
                              ? 'bg-[#0e7490] border-[#06b6d4] text-white'
                              : 'bg-[#1e1338] border-[#2d1b4e] text-[#94a3b8] hover:text-[#f8fafc]'
                          }`}
                        >
                          {isSecondary ? '✓ Actif en secondaire' : '+ Ajouter en secondaire'}
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* ÉTAPE 2 : UNIVERS & CAMÉRA */}
          {step === 2 && (
            <div className="space-y-5">
              <div>
                <h3 className="text-sm font-bold text-[#f8fafc] flex items-center gap-2">
                  <Layers size={16} className="text-[#38bdf8]" />
                  Étape 2 — Dans quel univers et quelle dimension ?
                </h3>
                <p className="text-xs text-[#94a3b8]">
                  Le moteur configure automatiquement la projection et le comportement de la caméra.
                </p>
              </div>

              <div>
                <label className="block text-xs font-bold text-[#94a3b8] uppercase mb-2">Dimension :</label>
                <div className="grid grid-cols-3 gap-3">
                  {[
                    { id: 'two_d', label: '2D Pure', desc: 'Rendu logiciel pixel-art 60 FPS sans surcoût GPU' },
                    { id: 'two_and_half_d', label: '2.5D Isométrique', desc: 'Monde avec profondeur et plans étagés' },
                    { id: 'three_d', label: '3D Stylisée', desc: 'Caméra libre et maillages low-poly glTF' },
                  ].map(d => (
                    <div
                      key={d.id}
                      onClick={() => setDimension(d.id as any)}
                      className={`p-3.5 rounded-lg border cursor-pointer transition ${
                        dimension === d.id
                          ? 'bg-[#2e1065] border-[#a855f7]'
                          : 'bg-[#150d27] border-[#2d1b4e] hover:border-[#64748b]'
                      }`}
                    >
                      <h4 className="font-bold text-xs text-[#f8fafc] mb-1">{d.label}</h4>
                      <p className="text-[11px] text-[#94a3b8]">{d.desc}</p>
                    </div>
                  ))}
                </div>
              </div>

              <div>
                <label className="block text-xs font-bold text-[#94a3b8] uppercase mb-2">Comportement Caméra :</label>
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                  {[
                    { id: 'follow_player', label: 'Suivi Fluide Centré', desc: 'Deadzone douce et recentrage continu' },
                    { id: 'fixed', label: 'Fixe par Écran / Salle', desc: 'Transition franche par pièce' },
                    { id: 'auto_scroll_vertical', label: 'Défilement Auto (Vertical)', desc: 'Idéal Shoot’em up spatial' },
                    { id: 'auto_scroll_horizontal', label: 'Défilement Auto (Horizontal)', desc: 'Idéal Runner cinétique' },
                    { id: 'top_down_free', label: 'Vue de Dessus Libre', desc: 'Exploration libre type Zelda 2D' },
                  ].map(c => (
                    <div
                      key={c.id}
                      onClick={() => setCamera(c.id as any)}
                      className={`p-3 rounded-lg border cursor-pointer transition ${
                        camera === c.id
                          ? 'bg-[#1e1b4b] border-[#6366f1]'
                          : 'bg-[#150d27] border-[#2d1b4e] hover:border-[#64748b]'
                      }`}
                    >
                      <h4 className="font-bold text-xs text-[#f8fafc] mb-1">{c.label}</h4>
                      <p className="text-[11px] text-[#94a3b8]">{c.desc}</p>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* ÉTAPE 3 : JOUABILITÉ & COMBAT */}
          {step === 3 && (
            <div className="space-y-5">
              <div>
                <h3 className="text-sm font-bold text-[#f8fafc] flex items-center gap-2">
                  <Move size={16} className="text-[#34d399]" />
                  Étape 3 — Comment joue-t-on ? (Déplacement & Capacités)
                </h3>
                <p className="text-xs text-[#94a3b8]">
                  Définissez la dynamique motrice du joueur et son style d'action.
                </p>
              </div>

              <div>
                <label className="block text-xs font-bold text-[#94a3b8] uppercase mb-2">Direction du déplacement :</label>
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                  {[
                    { id: 'horizontal_only', label: 'Gauche & Droite seul', desc: 'Classique Plateforme 2D' },
                    { id: 'eight_directions', label: '8 Directions libres', desc: 'Exploration fluide haut/bas/diagonales' },
                    { id: 'free_inertia', label: 'Inertie Continue Spatiale', desc: 'Propulsion vectorielle et frottement' },
                    { id: 'grid', label: 'Grille Case par Case (Sokoban)', desc: 'Déplacement quantifié par pas de tuile' },
                    { id: 'four_directions', label: '4 Directions cardinales', desc: 'Navigation orthogonale stricte' },
                  ].map(d => (
                    <div
                      key={d.id}
                      onClick={() => setDirections(d.id as any)}
                      className={`p-3 rounded-lg border cursor-pointer transition ${
                        directions === d.id
                          ? 'bg-[#064e3b] border-[#10b981]'
                          : 'bg-[#150d27] border-[#2d1b4e] hover:border-[#64748b]'
                      }`}
                    >
                      <h4 className="font-bold text-xs text-[#f8fafc] mb-1">{d.label}</h4>
                      <p className="text-[11px] text-[#94a3b8]">{d.desc}</p>
                    </div>
                  ))}
                </div>
              </div>

              <div>
                <label className="block text-xs font-bold text-[#94a3b8] uppercase mb-2">Capacités physiques :</label>
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
                  {[
                    { key: 'jump', label: 'Saut d’impulsion' },
                    { key: 'double_jump', label: 'Double Saut' },
                    { key: 'dash', label: 'Dash / Esquive' },
                    { key: 'crouch', label: 'S’accroupir' },
                    { key: 'run', label: 'Course rapide' },
                    { key: 'interact', label: 'Interaction contextuelle' },
                  ].map(ab => (
                    <label
                      key={ab.key}
                      className={`flex items-center gap-2 p-2.5 rounded border cursor-pointer transition ${
                        (abilities as any)[ab.key]
                          ? 'bg-[#2e1065] border-[#a855f7] text-[#f8fafc]'
                          : 'bg-[#150d27] border-[#2d1b4e] text-[#94a3b8]'
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={(abilities as any)[ab.key]}
                        onChange={e => setAbilities(prev => ({ ...prev, [ab.key]: e.target.checked }))}
                        className="rounded accent-[#a855f7]"
                      />
                      <span className="text-xs font-medium">{ab.label}</span>
                    </label>
                  ))}
                </div>
              </div>

              <div>
                <label className="block text-xs font-bold text-[#94a3b8] uppercase mb-2">Style d'Affrontement :</label>
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
                  {[
                    { id: 'melee', label: 'Corps à corps (Épée/Mêlée)' },
                    { id: 'projectiles', label: 'Tirs de Projectiles (Lasers)' },
                    { id: 'hybrid', label: 'Hybride (Mêlée & Tir)' },
                    { id: 'none', label: 'Pacifique (Aucun combat)' },
                  ].map(c => (
                    <div
                      key={c.id}
                      onClick={() => setCombat(c.id as any)}
                      className={`p-2.5 rounded border cursor-pointer text-xs font-medium text-center transition ${
                        combat === c.id
                          ? 'bg-[#7e22ce] text-white border-[#a855f7]'
                          : 'bg-[#150d27] text-[#94a3b8] border-[#2d1b4e] hover:bg-[#1e1338]'
                      }`}
                    >
                      {c.label}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* ÉTAPE 4 : LE MONDE & INTERACTIVITÉ */}
          {step === 4 && (
            <div className="space-y-5">
              <div>
                <h3 className="text-sm font-bold text-[#f8fafc] flex items-center gap-2">
                  <Globe2 size={16} className="text-[#f59e0b]" />
                  Étape 4 — Le Monde & Éléments Interactifs
                </h3>
                <p className="text-xs text-[#94a3b8]">
                  Structure environnementale, physique gravitationnelle et mécanismes de puzzle.
                </p>
              </div>

              <div className="p-3.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg flex items-center justify-between">
                <div>
                  <h4 className="font-bold text-xs text-[#f8fafc]">Gravité Vectorielle Activable</h4>
                  <p className="text-[11px] text-[#94a3b8]">
                    Applique une accélération vers le bas (+900 px/s²) pour la physique de plateforme.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => setGravity(prev => !prev)}
                  className={`px-3 py-1.5 rounded text-xs font-bold transition ${
                    gravity ? 'bg-[#10b981] text-black' : 'bg-[#1e1338] text-[#94a3b8] border border-[#2d1b4e]'
                  }`}
                >
                  {gravity ? 'GRAVITÉ ACTIVE' : 'SANS GRAVITÉ'}
                </button>
              </div>

              <div>
                <label className="block text-xs font-bold text-[#94a3b8] uppercase mb-2">Composants du Monde :</label>
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
                  {[
                    { id: 'door', label: 'Portes & Clés de Donjon' },
                    { id: 'chest', label: 'Coffres au Trésor Scellés' },
                    { id: 'lever', label: 'Leviers & Interrupteurs' },
                    { id: 'pressure_plate', label: 'Plaques de Pression au Sol' },
                    { id: 'trap', label: 'Pièges à Pointes & Lave' },
                    { id: 'moving_platform', label: 'Plateformes Mobiles Oscillantes' },
                    { id: 'breakable_wall', label: 'Murs Fracturables' },
                    { id: 'collectible', label: 'Pièces & Gemmes de Score' },
                    { id: 'checkpoint', label: 'Points de Sauvegarde' },
                  ].map(elem => {
                    const active = interactiveElements.includes(elem.id);
                    return (
                      <div
                        key={elem.id}
                        onClick={() => toggleInteractiveElement(elem.id)}
                        className={`p-3 rounded-lg border cursor-pointer transition flex items-center justify-between ${
                          active
                            ? 'bg-[#2e1065] border-[#a855f7] text-[#f8fafc]'
                            : 'bg-[#150d27] border-[#2d1b4e] text-[#64748b]'
                        }`}
                      >
                        <span className="text-xs font-medium">{elem.label}</span>
                        {active && <Check size={14} className="text-[#34d399]" />}
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          )}

          {/* ÉTAPE 5 : EXPÉRIENCE & OBJECTIFS */}
          {step === 5 && (
            <div className="space-y-5">
              <div>
                <h3 className="text-sm font-bold text-[#f8fafc] flex items-center gap-2">
                  <Trophy size={16} className="text-[#ec4899]" />
                  Étape 5 — L'Expérience & les Objectifs de Jeu
                </h3>
                <p className="text-xs text-[#94a3b8]">
                  Modules de narration, quêtes, inventaire et combats épiques.
                </p>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {[
                  { key: 'inventory', title: 'Inventaire & Hotbar (5 Slots)', desc: 'Raccourcis 1 à 5 pour objets et clés' },
                  { key: 'dialogues', title: 'Dialogues PNJ & Choix Multiples', desc: 'Effet machine à écrire rythmé (38 car/s)' },
                  { key: 'quest_objectives', title: 'Arbre de Quêtes & Objectifs HUD', desc: 'Bandeau d’objectif narratif en haut à droite' },
                  { key: 'score_system', title: 'Système de Score & Récompenses', desc: 'Calcul de points et jingles triomphaux KVL' },
                  { key: 'boss_fight', title: 'Combat de Boss à Phases (Titan)', desc: 'Barre de vie géante, enrage et télégraphie' },
                  { key: 'wave_spawner', title: 'Générateur de Vagues d’Ennemis', desc: 'Apparitions cadencées de créatures A*' },
                ].map(feat => {
                  const val = (gameplay as any)[feat.key];
                  return (
                    <div
                      key={feat.key}
                      onClick={() => setGameplay(prev => ({ ...prev, [feat.key]: !val }))}
                      className={`p-3.5 rounded-lg border cursor-pointer transition flex items-start justify-between ${
                        val
                          ? 'bg-[#2e1065] border-[#a855f7]'
                          : 'bg-[#150d27] border-[#2d1b4e]'
                      }`}
                    >
                      <div>
                        <h4 className="font-bold text-xs text-[#f8fafc] mb-1">{feat.title}</h4>
                        <p className="text-[11px] text-[#94a3b8]">{feat.desc}</p>
                      </div>
                      <div className={`w-5 h-5 rounded flex items-center justify-center border ${
                        val ? 'bg-[#10b981] border-[#059669] text-black font-bold' : 'border-[#475569]'
                      }`}>
                        {val && <Check size={12} />}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* ÉTAPE 6 : PROFIL MATÉRIEL & SYNTHÈSE */}
          {step === 6 && (
            <div className="space-y-5">
              <div>
                <h3 className="text-sm font-bold text-[#f8fafc] flex items-center gap-2">
                  <Cpu size={16} className="text-[#a855f7]" />
                  Étape 6 — Profil Matériel & Validation des Dépendances
                </h3>
                <p className="text-xs text-[#94a3b8]">
                  Garantie 60 FPS constants et vérification automatique des 6 règles de cohérence.
                </p>
              </div>

              <div>
                <label className="block text-xs font-bold text-[#94a3b8] uppercase mb-2">Profil Matériel Cible :</label>
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
                  {[
                    { id: 'low_end', label: 'Modeste (Low-End 60 FPS)', desc: 'Zéro alloc, budget < 16.6ms' },
                    { id: 'balanced', label: 'Équilibré (PC Standard)', desc: 'Particules complètes' },
                    { id: 'high_end', label: 'Élevé (GPU Dédié)', desc: 'Effets dynamiques' },
                    { id: 'mobile', label: 'Mobile & Batterie', desc: 'Économie d’énergie' },
                  ].map(p => (
                    <div
                      key={p.id}
                      onClick={() => setPerformance(p.id as any)}
                      className={`p-3 rounded-lg border cursor-pointer transition ${
                        performance === p.id
                          ? 'bg-[#2e1065] border-[#a855f7]'
                          : 'bg-[#150d27] border-[#2d1b4e]'
                      }`}
                    >
                      <h4 className="font-bold text-xs text-[#f8fafc] mb-1">{p.label}</h4>
                      <p className="text-[10px] text-[#94a3b8]">{p.desc}</p>
                    </div>
                  ))}
                </div>
              </div>

              {/* Rapport de Résolution des Dépendances */}
              <div className="p-4 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold uppercase tracking-wider text-[#38bdf8] flex items-center gap-1.5">
                    <ShieldCheck size={16} />
                    Résolveur de Dépendances Automatique (Règles 1 à 6)
                  </span>
                  <span className={`text-[11px] font-bold px-2 py-0.5 rounded ${
                    validationReport.is_valid ? 'bg-[#064e3b] text-[#34d399]' : 'bg-[#881337] text-[#f43f5e]'
                  }`}>
                    {validationReport.is_valid ? 'Recette Valide & Prête' : 'Conflit Détecté'}
                  </span>
                </div>

                {validationReport.resolved_dependencies?.length > 0 && (
                  <div className="space-y-1">
                    <span className="text-[11px] font-semibold text-[#94a3b8]">Dépendances injectées automatiquement :</span>
                    <ul className="text-xs space-y-1 text-[#cbd5e1]">
                      {validationReport.resolved_dependencies.map((dep: string, i: number) => (
                        <li key={i} className="flex items-center gap-2">
                          <span className="w-1.5 h-1.5 rounded-full bg-[#10b981]" />
                          {dep}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}

                {validationReport.conflicts?.length > 0 && (
                  <div className="p-2.5 rounded bg-[#881337]/30 border border-[#e11d48] text-xs text-[#f43f5e] space-y-1">
                    <span className="font-bold flex items-center gap-1.5">
                      <AlertTriangle size={14} />
                      Conflits à résoudre :
                    </span>
                    {validationReport.conflicts.map((c: string, i: number) => (
                      <p key={i}>• {c}</p>
                    ))}
                  </div>
                )}
              </div>

              {/* Titre et Auteur du Projet */}
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-2">
                <div>
                  <label className="block text-xs font-bold text-[#94a3b8] mb-1">Nom du Jeu / Projet :</label>
                  <input
                    type="text"
                    value={title}
                    onChange={e => setTitle(e.target.value)}
                    className="w-full text-xs font-bold text-[#f8fafc] bg-[#07040d] border border-[#2d1b4e] rounded px-3 py-2 focus:border-[#a855f7]"
                  />
                </div>
                <div>
                  <label className="block text-xs font-bold text-[#94a3b8] mb-1">Auteur & Studio :</label>
                  <input
                    type="text"
                    value={author}
                    onChange={e => setAuthor(e.target.value)}
                    className="w-full text-xs font-bold text-[#f8fafc] bg-[#07040d] border border-[#2d1b4e] rounded px-3 py-2 focus:border-[#a855f7]"
                  />
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Pied de Page avec Navigation Précédent / Suivant / Créer */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#150d27] border-t border-[#2d1b4e]">
          <button
            type="button"
            disabled={step === 1}
            onClick={() => setStep(prev => Math.max(1, prev - 1))}
            className="flex items-center gap-1.5 px-4 py-2 bg-[#1e1338] hover:bg-[#2d1b4e] disabled:opacity-30 text-[#f8fafc] rounded-lg text-xs font-medium transition"
          >
            <ChevronLeft size={14} />
            Précédent
          </button>

          <div className="flex items-center gap-3">
            <span className="text-xs text-[#64748b]">Étape {step} sur 6</span>
            {step < 6 ? (
              <button
                type="button"
                onClick={() => setStep(prev => Math.min(6, prev + 1))}
                className="flex items-center gap-1.5 px-5 py-2 bg-[#7e22ce] hover:bg-[#9333ea] text-white rounded-lg text-xs font-bold shadow-lg shadow-purple-950/60 transition"
              >
                Suivant
                <ChevronRight size={14} />
              </button>
            ) : (
              <button
                type="button"
                onClick={handleCreateProject}
                disabled={!validationReport.is_valid}
                className="flex items-center gap-2 px-6 py-2 bg-gradient-to-r from-[#10b981] to-[#059669] hover:from-[#34d399] hover:to-[#10b981] text-black font-extrabold text-xs rounded-lg shadow-lg shadow-emerald-950/60 transition"
              >
                <Play size={14} className="fill-black" />
                Créer & Jouer Immédiatement
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
