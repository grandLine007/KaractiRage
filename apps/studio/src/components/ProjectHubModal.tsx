import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle, Archive, ArrowRight, Bot, Clock, Copy, Cpu, FolderOpen, FolderSearch,
  Gamepad2, HardDrive, Loader2, Pencil, Play, Plus, RefreshCw, Search, Sparkles,
  Wrench, X,
} from 'lucide-react';
import {
  archiveProject, createProject, duplicateProject, isTauri, listProjects, openProject,
  removeRecent, renameProject, revealProject, systemDiagnostics,
  type NewProjectConfig, type ProjectEntry, type SystemDiagnostics,
} from '../services/projectBridge';
import { loadProjectJson } from '../services/engineBridge';
import { ARCHETYPES } from '../services/archetypes';

interface ProjectHubModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenArchetypes: () => void;
  onOpenWizard: () => void;
  onOpenAiTab: () => void;
  onProjectOpened: (entry: ProjectEntry, mode: 'play' | 'edit') => void;
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

type Filter = 'all' | 'scene' | 'project' | 'recent';
type SortKey = 'date' | 'name' | 'genre';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} Ko`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
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

function SceneThumbnail({ entry }: { entry: ProjectEntry }) {
  const color = genreColor(entry.genre);
  const seed = hashString(entry.path || entry.title);
  const bars = useMemo(() => {
    const n = Math.min(10, 4 + (entry.entity_count % 7));
    return Array.from({ length: n }, (_, i) => {
      const h = 10 + ((seed >> (i % 16)) % 42);
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
  isOpen, onClose, onOpenArchetypes, onOpenWizard, onOpenAiTab, onProjectOpened,
}) => {
  const [activeTab, setActiveTab] = useState<0 | 1 | 2>(0);
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

  useEffect(() => {
    if (isOpen) void refresh();
  }, [isOpen, refresh]);

  useEffect(() => {
    if (isOpen && activeTab === 2) {
      systemDiagnostics().then(setDiag).catch(() => setDiag(null));
    }
  }, [isOpen, activeTab]);

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

  // Derniers projets ouverts (registre "récents" Rust), du plus récent au plus ancien.
  const recents = useMemo(() => {
    return projects
      .filter(p => p.is_recent)
      .sort((a, b) => b.modified_at - a.modified_at)
      .slice(0, 6);
  }, [projects]);

  if (!isOpen) return null;

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

  return (
    <div data-testid="hub" className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md select-none">
      <div className="relative flex flex-col w-full max-w-5xl max-h-[92vh] bg-[#0d0819] border border-[#a855f7] rounded-xl shadow-2xl shadow-purple-950/90 overflow-hidden">
        {/* En-tête */}
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
            aria-label="Fermer"
          >
            <X size={18} />
          </button>
        </div>

        {/* Barre d'onglets */}
        <div className="flex items-center px-6 pt-3 bg-[#0d0819] border-b border-[#2d1b4e] gap-2">
          <TabButton active={activeTab === 0} onClick={() => setActiveTab(0)} icon={<Sparkles size={14} />}>
            Nouveau projet
          </TabButton>
          <TabButton active={activeTab === 1} onClick={() => setActiveTab(1)} icon={<FolderOpen size={14} />}>
            Projets sur disque
          </TabButton>
          <TabButton active={activeTab === 2} onClick={() => setActiveTab(2)} icon={<Bot size={14} />}>
            Outils & IA
          </TabButton>
        </div>

        {/* Bandeau d'erreur */}
        {error && (
          <div className="mx-6 mt-3 flex items-center gap-2 px-3 py-2 bg-[#3b0a0a] border border-[#ef4444] rounded text-xs text-[#fecaca]">
            <AlertTriangle size={14} />
            {error}
            <button className="ml-auto text-[#fecaca]/70 hover:text-white" onClick={() => setError('')}>
              <X size={14} />
            </button>
          </div>
        )}

        {/* Corps */}
        <div className="flex-1 overflow-y-auto p-6 space-y-5">
          {/* ============ ONGLET 1.1 : NOUVEAU PROJET ============ */}
          {activeTab === 0 && (
            <>
              {/* Derniers projets (registre "récents" Rust) — visibles dès l'ouverture */}
              <section className="space-y-3">
                <div className="flex items-center justify-between">
                  <h3 className="font-bold text-sm text-[#f8fafc] flex items-center gap-2">
                    <Clock size={15} className="text-[#c084fc]" />
                    Derniers projets
                    <span className="text-[10px] font-medium text-[#94a3b8]">({recents.length})</span>
                  </h3>
                  <button
                    onClick={() => { setFilter('recent'); setActiveTab(1); }}
                    className="text-[11px] text-[#c084fc] hover:text-[#f3e8ff] font-semibold"
                  >
                    Voir tout →
                  </button>
                </div>

                {loading ? (
                  <p className="text-xs text-[#94a3b8] flex items-center gap-2">
                    <Loader2 size={14} className="animate-spin" /> Chargement des projets…
                  </p>
                ) : recents.length === 0 ? (
                  <p className="text-xs text-[#94a3b8] px-3 py-4 bg-[#150d27] border border-dashed border-[#2d1b4e] rounded-lg">
                    Aucun projet récent pour l'instant — créez un projet ci-dessous, ou
                    ouvrez-en un depuis l'onglet « Projets sur disque ».
                  </p>
                ) : (
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    {recents.map(entry => (
                      <div
                        key={entry.path}
                        className="flex items-center gap-3 p-3 bg-[#150d27] border border-[#2d1b4e] rounded-lg hover:border-[#a855f7]/60 transition"
                      >
                        <button
                          onClick={() => openEntry(entry, 'play')}
                          className="flex items-center gap-3 flex-1 min-w-0 text-left"
                          title={`Jouer : ${entry.title}`}
                        >
                          <div className="w-20 h-12 shrink-0 rounded overflow-hidden border border-[#334155]">
                            <SceneThumbnail entry={entry} />
                          </div>
                          <div className="flex-1 min-w-0">
                            <div className="font-semibold text-sm text-[#f8fafc] truncate" title={entry.title}>{entry.title}</div>
                            <div className="text-[11px] text-[#94a3b8]">
                              {entry.genre} • {entry.dimension} • {formatDate(entry.modified_at)}
                            </div>
                          </div>
                        </button>
                        <div className="flex items-center gap-1 shrink-0">
                          <IconButton onClick={() => openEntry(entry, 'play')} title="Jouer"><Play size={14} /></IconButton>
                          <IconButton onClick={() => openEntry(entry, 'edit')} title="Ouvrir dans le Studio"><Wrench size={14} /></IconButton>
                          <IconButton onClick={() => onRemoveRecent(entry.path)} title="Retirer des récents"><X size={14} /></IconButton>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </section>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="p-4 bg-[#170f28] border border-[#a855f7] rounded-lg space-y-3">
                  <h3 className="font-bold text-sm text-[#f8fafc] flex items-center gap-2">
                    <Plus size={15} className="text-[#c084fc]" />
                    Créer un projet sur disque
                  </h3>
                  <label className="block text-[11px] text-[#94a3b8]">
                    Nom du projet
                    <input
                      value={form.title}
                      onChange={e => setForm(f => ({ ...f, title: e.target.value }))}
                      placeholder="Ex : Mon Donjon de Lave"
                      className="mt-1 w-full px-3 py-2 bg-[#0f172a] border border-[#334155] rounded text-sm text-[#f8fafc] focus:border-[#a855f7] outline-none"
                    />
                  </label>
                  <div className="grid grid-cols-2 gap-3">
                    <label className="block text-[11px] text-[#94a3b8]">
                      Genre
                      <select
                        value={form.genre}
                        onChange={e => setForm(f => ({ ...f, genre: e.target.value }))}
                        className="mt-1 w-full px-2 py-2 bg-[#0f172a] border border-[#334155] rounded text-sm text-[#f8fafc] outline-none"
                      >
                        {GENRES.map(g => <option key={g} value={g}>{g}</option>)}
                      </select>
                    </label>
                    <label className="block text-[11px] text-[#94a3b8]">
                      Dimension
                      <select
                        value={form.dimension}
                        onChange={e => setForm(f => ({ ...f, dimension: e.target.value }))}
                        className="mt-1 w-full px-2 py-2 bg-[#0f172a] border border-[#334155] rounded text-sm text-[#f8fafc] outline-none"
                      >
                        {DIMENSIONS.map(d => <option key={d} value={d}>{d}</option>)}
                      </select>
                    </label>
                  </div>
                  <label className="block text-[11px] text-[#94a3b8]">
                    Modèle de départ (optionnel)
                    <select
                      value={form.archetype}
                      onChange={e => setForm(f => ({ ...f, archetype: e.target.value }))}
                      className="mt-1 w-full px-2 py-2 bg-[#0f172a] border border-[#334155] rounded text-sm text-[#f8fafc] outline-none"
                    >
                      <option value="">Projet vierge (Studio)</option>
                      {ARCHETYPES.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
                    </select>
                  </label>
                  <label className="block text-[11px] text-[#94a3b8]">
                    Dossier de destination (vide = défaut)
                    <input
                      value={form.destination}
                      onChange={e => setForm(f => ({ ...f, destination: e.target.value }))}
                      placeholder="~/.karactirage/projects"
                      className="mt-1 w-full px-3 py-2 bg-[#0f172a] border border-[#334155] rounded text-sm text-[#f8fafc] focus:border-[#a855f7] outline-none"
                    />
                  </label>
                  <button
                    onClick={handleCreate}
                    disabled={creating}
                    className="w-full flex items-center justify-center gap-2 py-2.5 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] text-white rounded text-xs font-bold transition disabled:opacity-60"
                  >
                    {creating ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
                    {creating ? 'Création…' : 'Créer & ouvrir dans le Studio'}
                  </button>
                </div>

                <div className="p-4 bg-[#170f28] border border-[#a855f7]/50 rounded-lg space-y-3">
                  <h3 className="font-bold text-sm text-[#f8fafc] flex items-center gap-2">
                    <Wrench size={15} className="text-[#c084fc]" />
                    Assistant guidé 6 étapes
                  </h3>
                  <p className="text-xs text-[#94a3b8]">
                    Configurez genres, héros, physique et gameplay pas à pas, puis générez la
                    recette KVL complète.
                  </p>
                  <div className="flex gap-2">
                    <button
                      onClick={onOpenWizard}
                      className="flex-1 flex items-center justify-center gap-1.5 py-2 bg-[#7e22ce] hover:bg-[#9333ea] text-white rounded text-xs font-bold transition"
                    >
                      Assistant [W] <ArrowRight size={13} />
                    </button>
                    <button
                      onClick={onOpenAiTab}
                      className="flex-1 flex items-center justify-center gap-1.5 py-2 bg-[#2e1065] hover:bg-[#3b0764] text-[#c084fc] border border-[#a855f7] rounded text-xs font-semibold transition"
                    >
                      <Bot size={13} /> Intention IA [I]
                    </button>
                  </div>
                </div>
              </div>

              <div>
                <button
                  onClick={onOpenArchetypes}
                  className="w-full flex items-center justify-between gap-3 p-4 bg-gradient-to-r from-[#170f28] to-[#2e1065] border border-[#a855f7]/60 rounded-lg hover:border-[#a855f7] transition group"
                >
                  <div className="flex items-center gap-3 text-left">
                    <Gamepad2 size={22} className="text-[#38bdf8] shrink-0" />
                    <div>
                      <h4 className="text-sm font-bold text-[#f8fafc]">5 archétypes de jeu en 1 clic</h4>
                      <p className="text-[11px] text-[#94a3b8]">
                        RPG · Platformer · Shmup · Sokoban · Titan Boss — lancez un jeu de référence complet en un clic.
                      </p>
                    </div>
                  </div>
                  <span className="shrink-0 px-3 py-1.5 bg-[#7e22ce] group-hover:bg-[#9333ea] text-white rounded text-xs font-bold transition flex items-center gap-1">
                    Ouvrir la galerie <ArrowRight size={13} />
                  </span>
                </button>
              </div>
            </>
          )}

          {/* ============ ONGLET 1.2 : PROJETS SUR DISQUE ============ */}
          {activeTab === 1 && (
            <>
              <div className="flex flex-wrap items-center gap-3">
                <div className="relative flex-1 min-w-[200px]">
                  <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-[#94a3b8]" />
                  <input
                    value={query}
                    onChange={e => setQuery(e.target.value)}
                    placeholder="Rechercher un projet ou une scène…"
                    className="w-full pl-8 pr-3 py-2 bg-[#0f172a] border border-[#334155] rounded text-sm text-[#f8fafc] focus:border-[#a855f7] outline-none"
                  />
                </div>
                <div className="flex items-center gap-1">
                  {(['all', 'scene', 'project', 'recent'] as Filter[]).map(f => (
                    <button
                      key={f}
                      onClick={() => setFilter(f)}
                      className={`px-2.5 py-1.5 rounded text-[11px] font-semibold border transition ${
                        filter === f
                          ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7]'
                          : 'bg-[#0f172a] text-[#94a3b8] border-[#334155] hover:text-[#f8fafc]'
                      }`}
                    >
                      {f === 'all' ? 'Tous' : f === 'scene' ? 'Scènes' : f === 'project' ? 'Projets' : 'Récents'}
                    </button>
                  ))}
                </div>
                <select
                  value={sort}
                  onChange={e => setSort(e.target.value as SortKey)}
                  className="px-2 py-2 bg-[#0f172a] border border-[#334155] rounded text-sm text-[#f8fafc] outline-none"
                >
                  <option value="date">Tri : Date</option>
                  <option value="name">Tri : Nom</option>
                  <option value="genre">Tri : Genre</option>
                </select>
                <button
                  onClick={() => void refresh()}
                  className="p-2 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#c084fc] rounded transition"
                  title="Actualiser"
                >
                  <RefreshCw size={14} className={loading ? 'animate-spin' : ''} />
                </button>
              </div>

              {confirmArchive && (
                <div className="flex items-center gap-3 px-4 py-2.5 bg-[#2b0f0f] border border-[#ef4444] rounded text-xs">
                  <AlertTriangle size={15} className="text-[#ef4444]" />
                  <span className="text-[#fecaca]">
                    Archiver ce projet ? Il sera déplacé vers l'archive (aucune suppression).
                  </span>
                  <button
                    onClick={() => void onArchive(confirmArchive)}
                    className="ml-auto px-3 py-1 bg-[#ef4444] hover:bg-[#dc2626] text-white rounded font-bold"
                  >
                    Confirmer
                  </button>
                  <button
                    onClick={() => setConfirmArchive(null)}
                    className="px-3 py-1 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] rounded"
                  >
                    Annuler
                  </button>
                </div>
              )}

              {loading && (
                <div className="flex items-center justify-center gap-2 py-10 text-[#94a3b8] text-sm">
                  <Loader2 size={18} className="animate-spin" />
                  Scan du disque en cours…
                </div>
              )}

              {!loading && visible.length === 0 && (
                <div className="py-10 text-center bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
                  <FolderSearch size={32} className="mx-auto text-[#a855f7]" />
                  <p className="text-sm text-[#f8fafc] font-semibold">Aucun projet trouvé</p>
                  <p className="text-xs text-[#94a3b8]">
                    Créez votre premier projet depuis l'onglet 1.1, ou ajustez vos filtres.
                  </p>
                </div>
              )}

              {!loading && visible.length > 0 && (
                <div className="space-y-3">
                  {visible.map(entry => (
                    <div
                      key={entry.path}
                      className="flex items-center gap-3 p-3 bg-[#150d27] border border-[#2d1b4e] rounded-lg hover:border-[#a855f7]/60 transition"
                    >
                      <div className="w-28 h-16 shrink-0 rounded overflow-hidden border border-[#334155]">
                        <SceneThumbnail entry={entry} />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-semibold text-sm text-[#f8fafc] truncate" title={entry.title}>{entry.title}</span>
                          {entry.is_recent && (
                            <span className="text-[9px] uppercase px-1.5 py-0.5 rounded bg-[#2e1065] text-[#c084fc]">Récent</span>
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
                          ? <span className="px-2 py-1 rounded text-[10px] font-bold bg-[#3b0a0a] text-[#fecaca]">⚠ Introuvable</span>
                          : <span className="px-2 py-1 rounded text-[10px] font-bold bg-[#052e1b] text-[#d1fae5]">✓ Disponible</span>}
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
            </>
          )}

          {/* ============ ONGLET 1.3 : OUTILS & IA ============ */}
          {activeTab === 2 && (
            <>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="p-4 bg-[#170f28] border border-[#a855f7] rounded-lg space-y-3">
                  <h3 className="font-bold text-sm text-[#f8fafc] flex items-center gap-2">
                    <Bot size={15} className="text-[#c084fc]" />
                    Intention IA & génération de niveaux
                  </h3>
                  <p className="text-xs text-[#94a3b8]">
                    Connectez LM Studio, Ollama ou vos clés API pour générer des niveaux
                    complets par description textuelle en français.
                  </p>
                  <button
                    onClick={onOpenAiTab}
                    className="w-full flex items-center justify-center gap-1.5 py-2 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] text-white rounded text-xs font-bold transition"
                  >
                    <Bot size={13} /> Ouvrir la console Assistant IA
                  </button>
                </div>

                <div className="p-4 bg-[#170f28] border border-[#a855f7]/50 rounded-lg space-y-3">
                  <h3 className="font-bold text-sm text-[#f8fafc] flex items-center gap-2">
                    <HardDrive size={15} className="text-[#38bdf8]" />
                    Importer un projet / des ressources
                  </h3>
                  <p className="text-xs text-[#94a3b8]">
                    Chargez un fichier <code>.karacti</code> JSON directement dans le moteur.
                  </p>
                  <label className="block w-full text-center py-2 bg-[#2e1065] hover:bg-[#3b0764] text-[#c084fc] border border-[#a855f7] rounded text-xs font-semibold cursor-pointer transition">
                    <input
                      type="file"
                      accept=".json,.karacti,application/json"
                      className="hidden"
                      onChange={handleImport}
                    />
                    Choisir un fichier…
                  </label>
                </div>
              </div>

              <div className="p-4 bg-[#170f28] border border-[#2d1b4e] rounded-lg space-y-3">
                <h3 className="font-bold text-sm text-[#f8fafc] flex items-center gap-2">
                  <FolderOpen size={15} className="text-[#c084fc]" />
                  Tutoriels & exemples
                </h3>
                <div className="flex flex-wrap gap-2">
                  <button
                    onClick={onOpenWizard}
                    className="px-3 py-2 bg-[#7e22ce] hover:bg-[#9333ea] text-white rounded text-xs font-bold transition"
                  >
                    Assistant 6 étapes [W]
                  </button>
                  <button
                    onClick={onOpenArchetypes}
                    className="px-3 py-2 bg-[#150d27] hover:bg-[#1e1338] border border-[#a855f7] rounded text-xs font-semibold text-[#cbd5e1] transition flex items-center gap-1.5"
                  >
                    5 archétypes de jeu <ArrowRight size={12} />
                  </button>
                </div>
              </div>

              <div className="p-4 bg-[#170f28] border border-[#2d1b4e] rounded-lg space-y-3">
                <h3 className="font-bold text-sm text-[#f8fafc] flex items-center gap-2">
                  <Cpu size={15} className="text-[#38bdf8]" />
                  Diagnostics système
                </h3>
                {diag ? (
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
                    <Diag label="Système" value={`${diag.os} / ${diag.arch}`} />
                    <Diag label="Cœurs CPU" value={String(diag.cpu_cores)} />
                    <Diag label="FPS" value={diag.fps.toFixed(0)} />
                    <Diag label="Entités" value={String(diag.entities)} />
                    <Diag label="Dossier projets" value={diag.projects_home} wide />
                    <Diag label="Moteur" value={diag.engine} wide />
                  </div>
                ) : (
                  <p className="text-xs text-[#94a3b8] flex items-center gap-2">
                    <Loader2 size={14} className="animate-spin" /> Collecte des métriques…
                  </p>
                )}
                {!isTauri() && (
                  <p className="text-[11px] text-[#f59e0b]">
                    Mode web : le Hub fonctionne en lecture seule (démonstration). Lancez le
                    Studio Tauri pour la gestion complète des projets sur disque.
                  </p>
                )}
              </div>
            </>
          )}
        </div>

        {/* Barre d'action inférieure */}
        <div className="flex items-center justify-between px-6 py-3.5 bg-[#150d27] border-t border-[#2d1b4e]">
          <span className="text-xs text-[#94a3b8]">
            {isTauri()
              ? 'Projets persistés dans ~/.karactirage — Échap pour fermer'
              : 'Mode web (lecture seule) — Échap pour fermer'}
          </span>
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

function TabButton({
  active, onClick, icon, children,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-t-lg border-t border-x transition ${
        active
          ? 'bg-[#150d27] text-[#c084fc] border-[#a855f7]'
          : 'text-[#94a3b8] border-transparent hover:text-[#f8fafc]'
      }`}
    >
      {icon}
      {children}
    </button>
  );
}

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
      className={`p-1.5 rounded transition ${
        danger
          ? 'text-[#f87171] hover:bg-[#3b0a0a]'
          : 'text-[#94a3b8] hover:bg-[#1e1338] hover:text-[#f8fafc]'
      }`}
    >
      {children}
    </button>
  );
}

function Diag({ label, value, wide }: { label: string; value: string; wide?: boolean }) {
  return (
    <div
      className={`${wide ? 'col-span-2 md:col-span-4' : ''} px-3 py-2 bg-[#0f172a] border border-[#1e293b] rounded`}
    >
      <div className="text-[10px] uppercase text-[#94a3b8]">{label}</div>
      <div className="text-[#f8fafc] font-medium truncate">{value}</div>
    </div>
  );
}








