import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle, Archive, ArrowLeft, ArrowRight, Bot, Box, Check, ChevronDown,
  Compass, Copy, Cpu, FolderOpen, FolderSearch, Gamepad2, Layers, Loader2,
  Pencil, Play, Plus, RefreshCw, Search, Sparkles, Upload, Wrench, X, Zap,
} from 'lucide-react';
import {
  archiveProject, createProject, duplicateProject, isTauri, listProjects, openProject,
  removeRecent, renameProject, revealProject, systemDiagnostics,
  type NewProjectConfig, type ProjectEntry, type SystemDiagnostics,
} from '../services/projectBridge';
import { importTiledMap, loadProjectJson } from '../services/engineBridge';
// Import de TYPE seulement : la preview three.js (WebGPU + GLTFLoader) reste
// dans un chunk séparé, chargé à la sélection d'un fichier — le Hub ne paie pas
// le poids de three.js à l'ouverture.
import type { MeshPreviewController, MeshPreviewStats } from '../render3d/meshPreview';
import { ARCHETYPES } from '../services/archetypes';

interface ProjectHubModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenArchetypes: () => void;
  onOpenWizard: () => void;
  onOpenAiTab: () => void;
  onProjectOpened: (entry: ProjectEntry, mode: 'play' | 'edit') => void;
  /**
   * Notifications non bloquantes (ToastCenter de l'App). Optionnel : sans lui,
   * les erreurs restent dans le bandeau du Hub.
   */
  onNotify?: (message: string, kind?: 'info' | 'success' | 'error') => void;
  /** Appelé après une modification du projet depuis le Hub (placement d'asset). */
  onSceneChanged?: () => void;
  /**
   * Appelé au clic sur « Créer et jouer » (parcours guidé) : l'App démarre le
   * chrono AC1 « Hub affiché → état jouable » et l'affiche après création.
   */
  onCreateStart?: () => void;
}

const GENRES = [
  'Aventure', 'Action', "Shoot'em up", 'Plateforme', 'Jeu de rôle (RPG)', 'Simulation',
  'Stratégie', 'Course', 'Puzzle et réflexion', 'Horreur et survie', 'Combat',
  'Jeu éducatif', 'Projet personnalisé',
];

const GENRE_KEYS: Record<string, string> = {
  'Aventure': 'adventure', 'Action': 'action', "Shoot'em up": 'shoot_em_up',
  'Plateforme': 'platformer', 'Jeu de rôle (RPG)': 'rpg', 'Simulation': 'simulation',
  'Stratégie': 'strategy', 'Course': 'racing', 'Puzzle et réflexion': 'puzzle',
  'Horreur et survie': 'horror', 'Combat': 'combat', 'Jeu éducatif': 'educational',
  'Projet personnalisé': 'custom',
};

const DIMENSIONS = ['2D', '2.5D', '3D'];

/**
 * Licences connues proposées à l'import d'un asset.
 *
 * On stocke l'identifiant canonique (SPDX quand il existe), pas le libellé :
 * c'est lui qui permettra d'afficher les obligations d'attribution dans le paquet
 * exporté. « Inconnue » est le défaut sûr ; « Autre » ouvre un champ libre.
 */

type Filter = 'all' | 'scene' | 'project' | 'recent';
type SortKey = 'date' | 'name' | 'genre';
type HubView = 'home' | 'create' | 'projects';
type TemplateId = 'platformer' | 'rpg' | 'boss';

interface GameTemplate {
  id: TemplateId;
  emoji: string;
  title: string;
  description: string;
  detail: string;
  /** Clé de genre attendue par le registre Rust (`engine_create_project`). */
  genre: string;
  /** Archétype moteur : fournit une scène jouable prête à l'emploi. */
  archetype: string;
}

/**
 * 3 modèles grand public — un emoji, un titre, une phrase. Volontairement pas de
 * genre/dimension/archétype visibles : les valeurs techniques vivent dans « Avancé ».
 */
const TEMPLATES: GameTemplate[] = [
  {
    id: 'platformer',
    emoji: '🏃',
    title: 'Plateforme',
    description: 'Sautez de plateforme en plateforme jusqu’au portail d’arrivée.',
    detail: 'Gravité, saut, dash',
    genre: 'platformer',
    archetype: 'platformer',
  },
  {
    id: 'rpg',
    emoji: '🗺️',
    title: 'Aventure',
    description: 'Explorez un donjon, ramassez des pièces et ouvrez des portes.',
    detail: 'Vue de dessus, clés et coffres',
    genre: 'adventure',
    archetype: 'rpg',
  },
  {
    id: 'boss',
    emoji: '⚔️',
    title: 'Boss',
    description: 'Affrontez un titan dans une arène et esquivez ses attaques.',
    detail: 'Combat, esquive, riposte',
    genre: 'combat',
    archetype: 'boss',
  },
];

const DEFAULT_GAME_NAME = 'Mon premier jeu';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} Ko`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
}


/** Détection de format sans charger three.js (miroir de `meshPreview.ts`). */
function detecterFormat3D(buffer: ArrayBuffer): 'glb' | 'gltf' | null {
  const octets = new Uint8Array(buffer);
  if (
    octets.length >= 4 &&
    octets[0] === 0x67 && octets[1] === 0x6c && octets[2] === 0x54 && octets[3] === 0x46
  ) {
    return 'glb';
  }
  const debut = new TextDecoder().decode(octets.subarray(0, Math.min(octets.length, 512)));
  return debut.trimStart()[0] === '{' ? 'gltf' : null;
}

function formatDate(epoch: number): string {
  if (!epoch) return '—';
  return new Date(epoch * 1000).toLocaleDateString('fr-FR', {
    year: 'numeric', month: '2-digit', day: '2-digit',
  });
}

function genreColor(genre: string): string {
  const map: Record<string, string> = {
    'Aventure': '#10b981', 'Plateforme': '#8b5cf6', "Shoot'em up": '#ec4899',
    'Combat': '#ef4444', 'Puzzle et réflexion': '#f59e0b', 'Action': '#f97316',
    'Jeu de rôle (RPG)': '#38bdf8',
  };
  return map[genre] ?? '#a855f7';
}

function hashString(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

/** Vrai si le chemin déposé est une carte Tiled `.tmx` (insensible à la casse). */
function estCarteTiled(chemin: string): boolean {
  return /\.tmx$/i.test(chemin);
}

/**
 * Titre lisible déduit du nom de fichier du projet écrit par le moteur
 * (`carte_du_donjon.karacti` → « carte du donjon »).
 */
function titreDepuisChemin(chemin: string): string {
  const fichier = chemin.split(/[\\/]/).pop() ?? chemin;
  const sansExtension = fichier.replace(/\.karacti$/i, '').replace(/_/g, ' ');
  return sansExtension.trim() || 'Carte Tiled importée';
}

function SceneThumbnail({ entry }: { entry: ProjectEntry }) {
  const color = genreColor(entry.genre);
  const seed = hashString(entry.path || entry.title);
  const bars = useMemo(() => {
    const n = Math.min(10, 4 + (entry.entity_count % 7));
    return Array.from({ length: n }, (_, i) => {
      // Décalage NON signé : `seed` est un u32, `>>` peut produire un négatif et
      // donner une hauteur < 0 (SVG refusée, 9 erreurs console à l'ouverture du hub).
      const h = 10 + ((seed >>> (i % 16)) % 42);
      return { x: 6 + i * 11, y: 62 - h, h };
    });
  }, [seed, entry.entity_count]);
  return (
    <svg viewBox="0 0 120 72" className="w-full h-full" role="img" aria-label={`Aperçu de ${entry.title}`}>
      <rect width="120" height="72" fill="#0f172a" />
      <rect x="0" y="60" width="120" height="12" fill="#1e293b" />
      {bars.map((b, i) => (
        <rect key={i} x={b.x} y={b.y} width={10} height={b.h} fill={color} opacity={0.85} rx={1} />
      ))}
      <circle cx={16 + (seed % 70)} cy={16 + (seed % 18)} r={6} fill="#fbbf24" opacity={0.9} />
    </svg>
  );
}

export const ProjectHubModal: React.FC<ProjectHubModalProps> = ({
  isOpen, onClose, onOpenArchetypes, onOpenWizard, onOpenAiTab, onProjectOpened, onNotify,
  onCreateStart,
}) => {
  // Parcours débutant : accueil (1 CTA) → création (nom + modèle) → jeu.
  const [view, setView] = useState<HubView>('home');
  // « Avancé » : fermé par défaut, seul endroit où vivent Tiled / JSON / technique.
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [gameName, setGameName] = useState(DEFAULT_GAME_NAME);
  const [templateId, setTemplateId] = useState<TemplateId>('platformer');
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<Filter>('all');
  const [sort, setSort] = useState<SortKey>('date');
  const [confirmArchive, setConfirmArchive] = useState<string | null>(null);
  const [form, setForm] = useState({
    title: '', genre: 'Aventure', dimension: '2D', archetype: '', destination: '',
  });
  const [creating, setCreating] = useState(false);
  const [diag, setDiag] = useState<SystemDiagnostics | null>(null);
  const [guide, setGuide] = useState(false);
  // Import Tiled par glisser-déposer (Tauri v2 natif, sans plugin de dialogue).
  const [dropActive, setDropActive] = useState(false);
  const [importingTiled, setImportingTiled] = useState(false);
  // Verrou synchrone : deux drops rapprochés ne lancent pas deux imports.
  const importingTiledRef = useRef(false);

  // --- Import 3D (C2) : aperçu local au Hub, enregistrement réservé à Tauri ---
  const meshPreviewHostRef = useRef<HTMLDivElement | null>(null);
  const meshPreviewCtlRef = useRef<MeshPreviewController | null>(null);
  // Les octets restent hors de l'état React : un GLB de plusieurs Mo n'a pas à
  // provoquer de rendu ni de copie à chaque frappe dans le champ licence.
  const meshBytesRef = useRef<ArrayBuffer | null>(null);
  const meshLoadSeqRef = useRef(0);
  const [meshFileName, setMeshFileName] = useState('');
  const [meshStats, setMeshStats] = useState<MeshPreviewStats | null>(null);
  const [meshBusy, setMeshBusy] = useState(false);
  const [meshError, setMeshError] = useState('');
  const [meshPreviewActive, setMeshPreviewActive] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      setProjects(await listProjects());
    } catch (e: any) {
      setError(e?.message ?? String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  /** Notification non bloquante ; repli sur le bandeau d'erreur du Hub. */
  const notify = useCallback((message: string, kind: 'info' | 'success' | 'error' = 'info') => {
    if (onNotify) onNotify(message, kind);
    else if (kind === 'error') setError(message);
  }, [onNotify]);

  /**
   * Importe la première carte `.tmx` déposée : le moteur écrit le projet à côté
   * de la carte, l'ouvre, puis on rafraîchit la liste et on entre dans le Studio.
   */
  const importerCarteTiled = useCallback(async (paths: string[]) => {
    const cartes = paths.filter(estCarteTiled);
    if (cartes.length === 0) {
      notify('Fichier ignoré : seules les cartes Tiled (.tmx) peuvent être déposées.', 'error');
      return;
    }
    if (importingTiledRef.current) {
      notify('Un import de carte Tiled est déjà en cours.', 'info');
      return;
    }
    importingTiledRef.current = true;
    setImportingTiled(true);
    setError('');
    try {
      if (cartes.length > 1) {
        notify(
          `Une seule carte par import : « ${titreDepuisChemin(cartes[0])} » est utilisée.`,
          'info',
        );
      }
      const resultat = await importTiledMap(cartes[0]);
      await refresh();
      const entree: ProjectEntry = {
        path: resultat.project_path,
        title: titreDepuisChemin(resultat.project_path),
        description: 'Carte Tiled importée dans le Studio.',
        objective: null,
        genre: 'Personnalisé',
        dimension: '2D',
        kind: 'project',
        size_bytes: 0,
        modified_at: Math.floor(Date.now() / 1000),
        entity_count: resultat.entities,
        is_recent: true,
        status: 'available',
      };
      const resume = `Carte Tiled importée : ${entree.title} • ${resultat.entities} entité(s)`;
      notify(
        resultat.warnings.length > 0
          ? `${resume} • ${resultat.warnings.length} avertissement(s)`
          : resume,
        'success',
      );
      if (resultat.warnings.length > 0) notify(resultat.warnings[0], 'info');
      onProjectOpened(entree, 'edit');
    } catch (e: any) {
      const message = e?.message ?? String(e);
      setError(`Import Tiled impossible : ${message}`);
      notify(`Import Tiled impossible : ${message}`, 'error');
    } finally {
      importingTiledRef.current = false;
      setImportingTiled(false);
    }
  }, [notify, refresh, onProjectOpened]);

  // Le listener Tauri est enregistré une fois par ouverture de la section
  // « Avancé ». L'importeur passe par une ref : les re-rendus de l'App
  // (métriques) ne réinscrivent pas le listener à chaque frame.
  const importerRef = useRef(importerCarteTiled);
  useEffect(() => {
    importerRef.current = importerCarteTiled;
  }, [importerCarteTiled]);

  useEffect(() => {
    // Le glisser-déposer n'est actif que lorsque la zone est réellement visible :
    // un drop alors que « Avancé » est replié ne doit pas importer en silence.
    if (!isOpen || !advancedOpen || view !== 'home' || !isTauri()) return;
    let demonte = false;
    let desinscrire: (() => void) | null = null;

    void (async () => {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        const arreter = await getCurrentWebview().onDragDropEvent(evenement => {
          const charge = evenement.payload;
          if (charge.type === 'enter') {
            // Seules les cartes Tiled allument la zone ; le reste est ignoré.
            setDropActive(charge.paths.some(estCarteTiled));
          } else if (charge.type === 'leave') {
            setDropActive(false);
          } else if (charge.type === 'drop') {
            setDropActive(false);
            void importerRef.current(charge.paths);
          }
        });
        if (demonte) arreter();
        else desinscrire = arreter;
      } catch {
        // Fenêtre non native (navigateur) ou permission absente : la zone reste
        // informative et aucune exception ne remonte au rendu React.
      }
    })();

    return () => {
      demonte = true;
      setDropActive(false);
      if (desinscrire) desinscrire();
    };
  }, [isOpen, advancedOpen, view]);

  useEffect(() => {
    if (isOpen) void refresh();
  }, [isOpen, refresh]);

  useEffect(() => {
    // Diagnostics : chargés seulement quand « Avancé » est déplié (section repliée
    // par défaut → aucun IPC superflu à l'ouverture du Hub).
    if (isOpen && advancedOpen) {
      systemDiagnostics().then(setDiag).catch(() => setDiag(null));
    }
  }, [isOpen, advancedOpen]);

  useEffect(() => {
    // Aperçu 3D : le device GPU n'est créé qu'à la sélection d'un fichier, puis
    // libéré dès que « Avancé » se replie ou que le Hub change d'écran. Sortir
    // de la section ne doit pas laisser un renderer orphelin en arrière-plan.
    // L'état React est remis à zéro par les gestionnaires de navigation (pas ici :
    // un setState synchrone dans un effet déclencherait un rendu en cascade).
    if (advancedOpen && view === 'home') return;
    meshLoadSeqRef.current++;
    meshPreviewCtlRef.current?.dispose();
    meshPreviewCtlRef.current = null;
  }, [advancedOpen, view]);

  useEffect(() => () => {
    // Démontage du Hub (fermeture : App remonte le composant via sa `key`).
    meshLoadSeqRef.current++;
    meshPreviewCtlRef.current?.dispose();
    meshPreviewCtlRef.current = null;
  }, []);

  const visible = useMemo(() => {
    let list = [...projects];
    if (filter === 'scene') list = list.filter(p => p.kind === 'scene');
    if (filter === 'project') list = list.filter(p => p.kind === 'project');
    if (filter === 'recent') list = list.filter(p => p.is_recent);
    if (query.trim()) {
      const q = query.toLowerCase();
      list = list.filter(p =>
        `${p.title} ${p.genre} ${p.dimension}`.toLowerCase().includes(q));
    }
    list.sort((a, b) => {
      if (sort === 'name') return a.title.localeCompare(b.title, 'fr');
      if (sort === 'genre') return a.genre.localeCompare(b.genre, 'fr') || a.title.localeCompare(b.title, 'fr');
      return b.modified_at - a.modified_at;
    });
    return list;
  }, [projects, filter, query, sort]);

  if (!isOpen) return null;

  const tauriDisponible = isTauri();

  /** Ferme l'aperçu 3D et oublie la sélection (changement de section/écran). */
  const reinitialiserApercuMesh = () => {
    meshLoadSeqRef.current++;
    meshPreviewCtlRef.current?.dispose();
    meshPreviewCtlRef.current = null;
    meshBytesRef.current = null;
    setMeshPreviewActive(false);
    setMeshStats(null);
    setMeshFileName('');
    setMeshError('');
  };

  /** Parcours guidé : un nom + un modèle → création directe en mode Jouer. */
  const handleQuickCreate = async () => {
    const template = TEMPLATES.find(t => t.id === templateId) ?? TEMPLATES[0];
    const title = gameName.trim() || DEFAULT_GAME_NAME;
    setCreating(true);
    setError('');
    onCreateStart?.();
    try {
      const config: NewProjectConfig = {
        title,
        genre: template.genre,
        dimension: '2D',
        archetype: template.archetype,
        destination_dir: '',
        default_mode: 'play',
      };
      const entry = await createProject(config);
      onProjectOpened(entry, 'play');
    } catch (e: any) {
      setError(`Création impossible : ${e?.message ?? e}`);
    } finally {
      setCreating(false);
    }
  };

  const handleCreate = async () => {
    if (!form.title.trim()) {
      setError('Donnez un nom au projet.');
      return;
    }
    setCreating(true);
    setError('');
    try {
      const config: NewProjectConfig = {
        title: form.title.trim(),
        genre: GENRE_KEYS[form.genre] ?? 'adventure',
        dimension: form.dimension,
        archetype: form.archetype || null,
        destination_dir: form.destination.trim(),
        default_mode: 'edit',
      };
      const entry = await createProject(config);
      onProjectOpened(entry, 'edit');
    } catch (e: any) {
      setError(e?.message ?? String(e));
    } finally {
      setCreating(false);
    }
  };

  const openEntry = async (entry: ProjectEntry, mode: 'play' | 'edit') => {
    setError('');
    try {
      await openProject(entry.path);
      onProjectOpened(entry, mode);
    } catch (e: any) {
      setError(`Ouverture impossible : ${e?.message ?? e}`);
    }
  };

  const onDuplicate = async (entry: ProjectEntry) => {
    try {
      await duplicateProject(entry.path, `${entry.title} (copie)`);
      await refresh();
    } catch (e: any) { setError(e?.message ?? String(e)); }
  };

  const onRename = async (entry: ProjectEntry) => {
    const name = window.prompt('Nouveau nom du projet :', entry.title);
    if (!name?.trim()) return;
    try {
      await renameProject(entry.path, name.trim());
      await refresh();
    } catch (e: any) { setError(e?.message ?? String(e)); }
  };

  const onArchive = async (path: string) => {
    try {
      await archiveProject(path);
      setConfirmArchive(null);
      await refresh();
    } catch (e: any) { setError(e?.message ?? String(e)); }
  };

  const onRemoveRecent = async (path: string) => {
    try {
      await removeRecent(path);
      await refresh();
    } catch (e: any) { setError(e?.message ?? String(e)); }
  };

  const handleImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const code = await loadProjectJson(text);
      if (code === 0) {
        const imported = {
          path: file.name,
          title: file.name.replace(/\.(json|karacti)$/i, ''),
          description: 'Projet importé depuis un fichier .karacti JSON.',
          objective: null,
          genre: 'Personnalisé',
          dimension: '2D',
          kind: 'project',
          size_bytes: file.size,
          modified_at: Math.floor(Date.now() / 1000),
          entity_count: 0,
          is_recent: false,
          status: 'available',
        } as ProjectEntry;
        onProjectOpened(imported, 'edit');
      } else {
        setError(`Import échoué (code ${code}). Vérifiez le format .karacti.`);
      }
    } catch (err: any) {
      setError(`Lecture du fichier impossible : ${err?.message ?? err}`);
    }
  };

  /** Crée le renderer d'aperçu à la demande (une seule fois par ouverture). */
  const obtenirApercuMesh = async (): Promise<MeshPreviewController> => {
    if (meshPreviewCtlRef.current) return meshPreviewCtlRef.current;
    const hote = meshPreviewHostRef.current;
    if (!hote) {
      throw new Error("Zone d'aperçu 3D indisponible : rouvrez la section « Avancé ».");
    }
    const { startMeshPreview } = await import('../render3d/meshPreview');
    const controleur = await startMeshPreview(hote, (message) => {
      // La libération GPU ne doit jamais faire échouer l'import déjà enregistré.
      console.warn(`Aperçu 3D : ${message}`);
    });
    meshPreviewCtlRef.current = controleur;
    setMeshPreviewActive(true);
    return controleur;
  };

  /**
   * Lit le fichier choisi et l'affiche. Un format invalide laisse l'état
   * précédent intact : aucun octet, aucun résumé, aucune entrée de projet.
   */
  const handleMeshFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    // Permet de re-sélectionner le même fichier après une erreur.
    e.target.value = '';
    if (!file) return;

    if (!/\.(glb|gltf)$/i.test(file.name)) {
      setMeshError(
        `Format non reconnu : « ${file.name} ». Choisissez un fichier .glb ou .gltf (export glTF 2.0).`,
      );
      return;
    }

    const seq = ++meshLoadSeqRef.current;
    setMeshBusy(true);
    setMeshError('');

    try {
      const buffer = await file.arrayBuffer();
      const format = detecterFormat3D(buffer);
      if (!format) {
        setMeshError(
          'Contenu 3D non reconnu : le fichier doit être un GLB (magic « glTF ») ou un glTF JSON. Aucun état du projet n’a été modifié.',
        );
        return;
      }

      const controleur = await obtenirApercuMesh();
      if (seq !== meshLoadSeqRef.current) return;
      const stats = await controleur.load(buffer, file.name);
      if (seq !== meshLoadSeqRef.current) return;

      meshBytesRef.current = buffer;
      setMeshFileName(file.name);
      setMeshStats(stats);
      notify(
        `Modèle 3D chargé : ${stats.meshes} mesh(es), ${stats.triangles} triangles, ${stats.materials} matériau(x).`,
        'success',
      );
    } catch (err: any) {
      meshBytesRef.current = null;
      setMeshStats(null);
      setMeshFileName('');
      setMeshError(
        `Aperçu 3D impossible : ${err?.message ?? String(err)}`,
      );
      notify(`Aperçu 3D impossible : ${err?.message ?? String(err)}`, 'error');
    } finally {
      if (seq === meshLoadSeqRef.current) setMeshBusy(false);
    }
  };



  /** Toute sortie du Hub libère le contexte WebGPU de l'aperçu 3D. */
  const fermerHub = () => {
    reinitialiserApercuMesh();
    onClose();
  };



  const goHome = () => {
    reinitialiserApercuMesh();
    setView('home');
    setError('');
  };

  return (
    <div data-testid="hub" className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md select-none">
      <div className="relative flex flex-col w-full max-w-5xl max-h-[92vh] bg-[#0d0819] border border-[#a855f7] rounded-xl shadow-2xl shadow-purple-950/90 overflow-hidden">
        {/* En-tête */}
        <div className="flex items-center justify-between gap-3 px-6 py-4 bg-[#150d27] border-b border-[#2d1b4e]">
          <div className="flex items-center gap-3 min-w-0">
            <div className="flex items-center justify-center w-8 h-8 shrink-0 rounded bg-[#2d1b4e] border border-[#a855f7]">
              <span className="font-extrabold text-sm text-[#f8fafc]">KR</span>
            </div>
            <div className="min-w-0">
              <h2 className="text-sm font-bold text-[#f8fafc] tracking-wide">
                KARACTIRAGE STUDIO
              </h2>
              <p className="text-xs text-[#94a3b8]">
                Créez un jeu jouable en 3 clics — sans documentation.
              </p>
            </div>
          </div>
          <button
            onClick={fermerHub}
            className="flex items-center justify-center w-8 h-8 shrink-0 text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] rounded-lg transition"
            aria-label="Fermer"
          >
            <X size={18} />
          </button>
        </div>

        {/* Bandeau d'erreur — visible quel que soit l'écran du Hub. */}
        {error && (
          <div
            role="alert"
            className="mx-6 mt-3 flex items-center gap-2 px-3 py-2 bg-[#150d27] border border-[#f43f5e] rounded text-xs text-[#f8fafc]"
          >
            <AlertTriangle size={14} className="shrink-0 text-[#f43f5e]" />
            <span className="flex-1">{error}</span>
            <button
              className="flex items-center justify-center w-6 h-6 shrink-0 text-[#94a3b8] hover:text-[#f8fafc]"
              onClick={() => setError('')}
              aria-label="Masquer l'erreur"
            >
              <X size={14} />
            </button>
          </div>
        )}

        <div className="flex-1 overflow-y-auto">
          {/* ================= ACCUEIL — un seul CTA dominant ================= */}
          {view === 'home' && (
            <section className="flex flex-col items-center px-6 py-8 md:py-10 text-center">
              <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[#a855f7]">
                Bienvenue
              </p>
              <h3 className="mt-2 text-xl md:text-2xl font-bold text-[#f8fafc]">
                Votre premier jeu en 3 gestes
              </h3>
              <p className="mt-2 max-w-md text-sm leading-relaxed text-[#94a3b8]">
                Choisissez un modèle, gardez le nom proposé, cliquez
                sur «&nbsp;Créer et jouer&nbsp;». Aucun réglage à comprendre.
              </p>

              <button
                data-testid="hub-create-game"
                onClick={() => { setError(''); setView('create'); }}
                className="group mt-6 flex w-full max-w-xl items-center gap-4 rounded-xl border border-[#a855f7] bg-gradient-to-r from-[#9333ea] to-[#7e22ce] p-4 text-left shadow-lg shadow-purple-950/40 transition hover:border-[#f8fafc] hover:shadow-xl focus-visible:outline-[#f8fafc] md:p-5"
              >
                <span
                  aria-hidden="true"
                  className="flex items-center justify-center w-12 h-12 shrink-0 rounded-lg border border-[#f8fafc]/40 bg-[#0d0819]/40 text-[#f8fafc]"
                >
                  <Sparkles size={24} />
                </span>
                <span className="flex-1 min-w-0">
                  <span className="block text-lg font-bold text-[#f8fafc]">Créer un jeu</span>
                  <span className="mt-0.5 block text-sm text-[#f8fafc]">3 clics, sans documentation</span>
                </span>
                <ArrowRight
                  size={20}
                  aria-hidden="true"
                  className="shrink-0 text-[#f8fafc] transition group-hover:translate-x-0.5"
                />
              </button>

              <div className="mt-3 flex w-full max-w-xl flex-col gap-2 sm:flex-row">
                <button
                  data-testid="hub-open-project"
                  onClick={() => { setError(''); setView('projects'); }}
                  className="flex flex-1 items-center justify-center gap-2 min-h-[44px] px-4 rounded-lg border border-[#2d1b4e] bg-[#150d27] text-sm font-semibold text-[#f8fafc] transition hover:border-[#a855f7] hover:bg-[#1e1338]"
                >
                  <FolderOpen size={16} className="text-[#94a3b8]" aria-hidden="true" />
                  Ouvrir un projet
                </button>
                <button
                  data-testid="hub-see-demo"
                  onClick={onOpenArchetypes}
                  className="flex flex-1 items-center justify-center gap-2 min-h-[44px] px-4 rounded-lg border border-[#2d1b4e] bg-[#150d27] text-sm font-semibold text-[#f8fafc] transition hover:border-[#a855f7] hover:bg-[#1e1338]"
                >
                  <Gamepad2 size={16} className="text-[#94a3b8]" aria-hidden="true" />
                  Voir une démo
                </button>
              </div>

              {/* ============== AVANCÉ — replié par défaut ============== */}
              <div className="mt-6 w-full max-w-xl text-left">
                <button
                  data-testid="hub-advanced-toggle"
                  type="button"
                  aria-expanded={advancedOpen}
                  aria-controls="hub-advanced"
                  onClick={() => setAdvancedOpen(v => !v)}
                  className="flex items-center justify-between gap-2 w-full min-h-[40px] px-4 py-2 rounded-lg border border-[#2d1b4e] bg-[#150d27] text-xs font-semibold text-[#94a3b8] transition hover:border-[#a855f7] hover:text-[#f8fafc]"
                >
                  <span className="flex items-center gap-2">
                    <Wrench size={14} aria-hidden="true" />
                    Avancé
                  </span>
                  <span className="flex items-center gap-2 text-[11px] font-normal">
                    Tiled, JSON, réglages techniques
                    <ChevronDown
                      size={14}
                      aria-hidden="true"
                      className={advancedOpen ? 'rotate-180 transition-transform' : 'transition-transform'}
                    />
                  </span>
                </button>

                <div id="hub-advanced" hidden={!advancedOpen} className="mt-3 space-y-3">
                  {/* Bande Tiled : présente uniquement quand « Avancé » est ouvert. */}
                  <div className="flex flex-wrap items-center gap-2">
                    <div
                      data-testid="tiled-drop-zone"
                      role="region"
                      aria-label="Import de carte Tiled par glisser-déposer"
                      aria-describedby="tiled-drop-help"
                      aria-busy={importingTiled}
                      tabIndex={0}
                      className={`flex min-h-[40px] flex-1 min-w-[220px] items-center gap-2 px-3 py-2 rounded border border-dashed text-[11px] outline-none transition ${
                        dropActive
                          ? 'border-[#a855f7] bg-[#2e1065]/60 text-[#f8fafc]'
                          : 'border-[#2d1b4e] bg-[#150d27] text-[#94a3b8]'
                      } focus-visible:border-[#a855f7] focus-visible:ring-1 focus-visible:ring-[#a855f7] ${
                        tauriDisponible ? '' : 'opacity-60'
                      }`}
                    >
                      {importingTiled
                        ? <Loader2 size={13} className="animate-spin text-[#a855f7] shrink-0" />
                        : <Upload size={13} className="text-[#a855f7] shrink-0" />}
                      <span id="tiled-drop-help" className="font-medium">
                        {importingTiled
                          ? 'Import de la carte Tiled en cours…'
                          : dropActive
                            ? 'Relâchez pour importer la carte Tiled'
                            : 'Déposez une carte Tiled (.tmx)'}
                      </span>
                      <span className="ml-auto shrink-0 text-[10px] text-[#94a3b8]">
                        {tauriDisponible
                          ? (importingTiled ? 'écriture du projet .karacti…' : 'glisser-déposer')
                          : 'application de bureau uniquement'}
                      </span>
                    </div>

                    <label className="flex items-center justify-center gap-1.5 min-h-[40px] px-3 rounded border border-[#2d1b4e] bg-[#150d27] text-[11px] font-semibold text-[#f8fafc] cursor-pointer transition hover:border-[#a855f7] hover:bg-[#1e1338]">
                      <input
                        type="file"
                        accept=".json,.karacti,application/json"
                        className="hidden"
                        aria-label="Importer un projet JSON (.karacti)"
                        onChange={handleImport}
                      />
                      <Upload size={13} className="text-[#94a3b8]" aria-hidden="true" />
                      Importer un JSON (.karacti)
                    </label>
                  </div>

                  {!tauriDisponible && (
                    <p className="text-[10px] text-[#f59e0b]">
                      Mode navigateur : l'import Tiled nécessite la fenêtre native du Studio.
                    </p>
                  )}

                  {/* ============ Import d'un modèle 3D (C2) ============ */}
                  <div className="p-4 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-3">
                    <div className="flex flex-wrap items-start justify-between gap-2">
                      <div>
                        <h4 className="flex items-center gap-2 text-xs font-bold text-[#f8fafc]">
                          <Box size={14} className="text-[#a855f7]" aria-hidden="true" />
                          Importer un modèle 3D
                        </h4>
                        <p className="mt-1 text-[11px] leading-relaxed text-[#94a3b8]">
                          Fichiers <span className="font-mono text-[#f8fafc]">.glb</span> ou{' '}
                          <span className="font-mono text-[#f8fafc]">.gltf</span> autoporteur
                          (glTF 2.0). L’aperçu s’affiche ici, sans toucher à la scène du jeu.
                        </p>
                      </div>
                      <label className="flex items-center justify-center gap-1.5 min-h-[40px] px-3 rounded border border-[#a855f7] bg-[#2d1b4e] text-[11px] font-semibold text-[#f8fafc] cursor-pointer transition hover:bg-[#3b0764]">
                        <input
                          data-testid="mesh-file-input"
                          type="file"
                          accept=".glb,.gltf,model/gltf-binary,model/gltf+json"
                          className="hidden"
                          aria-label="Importer un modèle 3D (.glb, .gltf)"
                          onChange={handleMeshFile}
                        />
                        {meshBusy
                          ? <Loader2 size={13} className="animate-spin" aria-hidden="true" />
                          : <Upload size={13} aria-hidden="true" />}
                        {meshBusy ? 'Chargement…' : 'Importer un modèle 3D (.glb, .gltf)'}
                      </label>
                    </div>

                    {/* Zone d'aperçu : le canvas three.js est ajouté ici à la demande. */}
                    <div
                      data-testid="mesh-preview"
                      className="relative h-56 overflow-hidden rounded border border-[#2d1b4e] bg-[#0b0d14]"
                    >
                      <div ref={meshPreviewHostRef} className="absolute inset-0" />
                      {!meshPreviewActive && (
                        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-1 text-center">
                          <Box size={28} className="text-[#2d1b4e]" aria-hidden="true" />
                          <p className="text-[11px] font-medium text-[#94a3b8]">
                            Aucun modèle chargé
                          </p>
                          <p className="text-[10px] text-[#64748b]">
                            Glisser-déposer non requis : utilisez le bouton ci-dessus.
                          </p>
                        </div>
                      )}
                      {meshPreviewActive && (
                        <span className="pointer-events-none absolute bottom-2 left-2 rounded bg-black/60 px-2 py-0.5 font-mono text-[10px] text-[#94a3b8]">
                          glisser pour orbiter · molette pour zoomer
                        </span>
                      )}
                    </div>

                    {meshError && (
                      <p
                        role="alert"
                        data-testid="mesh-import-error"
                        className="flex items-start gap-1.5 rounded border border-[#f43f5e] bg-[#0d0819] px-2.5 py-2 text-[11px] text-[#fecaca]"
                      >
                        <AlertTriangle size={13} className="mt-0.5 shrink-0 text-[#f43f5e]" aria-hidden="true" />
                        <span className="flex-1">{meshError}</span>
                      </p>
                    )}

                    {meshStats && (
                      <div
                        data-testid="mesh-preview-summary"
                        data-meshes={meshStats.meshes}
                        data-triangles={meshStats.triangles}
                        data-materials={meshStats.materials}
                        data-animations={meshStats.animations}
                        data-format={meshStats.format}
                        className="grid grid-cols-2 gap-2 sm:grid-cols-4"
                      >
                        <MeshStat label="Meshes" value={String(meshStats.meshes)} testId="mesh-summary-meshes" />
                        <MeshStat label="Triangles" value={String(meshStats.triangles)} testId="mesh-summary-triangles" />
                        <MeshStat label="Matériaux" value={String(meshStats.materials)} testId="mesh-summary-materials" />
                        <MeshStat label="Animations" value={String(meshStats.animations)} testId="mesh-summary-animations" />
                        <p className="col-span-2 truncate text-[10px] text-[#94a3b8] sm:col-span-4" title={meshFileName}>
                          <span className="font-medium text-[#f8fafc]">{meshFileName}</span>
                          {' · '}{formatBytes(meshStats.sizeBytes)}
                          {' · '}{meshStats.nodes} nœud(s)
                          {' · '}{meshStats.textures} texture(s)
                          {' · '}boîte {meshStats.bounds.x} × {meshStats.bounds.y} × {meshStats.bounds.z}
                          {meshStats.animations > 0 && ` · clips : ${meshStats.animationNames.join(', ')}`}
                        </p>
                      </div>
                    )}

                    <p
                      data-testid="mesh-import-ailleurs"
                      className="rounded border border-[#a855f7] bg-[#2e1065] px-3 py-2 text-[11px] leading-relaxed text-[#e9d5ff]"
                    >
                      Pour l'utiliser : passez en <strong>vue 3D</strong>. Le bouton
                      «&nbsp;Importer un modèle 3D&nbsp;» est dans la barre du viewport : un seul
                      clic l'enregistre dans le projet et le place dans la scène.
                    </p>

                    {!tauriDisponible && (
                      <p data-testid="mesh-save-disabled" className="text-[10px] leading-relaxed text-[#f59e0b]">
                        Mode navigateur : l’aperçu 3D fonctionne, mais l’enregistrement dans le
                        projet est désactivé (la fenêtre native Tauri est requise pour écrire
                        assets/meshes et le manifeste).
                      </p>
                    )}



                    <p data-testid="mesh-placement-limite" className="text-[10px] leading-relaxed text-[#94a3b8]">
                      Le modèle placé devient une entité du projet (hiérarchie, sauvegarde). Le viewport
                      3D le dessine via le chargeur glTF ; si le fichier est illisible, un cube de repli
                      signale l'entité sans casser le rendu.
                    </p>
                  </div>

                  {/* Formulaire technique — valeurs par défaut saines, jamais montrées au débutant. */}
                  <div className="p-4 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-3">
                    <h4 className="flex items-center gap-2 text-xs font-bold text-[#f8fafc]">
                      <Wrench size={14} className="text-[#94a3b8]" aria-hidden="true" />
                      Création technique (Studio)
                    </h4>
                    <p className="text-[11px] leading-relaxed text-[#94a3b8]">
                      Pour choisir genre, dimension et modèle. Le parcours « Créer un jeu »
                      utilise des valeurs sûres par défaut.
                    </p>
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                      <label className="block text-[11px] text-[#94a3b8] sm:col-span-2">
                        Nom du projet
                        <input
                          value={form.title}
                          onChange={e => setForm(f => ({ ...f, title: e.target.value }))}
                          placeholder="Ex : Mon Donjon de Lave"
                          className="mt-1 w-full px-3 py-2 bg-[#0d0819] border border-[#2d1b4e] rounded text-sm text-[#f8fafc] focus:border-[#a855f7]"
                        />
                      </label>
                      <label className="block text-[11px] text-[#94a3b8]">
                        Genre
                        <select
                          value={form.genre}
                          onChange={e => setForm(f => ({ ...f, genre: e.target.value }))}
                          className="mt-1 w-full px-2 py-2 bg-[#0d0819] border border-[#2d1b4e] rounded text-sm text-[#f8fafc]"
                        >
                          {GENRES.map(g => <option key={g} value={g}>{g}</option>)}
                        </select>
                      </label>
                      <label className="block text-[11px] text-[#94a3b8]">
                        Dimension
                        {meshStats ? (
                          // Un maillage glTF impose la 3D : proposer 2D / 2.5D n'a plus de sens.
                          <p
                            data-testid="dimension-imposee"
                            className="mt-1 rounded border border-[#a855f7] bg-[#2e1065] px-2 py-2 text-sm font-semibold text-[#e9d5ff]"
                          >
                            3D (imposée par le modèle importé)
                          </p>
                        ) : (
                          <select
                            value={form.dimension}
                            onChange={e => setForm(f => ({ ...f, dimension: e.target.value }))}
                            className="mt-1 w-full px-2 py-2 bg-[#0d0819] border border-[#2d1b4e] rounded text-sm text-[#f8fafc]"
                          >
                            {DIMENSIONS.map(d => <option key={d} value={d}>{d}</option>)}
                          </select>
                        )}
                      </label>
                      <label className="block text-[11px] text-[#94a3b8] sm:col-span-2">
                        Modèle de départ (optionnel)
                        <select
                          value={form.archetype}
                          onChange={e => setForm(f => ({ ...f, archetype: e.target.value }))}
                          className="mt-1 w-full px-2 py-2 bg-[#0d0819] border border-[#2d1b4e] rounded text-sm text-[#f8fafc]"
                        >
                          <option value="">Projet vierge (Studio)</option>
                          {ARCHETYPES.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
                        </select>
                      </label>
                      <label className="block text-[11px] text-[#94a3b8] sm:col-span-2">
                        Dossier de destination (vide = défaut)
                        <input
                          value={form.destination}
                          onChange={e => setForm(f => ({ ...f, destination: e.target.value }))}
                          placeholder="~/.karactirage/projects"
                          className="mt-1 w-full px-3 py-2 bg-[#0d0819] border border-[#2d1b4e] rounded text-sm text-[#f8fafc] focus:border-[#a855f7]"
                        />
                      </label>
                    </div>
                    <button
                      onClick={handleCreate}
                      disabled={creating}
                      className="flex items-center justify-center gap-2 w-full min-h-[40px] px-4 py-2 bg-[#2d1b4e] hover:bg-[#3b0764] border border-[#a855f7] text-[#f8fafc] rounded text-xs font-bold transition disabled:opacity-60"
                    >
                      {creating ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
                      {creating ? 'Création…' : 'Créer & ouvrir dans le Studio'}
                    </button>
                  </div>

                  {/* Outils avancés (assistant, IA, guide, diagnostics). */}
                  <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                    <button
                      onClick={onOpenWizard}
                      className="flex items-center justify-center gap-1.5 min-h-[40px] px-3 rounded border border-[#2d1b4e] bg-[#150d27] text-xs font-semibold text-[#f8fafc] transition hover:border-[#a855f7] hover:bg-[#1e1338]"
                    >
                      Assistant 6 étapes [W] <ArrowRight size={13} aria-hidden="true" />
                    </button>
                    <button
                      onClick={onOpenAiTab}
                      className="flex items-center justify-center gap-1.5 min-h-[40px] px-3 rounded border border-[#2d1b4e] bg-[#150d27] text-xs font-semibold text-[#f8fafc] transition hover:border-[#a855f7] hover:bg-[#1e1338]"
                    >
                      <Bot size={13} className="text-[#94a3b8]" aria-hidden="true" /> Intention IA [I]
                    </button>
                    <button
                      onClick={() => setGuide(true)}
                      className="flex items-center justify-center gap-1.5 min-h-[40px] px-3 rounded border border-[#2d1b4e] bg-[#150d27] text-xs font-semibold text-[#f8fafc] transition hover:border-[#a855f7] hover:bg-[#1e1338]"
                    >
                      <Compass size={13} className="text-[#94a3b8]" aria-hidden="true" /> Visite guidée du Studio
                    </button>
                    <div className="rounded border border-[#2d1b4e] bg-[#150d27]">
                      <details>
                        <summary className="flex items-center gap-1.5 min-h-[40px] px-3 text-xs font-semibold text-[#f8fafc] cursor-pointer">
                          <Cpu size={13} className="text-[#94a3b8]" aria-hidden="true" /> Diagnostics système
                        </summary>
                        <div className="px-3 pb-3">
                          {diag ? (
                            <div className="grid grid-cols-2 gap-2 text-[11px]">
                              <Diag label="Système" value={`${diag.os} / ${diag.arch}`} />
                              <Diag label="Cœurs CPU" value={String(diag.cpu_cores)} />
                              <Diag label="FPS" value={diag.fps.toFixed(0)} />
                              <Diag label="Entités" value={String(diag.entities)} />
                              <Diag label="Dossier projets" value={diag.projects_home} />
                              <Diag label="Moteur" value={diag.engine} />
                            </div>
                          ) : (
                            <p className="text-[11px] text-[#94a3b8] flex items-center gap-2">
                              <Loader2 size={13} className="animate-spin" /> Collecte des métriques…
                            </p>
                          )}
                          {!tauriDisponible && (
                            <p className="mt-2 text-[11px] text-[#f59e0b]">
                              Mode web : le Hub fonctionne en lecture seule (démonstration). Lancez le
                              Studio Tauri pour la gestion complète des projets sur disque.
                            </p>
                          )}
                        </div>
                      </details>
                    </div>
                  </div>
                </div>
              </div>
            </section>
          )}

          {/* ================= ÉCRAN 1 DU PARCOURS — nom + 3 modèles ================= */}
          {view === 'create' && (
            <section className="px-6 py-6">
              <div className="mx-auto max-w-3xl">
                <button
                  onClick={goHome}
                  className="flex items-center gap-1.5 min-h-[32px] px-1 text-xs font-semibold text-[#94a3b8] transition hover:text-[#f8fafc]"
                >
                  <ArrowLeft size={14} aria-hidden="true" />
                  Accueil
                </button>

                <h3 className="mt-3 text-xl font-bold text-[#f8fafc]">Créer un jeu</h3>
                <p className="mt-1 text-sm leading-relaxed text-[#94a3b8]">
                  Un nom, un modèle — c’est tout. Vous pourrez tout modifier ensuite dans le Studio.
                </p>

                <label className="mt-5 block text-xs font-semibold text-[#94a3b8]">
                  Nom du jeu
                  <input
                    data-testid="game-name"
                    value={gameName}
                    onChange={e => setGameName(e.target.value)}
                    placeholder={DEFAULT_GAME_NAME}
                    className="mt-1 w-full min-h-[44px] px-3 py-2 bg-[#150d27] border border-[#2d1b4e] rounded-lg text-base text-[#f8fafc] focus:border-[#a855f7]"
                  />
                </label>

                <div
                  role="radiogroup"
                  aria-label="Modèle de jeu"
                  className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-3"
                >
                  {TEMPLATES.map(t => {
                    const selected = templateId === t.id;
                    return (
                      <button
                        key={t.id}
                        type="button"
                        role="radio"
                        aria-checked={selected}
                        data-testid={`template-${t.id}`}
                        onClick={() => setTemplateId(t.id)}
                        className={`flex flex-col min-h-[150px] p-4 rounded-xl border text-left transition ${
                          selected
                            ? 'border-[#a855f7] bg-[#1e1338] ring-1 ring-[#a855f7]'
                            : 'border-[#2d1b4e] bg-[#150d27] hover:border-[#a855f7] hover:bg-[#1e1338]'
                        }`}
                      >
                        <span className="flex items-start justify-between gap-2">
                          <span aria-hidden="true" className="text-3xl leading-none">{t.emoji}</span>
                          <span
                            aria-hidden="true"
                            className={`flex items-center justify-center w-6 h-6 shrink-0 rounded-full border ${
                              selected
                                ? 'border-[#a855f7] bg-[#a855f7] text-[#0d0819]'
                                : 'border-[#2d1b4e] text-transparent'
                            }`}
                          >
                            <Check size={14} />
                          </span>
                        </span>
                        <span className="mt-3 block text-base font-bold text-[#f8fafc]">{t.title}</span>
                        <span className="mt-1 block text-xs leading-relaxed text-[#94a3b8]">{t.description}</span>
                        <span className="mt-2 block text-[11px] text-[#94a3b8]">{t.detail}</span>
                      </button>
                    );
                  })}
                </div>

                <button
                  data-testid="create-and-play"
                  onClick={handleQuickCreate}
                  disabled={creating || !gameName.trim()}
                  className="mt-5 flex items-center justify-center gap-2 w-full min-h-[48px] px-4 py-3 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] text-[#f8fafc] rounded-lg text-base font-bold transition hover:shadow-lg hover:shadow-purple-950/60 focus-visible:outline-[#f8fafc] disabled:opacity-60"
                >
                  {creating ? <Loader2 size={16} className="animate-spin" /> : <Play size={16} aria-hidden="true" />}
                  {creating ? 'Création…' : 'Créer et jouer'}
                </button>
                <p className="mt-2 text-center text-[11px] text-[#94a3b8]">
                  Le jeu s’ouvre directement en mode Jouer. Flèches pour bouger, E pour interagir.
                </p>
              </div>
            </section>
          )}

          {/* ================= PROJETS SUR DISQUE ================= */}
          {view === 'projects' && (
            <section className="px-6 py-6 space-y-4">
              <div>
                <button
                  onClick={goHome}
                  className="flex items-center gap-1.5 min-h-[32px] px-1 text-xs font-semibold text-[#94a3b8] transition hover:text-[#f8fafc]"
                >
                  <ArrowLeft size={14} aria-hidden="true" />
                  Accueil
                </button>
                <h3 className="mt-2 text-xl font-bold text-[#f8fafc]">Ouvrir un projet</h3>
                <p className="mt-1 text-sm text-[#94a3b8]">
                  Vos projets et scènes sur disque. Lancez-les en mode Jouer ou ouvrez-les dans le Studio.
                </p>
              </div>

              <div className="flex flex-wrap items-center gap-3">
                <div className="relative flex-1 min-w-[200px]">
                  <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-[#94a3b8]" aria-hidden="true" />
                  <input
                    value={query}
                    onChange={e => setQuery(e.target.value)}
                    placeholder="Rechercher un projet ou une scène…"
                    aria-label="Rechercher un projet ou une scène"
                    className="w-full min-h-[40px] pl-8 pr-3 py-2 bg-[#150d27] border border-[#2d1b4e] rounded text-sm text-[#f8fafc] focus:border-[#a855f7]"
                  />
                </div>
                <div className="flex items-center gap-1" role="group" aria-label="Filtrer les projets">
                  {(['all', 'scene', 'project', 'recent'] as Filter[]).map(f => (
                    <button
                      key={f}
                      onClick={() => setFilter(f)}
                      aria-pressed={filter === f}
                      className={`min-h-[32px] px-2.5 py-1.5 rounded text-[11px] font-semibold border transition ${
                        filter === f
                          ? 'bg-[#2d1b4e] text-[#f8fafc] border-[#a855f7]'
                          : 'bg-[#150d27] text-[#94a3b8] border-[#2d1b4e] hover:text-[#f8fafc]'
                      }`}
                    >
                      {f === 'all' ? 'Tous' : f === 'scene' ? 'Scènes' : f === 'project' ? 'Projets' : 'Récents'}
                    </button>
                  ))}
                </div>
                <select
                  value={sort}
                  onChange={e => setSort(e.target.value as SortKey)}
                  aria-label="Trier les projets"
                  className="min-h-[40px] px-2 py-2 bg-[#150d27] border border-[#2d1b4e] rounded text-sm text-[#f8fafc]"
                >
                  <option value="date">Tri : Date</option>
                  <option value="name">Tri : Nom</option>
                  <option value="genre">Tri : Genre</option>
                </select>
                <button
                  onClick={() => void refresh()}
                  className="flex items-center justify-center w-10 h-10 bg-[#150d27] hover:bg-[#1e1338] border border-[#2d1b4e] text-[#94a3b8] rounded transition"
                  aria-label="Actualiser la liste des projets"
                  title="Actualiser"
                >
                  <RefreshCw size={14} className={loading ? 'animate-spin' : ''} aria-hidden="true" />
                </button>
              </div>

              {confirmArchive && (
                <div className="flex items-center gap-3 px-4 py-2.5 bg-[#150d27] border border-[#f43f5e] rounded text-xs">
                  <AlertTriangle size={15} className="text-[#f43f5e]" aria-hidden="true" />
                  <span className="text-[#f8fafc]">
                    Archiver ce projet ? Il sera déplacé vers l’archive (aucune suppression).
                  </span>
                  <button
                    onClick={() => void onArchive(confirmArchive)}
                    className="ml-auto min-h-[28px] px-3 py-1 bg-[#f43f5e] hover:bg-[#e11d48] text-[#f8fafc] rounded font-bold"
                  >
                    Confirmer
                  </button>
                  <button
                    onClick={() => setConfirmArchive(null)}
                    className="min-h-[28px] px-3 py-1 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] rounded"
                  >
                    Annuler
                  </button>
                </div>
              )}

              {loading && (
                <div className="flex items-center justify-center gap-2 py-10 text-[#94a3b8] text-sm">
                  <Loader2 size={18} className="animate-spin" aria-hidden="true" />
                  Scan du disque en cours…
                </div>
              )}

              {!loading && visible.length === 0 && (
                <div className="py-10 text-center bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
                  <FolderSearch size={32} className="mx-auto text-[#a855f7]" aria-hidden="true" />
                  <p className="text-sm text-[#f8fafc] font-semibold">Aucun projet trouvé</p>
                  <p className="text-xs text-[#94a3b8]">
                    Créez votre premier jeu depuis l’accueil, ou ajustez vos filtres.
                  </p>
                  <button
                    onClick={goHome}
                    className="mt-1 min-h-[32px] px-3 py-1.5 rounded border border-[#a855f7] bg-[#150d27] text-xs font-semibold text-[#f8fafc] hover:bg-[#1e1338]"
                  >
                    Créer un jeu
                  </button>
                </div>
              )}

              {!loading && visible.length > 0 && (
                <div className="space-y-3">
                  {visible.map(entry => (
                    <div
                      key={entry.path}
                      className="flex items-center gap-3 p-3 bg-[#150d27] border border-[#2d1b4e] rounded-lg hover:border-[#a855f7]/60 transition"
                    >
                      <div className="w-28 h-16 shrink-0 rounded overflow-hidden border border-[#2d1b4e]">
                        <SceneThumbnail entry={entry} />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-semibold text-sm text-[#f8fafc] truncate" title={entry.title}>{entry.title}</span>
                          {entry.is_recent && (
                            <span className="text-[9px] uppercase px-1.5 py-0.5 rounded bg-[#2d1b4e] text-[#f8fafc]">Récent</span>
                          )}
                          {entry.kind === 'scene' && (
                            <span className="text-[9px] uppercase px-1.5 py-0.5 rounded bg-[#0c4a6e] text-[#7dd3fc]">Démo</span>
                          )}
                        </div>
                        <div className="text-[11px] text-[#94a3b8]">
                          {entry.genre} • {entry.dimension} • {entry.entity_count} entités • {formatBytes(entry.size_bytes)} • {formatDate(entry.modified_at)}
                        </div>
                        <div className="text-[10px] text-[#94a3b8] truncate" title={entry.path}>{entry.path}</div>
                      </div>
                      <div className="shrink-0">
                        {entry.status === 'missing'
                          ? <span className="px-2 py-1 rounded text-[10px] font-bold bg-[#150d27] text-[#f43f5e] border border-[#f43f5e]">⚠ Introuvable</span>
                          : <span className="px-2 py-1 rounded text-[10px] font-bold bg-[#150d27] text-[#34d399] border border-[#34d399]">✓ Disponible</span>}
                      </div>
                      <div className="flex items-center gap-1 shrink-0">
                        <IconButton onClick={() => openEntry(entry, 'play')} title="Jouer"><Play size={14} /></IconButton>
                        <IconButton onClick={() => openEntry(entry, 'edit')} title="Ouvrir dans le Studio"><Wrench size={14} /></IconButton>
                        <IconButton onClick={() => void revealProject(entry.path).catch(() => setError("Impossible d'ouvrir le dossier."))} title="Ouvrir le dossier"><FolderSearch size={14} /></IconButton>
                        <IconButton onClick={() => onDuplicate(entry)} title="Dupliquer"><Copy size={14} /></IconButton>
                        <IconButton onClick={() => onRename(entry)} title="Renommer"><Pencil size={14} /></IconButton>
                        {entry.is_recent && <IconButton onClick={() => onRemoveRecent(entry.path)} title="Retirer des récents"><X size={14} /></IconButton>}
                        {entry.kind === 'project' && <IconButton danger onClick={() => setConfirmArchive(entry.path)} title="Archiver"><Archive size={14} /></IconButton>}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>
          )}
        </div>

        {/* Pied de page */}
        <div className="flex items-center justify-between gap-3 px-6 py-3.5 bg-[#150d27] border-t border-[#2d1b4e]">
          <span className="text-xs text-[#94a3b8]">
            {tauriDisponible
              ? 'Projets persistés dans ~/.karactirage — Échap pour fermer'
              : 'Mode web (lecture seule) — Échap pour fermer'}
          </span>
          <button
            onClick={fermerHub}
            className="min-h-[36px] px-4 py-2 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] rounded-lg text-xs font-medium transition"
          >
            Fermer [Échap]
          </button>
        </div>
      </div>
      {guide && <StudioGuideModal onClose={() => setGuide(false)} />}
    </div>
  );
};

function IconButton({
  onClick, title, danger, children,
}: {
  onClick: () => void;
  title: string;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      aria-label={title}
      className={`flex items-center justify-center w-7 h-7 rounded transition ${
        danger
          ? 'text-[#f87171] hover:bg-[#3b0a0a]'
          : 'text-[#94a3b8] hover:bg-[#1e1338] hover:text-[#f8fafc]'
      }`}
    >
      {children}
    </button>
  );
}

function MeshStat({ label, value, testId }: { label: string; value: string; testId: string }) {
  return (
    <div className="rounded border border-[#2d1b4e] bg-[#0d0819] px-2 py-1.5">
      <div className="text-[10px] uppercase tracking-wide text-[#94a3b8]">{label}</div>
      <div data-testid={testId} className="font-mono text-sm font-bold text-[#f8fafc]">{value}</div>
    </div>
  );
}

function Diag({ label, value }: { label: string; value: string }) {
  return (
    <div className="px-2 py-1.5 bg-[#0d0819] border border-[#2d1b4e] rounded">
      <div className="text-[10px] uppercase text-[#94a3b8]">{label}</div>
      <div className="text-[#f8fafc] font-medium break-words">{value}</div>
    </div>
  );
}

/** Visite guidée « Comprendre le Studio » (UX-501) : 3 écrans + raccourcis. */
function StudioGuideModal({ onClose }: { onClose: () => void }) {
  const [step, setStep] = useState(0);
  const STEPS = [
    {
      icon: <Sparkles size={20} />,
      title: 'Créer',
      text: 'Posez des objets (tiles, entités) dans le viewport 2D/3D, puis réglez leur transform et leurs propriétés dans l\u2019inspecteur.',
    },
    {
      icon: <Layers size={20} />,
      title: 'Logique',
      text: 'Dans l\u2019onglet Logique, générez une recette guidée qui produit un graphe KVL valide, ou câblez des nœuds à la main.',
    },
    {
      icon: <Zap size={20} />,
      title: 'Tester',
      text: 'Lancez le jeu (mode Play) pour valider la boucle déterministe 60 FPS. F6 change de mode, Ctrl+1…5 accède directement à un mode.',
    },
  ];
  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm select-none"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-md bg-[#0d0819] border border-[#a855f7] rounded-xl shadow-2xl shadow-purple-950/90 overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-3.5 bg-[#150d27] border-b border-[#2d1b4e]">
          <h3 className="text-sm font-bold text-[#f8fafc]">Comprendre le Studio</h3>
          <button
            onClick={onClose}
            className="flex items-center justify-center w-8 h-8 text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] rounded-lg transition"
            aria-label="Fermer le guide"
          >
            <X size={16} />
          </button>
        </div>
        <div className="p-5 space-y-4">
          <div className="flex gap-1.5">
            {STEPS.map((s, i) => (
              <button
                key={i}
                onClick={() => setStep(i)}
                aria-label={`Étape ${i + 1} : ${s.title}`}
                className={`h-1.5 flex-1 rounded-full transition ${i === step ? 'bg-[#a855f7]' : 'bg-[#2d1b4e] hover:bg-[#3b0764]'}`}
              />
            ))}
          </div>
          <div className="flex items-start gap-3">
            <div className="flex items-center justify-center w-10 h-10 rounded-lg bg-[#2d1b4e] border border-[#a855f7] shrink-0 text-[#f8fafc]">
              {STEPS[step].icon}
            </div>
            <div>
              <div className="font-bold text-sm text-[#f8fafc]">{STEPS[step].title}</div>
              <p className="text-xs text-[#94a3b8] leading-relaxed mt-1">{STEPS[step].text}</p>
            </div>
          </div>
          <div className="rounded-lg bg-[#150d27] border border-[#2d1b4e] p-3 space-y-1.5 text-[11px] text-[#94a3b8]">
            <div className="flex justify-between"><span>Mode suivant</span><kbd className="text-[#f8fafc] font-mono">F6</kbd></div>
            <div className="flex justify-between"><span>Changer de mode</span><kbd className="text-[#f8fafc] font-mono">Ctrl+1…5</kbd></div>
            <div className="flex justify-between"><span>Enregistrer / annuler / rétablir</span><kbd className="text-[#f8fafc] font-mono">Ctrl+S / Z / Y</kbd></div>
          </div>
          <div className="flex items-center justify-between">
            <button
              onClick={() => setStep(s => Math.max(0, s - 1))}
              disabled={step === 0}
              className="min-h-[32px] px-3 py-2 text-xs font-semibold text-[#94a3b8] hover:text-[#f8fafc] disabled:opacity-40 transition"
            >
              ← Précédent
            </button>
            {step < STEPS.length - 1 ? (
              <button
                onClick={() => setStep(s => Math.min(STEPS.length - 1, s + 1))}
                className="min-h-[32px] px-3 py-2 text-xs font-semibold text-[#a855f7] hover:text-[#f8fafc] transition"
              >
                Suivant →
              </button>
            ) : (
              <button
                onClick={onClose}
                className="min-h-[36px] px-4 py-2 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] text-[#f8fafc] rounded-md text-xs font-bold transition"
              >
                Compris
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
