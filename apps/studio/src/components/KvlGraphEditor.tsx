import { useCallback, useEffect, useState } from 'react';
import {
  ReactFlow, Controls, Background, BackgroundVariant, applyNodeChanges,
} from '@xyflow/react';
import type { Node, Edge, NodeChange } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import {
  Coins, Flame, Zap, CircleDot, Package, DoorOpen, CheckCircle2, AlertTriangle,
  Link2, GitFork, Wand2, Boxes, Search, Plus,
} from 'lucide-react';
import {
  getKvlGraphs, getKvlGraph, buildGuidedGraph, attachScript, getSelectedEntity,
  getEditorState, validateKvlGraph, getPluginCatalog, addKvlNode,
  type KvlGraphSummary, type GuidedGraphResult, type KvlDiagnostic,
  type PluginCatalog, type KvlNodeSchema,
} from '../services/engineBridge';

interface KvlNodeData {
  id: string; name: string; category: unknown; description: string; kind: unknown;
  inputs: unknown[]; outputs: unknown[];
  ui: { position_x: number; position_y: number; color_tag: string | null };
}
interface KvlConnection { id: string; from_node: string; from_pin: string; to_node: string; to_pin: string }
interface KvlGraph {
  id: string; name: string; description: string; version: number;
  variables: Record<string, unknown>; nodes: Record<string, KvlNodeData>; connections: KvlConnection[];
}

type KindKey = 'Event' | 'Condition' | 'Action' | 'Composite' | 'Other';
const KIND_STYLE: Record<KindKey, { bg: string; border: string; emoji: string }> = {
  Event: { bg: '#064e3b', border: '#059669', emoji: '🟢' },
  Condition: { bg: '#78350f', border: '#d97706', emoji: '🟡' },
  Action: { bg: '#4c1d95', border: '#a855f7', emoji: '🟣' },
  Composite: { bg: '#1e3a8a', border: '#3b82f6', emoji: '🔵' },
  Other: { bg: '#1e1338', border: '#2d1b4e', emoji: '⚪' },
};

function kindOf(kind: unknown): KindKey {
  if (typeof kind === 'object' && kind !== null) {
    const keys = Object.keys(kind as object);
    if (keys.includes('Event')) return 'Event';
    if (keys.includes('Condition')) return 'Condition';
    if (keys.includes('Action')) return 'Action';
    if (keys.includes('Composite')) return 'Composite';
  }
  return 'Other';
}
function categoryLabel(cat: unknown): string {
  if (typeof cat === 'string') return cat;
  if (typeof cat === 'object' && cat !== null && (cat as { Custom?: string }).Custom) {
    return (cat as { Custom: string }).Custom;
  }
  return 'Autre';
}
function layoutNodes(graph: KvlGraph): Node[] {
  const ids = Object.keys(graph.nodes);
  const level: Record<string, number> = {};
  ids.forEach(id => { level[id] = 0; });
  for (let pass = 0; pass < 12; pass++) {
    for (const c of graph.connections) {
      if (level[c.from_node] !== undefined && level[c.to_node] !== undefined) {
        level[c.to_node] = Math.max(level[c.to_node], level[c.from_node] + 1);
      }
    }
  }
  const byLevel: Record<number, string[]> = {};
  ids.forEach(id => { const l = level[id]; (byLevel[l] ??= []).push(id); });
  const nodes: Node[] = [];
  Object.entries(byLevel).forEach(([l, arr]) => {
    arr.forEach((id, i) => {
      const n = graph.nodes[id];
      const st = KIND_STYLE[kindOf(n.kind)];
      nodes.push({
        id,
        position: { x: 40 + Number(l) * 280, y: 40 + i * 110 },
        data: { label: `${st.emoji} ${n.name}` },
        style: { background: st.bg, color: '#f8fafc', border: `1px solid ${st.border}`, borderRadius: 8, padding: 10, fontSize: 12, fontWeight: 600, width: 220 },
      });
    });
  });
  return nodes;
}
function layoutEdges(graph: KvlGraph): Edge[] {
  return graph.connections.map(c => ({
    id: c.id, source: c.from_node, target: c.to_node, animated: true,
    style: { stroke: '#a855f7', strokeWidth: 2 },
  }));
}

type FieldDef =
  | { key: string; label: string; type: 'text' | 'number'; default: string | number; optional?: boolean }
  | { key: string; label: string; type: 'select'; options: { v: string; l: string }[]; default: string };

interface Recipe { id: string; title: string; desc: string; icon: React.ReactNode; fields: FieldDef[] }

const RECIPES: Recipe[] = [
  {
    id: 'collectible', title: 'Ramasser', desc: 'Score + objet disparaît', icon: <Coins size={14} />,
    fields: [
      { key: 'item_name', label: "Nom de l'objet", type: 'text', default: "Pièce d'or" },
      { key: 'score_points', label: 'Points', type: 'number', default: 10 },
    ],
  },
  {
    id: 'hazard', title: 'Piège', desc: 'Dégâts au contact', icon: <Flame size={14} />,
    fields: [
      { key: 'hazard_name', label: 'Nom', type: 'text', default: 'Lave' },
      { key: 'damage_amount', label: 'Dégâts', type: 'number', default: 20 },
    ],
  },
  {
    id: 'teleport', title: 'Téléporteur', desc: 'Déplace le joueur', icon: <Zap size={14} />,
    fields: [
      { key: 'portal_name', label: 'Nom', type: 'text', default: 'Portail' },
      { key: 'target_x', label: 'Cible X', type: 'number', default: 400 },
      { key: 'target_y', label: 'Cible Y', type: 'number', default: 300 },
    ],
  },
  {
    id: 'switch', title: 'Dalle', desc: 'Active une porte', icon: <CircleDot size={14} />,
    fields: [
      { key: 'switch_name', label: 'Nom', type: 'text', default: 'Dalle' },
      { key: 'door_var_to_open', label: 'Variable à activer', type: 'text', default: 'door_secret' },
    ],
  },
  {
    id: 'chest', title: 'Coffre', desc: 'Récompense à débloquer', icon: <Package size={14} />,
    fields: [
      { key: 'chest_name', label: 'Nom', type: 'text', default: 'Coffre' },
      { key: 'reward_score', label: 'Points récompense', type: 'number', default: 100 },
      { key: 'required_key', label: 'Clé requise (option)', type: 'text', default: '', optional: true },
    ],
  },
  {
    id: 'door', title: 'Porte', desc: "S'ouvre sous condition", icon: <DoorOpen size={14} />,
    fields: [
      { key: 'door_name', label: 'Nom', type: 'text', default: 'Porte' },
      { key: 'trigger', label: 'Déclencheur', type: 'select', options: [{ v: 'proximity', l: 'Proximité' }, { v: 'key', l: 'Touche' }], default: 'proximity' },
      { key: 'condition', label: 'Condition', type: 'select', options: [{ v: 'none', l: 'Aucune' }, { v: 'item', l: 'Objet requis' }, { v: 'score', l: 'Score requis' }], default: 'none' },
      { key: 'score', label: 'Score requis', type: 'number', default: 10 },
    ],
  },
];

function defaultParams(recipe: Recipe): Record<string, string | number> {
  const out: Record<string, string | number> = {};
  recipe.fields.forEach(f => { out[f.key] = f.default; });
  return out;
}

export const KvlGraphEditor: React.FC = () => {
  const [tab, setTab] = useState<'recipes' | 'graphs' | 'nodes'>('recipes');
  const [graphs, setGraphs] = useState<KvlGraphSummary[]>([]);
  const [formValues, setFormValues] = useState<Record<string, Record<string, string | number>>>({});
  const [graph, setGraph] = useState<KvlGraph | null>(null);
  const [result, setResult] = useState<GuidedGraphResult | null>(null);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [scriptPath, setScriptPath] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<KvlDiagnostic[]>([]);
  const [catalog, setCatalog] = useState<PluginCatalog | null>(null);
  const [nodeQuery, setNodeQuery] = useState('');

  const refreshGraphs = useCallback(async () => {
    try {
      setGraphs(await getKvlGraphs());
    } catch {
      /* moteur non prêt */
    }
  }, []);

  const loadGraph = useCallback(async (path: string) => {
    try {
      const json = await getKvlGraph(path);
      if (json) {
        const g = JSON.parse(json) as KvlGraph;
        setGraph(g);
        setNodes(layoutNodes(g));
        setEdges(layoutEdges(g));
        setScriptPath(path);
        setResult(null);
        setFeedback(null);
        try {
          setDiagnostics(await validateKvlGraph(json));
        } catch {
          setDiagnostics([]);
        }
      }
    } catch {
      setFeedback('Impossible de charger ce graphe.');
    }
  }, []);

  // Le catalogue peut arriver après le montage (boot moteur asynchrone) : on retente
  // à l'ouverture de l'onglet Nœuds s'il est encore absent.
  useEffect(() => {
    if (tab !== 'nodes' || catalog) return;
    void (async () => {
      try {
        setCatalog(await getPluginCatalog());
      } catch {
        /* catalogue indisponible */
      }
    })();
  }, [tab, catalog]);

  // À l'ouverture : charge le graphe de l'objet sélectionné, sinon le premier graphe.
  useEffect(() => {
    void refreshGraphs();
    void (async () => {
      try {
        setCatalog(await getPluginCatalog());
      } catch {
        /* catalogue indisponible */
      }
    })();
    void (async () => {
      try {
        const state = await getEditorState();
        const sel = state.entities.find(e => e.id === state.selected_id);
        if (sel?.script_path) {
          await loadGraph(sel.script_path);
          return;
        }
      } catch {
        /* moteur non prêt */
      }
      try {
        const list = await getKvlGraphs();
        if (list.length > 0) await loadGraph(list[0].path);
      } catch {
        /* aucun graphe */
      }
    })();
  }, [refreshGraphs, loadGraph]);

  const setField = (recipeId: string, key: string, value: string | number) => {
    setFormValues(prev => ({
      ...prev,
      [recipeId]: { ...(prev[recipeId] ?? defaultParams(RECIPES.find(r => r.id === recipeId)!)), [key]: value },
    }));
  };

  const generate = async (recipe: Recipe) => {
    setBusy(true);
    setFeedback(null);
    const params: Record<string, unknown> = {};
    const values = formValues[recipe.id] ?? defaultParams(recipe);
    recipe.fields.forEach(f => {
      const v = values[f.key];
      if (f.type === 'number') params[f.key] = Number(v);
      else if (f.type === 'text' && f.optional === true && v === '') params[f.key] = null;
      else params[f.key] = v;
    });
    try {
      const res = await buildGuidedGraph(recipe.id, params);
      setResult(res);
      if (res.ok && res.graph_json) {
        const g = JSON.parse(res.graph_json) as KvlGraph;
        setGraph(g);
        setNodes(layoutNodes(g));
        setEdges(layoutEdges(g));
        setScriptPath(res.script_path ?? null);
        setDiagnostics(res.diagnostics ?? []);
        setFeedback(res.attached
          ? `Mécanique attachée à « ${res.target_entity} ».`
          : 'Mécanique générée (aucun objet sélectionné, un objet a été créé).');
      } else {
        setFeedback(res.error ?? 'Échec de la génération.');
      }
      void refreshGraphs();
    } catch (e) {
      setFeedback((e as Error)?.message ?? 'Erreur moteur.');
    } finally {
      setBusy(false);
    }
  };

  const attach = async () => {
    if (!scriptPath) {
      setFeedback('Choisissez ou générez d\'abord un graphe.');
      return;
    }
    const id = await getSelectedEntity();
    if (id == null) {
      setFeedback('Aucun objet sélectionné : revenez au Studio (Tab) et cliquez sur un objet.');
      return;
    }
    const ok = await attachScript(id, scriptPath);
    setFeedback(ok ? `Script attaché à l'objet #${id}.` : "Échec de l'attachement.");
  };

  const addNode = async (schema: KvlNodeSchema) => {
    if (!graph || !scriptPath) {
      setFeedback("Chargez d'abord un graphe (onglet Graphes) pour insérer un nœud.");
      return;
    }
    setBusy(true);
    setFeedback(null);
    try {
      const count = Object.keys(graph.nodes).length;
      const x = 40 + (count % 3) * 280;
      const y = 40 + Math.floor(count / 3) * 110;
      const json = await addKvlNode(scriptPath, schema.id, x, y);
      const next = JSON.parse(json) as KvlGraph;
      setGraph(next);
      setNodes(layoutNodes(next));
      setEdges(layoutEdges(next));
      setDiagnostics(await validateKvlGraph(json));
      setFeedback(`Nœud « ${schema.label} » ajouté (${schema.id}).`);
      void refreshGraphs();
    } catch (e) {
      setFeedback(typeof e === 'string' ? e : "Échec de l'ajout du nœud.");
    } finally {
      setBusy(false);
    }
  };

  const onNodesChange = useCallback((changes: NodeChange[]) => {
    setNodes(nds => applyNodeChanges(changes, nds));
  }, []);

  const errCount = diagnostics.filter(d => d.severity === 'error').length;
  const warnCount = diagnostics.filter(d => d.severity === 'warning').length;

  const catalogNodes = catalog?.nodes ?? [];
  const normalizedQuery = nodeQuery.trim().toLowerCase();
  const filteredSchemas = normalizedQuery
    ? catalogNodes.filter(schema =>
        `${schema.id} ${schema.label} ${schema.description}`.toLowerCase().includes(normalizedQuery))
    : catalogNodes;
  const groupedSchemas = filteredSchemas.reduce<Record<string, KvlNodeSchema[]>>((acc, schema) => {
    const key = categoryLabel(schema.category);
    (acc[key] ??= []).push(schema);
    return acc;
  }, {});
  const orderedGroups = Object.entries(groupedSchemas).sort(([a], [b]) => a.localeCompare(b));

  return (
    <div className="flex flex-col h-full bg-[#07040d] overflow-hidden select-none">
      <div className="flex items-center justify-between px-4 py-2 bg-[#0d0819] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2">
          <span className="badge-tag bg-[#2e1065] text-[#34d399] border border-[#059669]">KVL VISUAL LOGIC</span>
          <span className="text-xs text-[#94a3b8]">Recettes guidées → graphe valide, sans connaître KVL</span>
        </div>
        <div className="flex items-center gap-2">
          {graph && (
            <span className={`badge-tag ${errCount === 0 ? 'bg-[#064e3b] text-[#34d399]' : 'bg-[#881337] text-[#f43f5e]'}`}>
              {errCount === 0 ? `✓ Valide (${warnCount} avert.)` : `✗ ${errCount} erreur(s)`}
            </span>
          )}
          <button
            onClick={attach}
            disabled={!scriptPath}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-semibold transition ${scriptPath ? 'bg-[#1e1338] hover:bg-[#2d1b4e] border border-[#a855f7] text-[#c084fc]' : 'bg-[#150d27] text-[#94a3b8] cursor-not-allowed'}`}
            title="Attacher ce script à l'objet sélectionné"
          >
            <Link2 size={13} />
            Attacher à la sélection
          </button>
        </div>
      </div>

      <div className="flex flex-1 overflow-hidden">
        <aside data-testid="kv-palette" className="w-72 flex flex-col h-full bg-[#0d0819] border-r border-[#2d1b4e] shrink-0">
          <div className="flex border-b border-[#2d1b4e]">
            <button
              onClick={() => setTab('recipes')}
              className={`flex-1 px-3 py-2 text-xs font-semibold transition ${tab === 'recipes' ? 'bg-[#150d27] text-[#f8fafc] border-b-2 border-[#a855f7]' : 'text-[#94a3b8] hover:text-[#cbd5e1]'}`}
            >
              <Wand2 size={12} className="inline mr-1" />
              Recettes
            </button>
            <button
              onClick={() => setTab('graphs')}
              className={`flex-1 px-3 py-2 text-xs font-semibold transition ${tab === 'graphs' ? 'bg-[#150d27] text-[#f8fafc] border-b-2 border-[#a855f7]' : 'text-[#94a3b8] hover:text-[#cbd5e1]'}`}
            >
              <GitFork size={12} className="inline mr-1" />
              Graphes ({graphs.length})
            </button>
            <button
              onClick={() => setTab('nodes')}
              title="Palette générée depuis le catalogue de nœuds du moteur"
              className={`flex-1 px-2 py-2 text-xs font-semibold transition ${tab === 'nodes' ? 'bg-[#150d27] text-[#f8fafc] border-b-2 border-[#a855f7]' : 'text-[#94a3b8] hover:text-[#cbd5e1]'}`}
            >
              <Boxes size={12} className="inline mr-1" />
              Nœuds ({catalog?.node_count ?? 0})
            </button>
          </div>

          <div className="flex-1 overflow-y-auto p-2 space-y-2">
            {tab === 'nodes' && (
              <div className="flex flex-col gap-2">
                <div className="relative">
                  <Search size={12} className="absolute left-2 top-2.5 text-[#94a3b8]" />
                  <input
                    value={nodeQuery}
                    onChange={e => setNodeQuery(e.target.value)}
                    placeholder="Rechercher un nœud…"
                    className="w-full text-[11px] py-1.5 pl-7 pr-2 bg-[#07040d] border border-[#2d1b4e] rounded focus:border-[#a855f7]"
                  />
                </div>
                {!scriptPath && (
                  <p className="text-[10px] text-[#f59e0b] leading-relaxed">
                    Chargez un graphe dans l'onglet « Graphes » pour activer l'insertion.
                  </p>
                )}
                {orderedGroups.map(([category, schemas]) => (
                  <div key={category} className="space-y-1">
                    <div className="text-[10px] uppercase tracking-wide text-[#94a3b8] px-0.5">{category}</div>
                    {schemas.map(schema => (
                      <button
                        key={schema.id}
                        onClick={() => void addNode(schema)}
                        disabled={busy || !scriptPath}
                        title={schema.description}
                        className="w-full text-left px-2 py-1.5 rounded bg-[#150d27] hover:bg-[#2d1b4e] border border-[#2d1b4e] hover:border-[#a855f7] disabled:opacity-40 disabled:cursor-not-allowed transition"
                      >
                        <div className="flex items-center gap-1.5">
                          <span className="text-[9px] px-1 rounded bg-[#2e1065] text-[#c084fc] uppercase">{schema.flavor}</span>
                          <span className="text-[11px] text-[#f8fafc] truncate">{schema.label}</span>
                          <Plus size={10} className="ml-auto text-[#a855f7] shrink-0" />
                        </div>
                        <div className="text-[9px] text-[#94a3b8] truncate mt-0.5">
                          {schema.id}{schema.default_argument ? `:${schema.default_argument}` : ''}
                        </div>
                      </button>
                    ))}
                  </div>
                ))}
                {filteredSchemas.length === 0 && (
                  <p className="p-3 text-center text-[11px] text-[#94a3b8]">Aucun nœud ne correspond.</p>
                )}
              </div>
            )}

            {tab === 'recipes' && RECIPES.map(r => (
              <div key={r.id} className="p-2.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg space-y-2">
                <div className="flex items-center gap-1.5 text-xs font-bold text-[#f8fafc]">
                  <span className="text-[#a855f7]">{r.icon}</span>
                  {r.title}
                  <span className="text-[10px] text-[#94a3b8] font-normal ml-auto">{r.desc}</span>
                </div>
                <div className="space-y-1.5">
                  {r.fields.map(f => {
                    const values = formValues[r.id] ?? defaultParams(r);
                    if (f.type === 'select') {
                      return (
                        <div key={f.key}>
                          <label className="text-[10px] text-[#94a3b8] block mb-0.5">{f.label}</label>
                          <select
                            value={String(values[f.key])}
                            onChange={e => setField(r.id, f.key, e.target.value)}
                            className="w-full text-[11px] py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded focus:border-[#a855f7]"
                          >
                            {f.options.map(o => <option key={o.v} value={o.v}>{o.l}</option>)}
                          </select>
                        </div>
                      );
                    }
                    return (
                      <div key={f.key}>
                        <label className="text-[10px] text-[#94a3b8] block mb-0.5">{f.label}</label>
                        <input
                          type={f.type}
                          value={values[f.key]}
                          onChange={e => setField(r.id, f.key, f.type === 'number' ? Number(e.target.value) : e.target.value)}
                          className="w-full text-[11px] py-1 px-1.5 bg-[#07040d] border border-[#2d1b4e] rounded focus:border-[#a855f7]"
                        />
                      </div>
                    );
                  })}
                </div>
                <button
                  onClick={() => generate(r)}
                  disabled={busy}
                  className="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] disabled:opacity-50 text-white rounded-md text-xs font-semibold transition"
                >
                  <Wand2 size={13} />
                  {busy ? 'Génération…' : 'Générer & attacher'}
                </button>
              </div>
            ))}

            {tab === 'graphs' && graphs.map(g => (
              <button
                key={g.path}
                onClick={() => loadGraph(g.path)}
                className="w-full text-left p-2.5 bg-[#150d27] hover:bg-[#1e1338] border border-[#2d1b4e] hover:border-[#a855f7] rounded-lg transition"
              >
                <div className="text-xs font-bold text-[#f8fafc] truncate">{g.name}</div>
                <div className="text-[10px] text-[#94a3b8] truncate mt-0.5">{g.path}</div>
                <div className="text-[10px] text-[#94a3b8] mt-1">{g.node_count} nœuds · {g.connection_count} liens</div>
              </button>
            ))}

            {tab === 'graphs' && graphs.length === 0 && (
              <div className="p-4 text-center text-xs text-[#94a3b8] leading-relaxed">
                Aucun graphe chargé. Générez une recette pour en créer un.
              </div>
            )}
          </div>
        </aside>

        <main data-testid="kv-canvas" className="flex-1 h-full">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            fitView
            className="bg-[#07040d]"
          >
            <Background variant={BackgroundVariant.Dots} gap={16} size={1} color="#2d1b4e" />
            <Controls className="bg-[#150d27] border border-[#2d1b4e] rounded-lg text-[#f8fafc]" />
          </ReactFlow>
        </main>

        <aside className="w-80 flex flex-col h-full bg-[#0d0819] border-l border-[#2d1b4e] overflow-y-auto shrink-0">
          {graph ? (
            <div className="p-3 space-y-3 text-xs">
              <h3 className="font-bold text-sm text-[#f8fafc]">{graph.name}</h3>
              <p className="text-[#94a3b8] leading-relaxed">{graph.description}</p>

              {result?.summary && (
                <div className="p-2.5 bg-[#2e1065]/40 border border-[#a855f7] rounded-lg">
                  <div className="text-[10px] font-bold uppercase tracking-wide text-[#c084fc] mb-1">Traduction</div>
                  <div className="text-[#f3e8ff] leading-relaxed">{result.summary}</div>
                </div>
              )}

              {feedback && (
                <div className={`p-2.5 rounded-lg border ${result?.ok === false ? 'bg-[#881337]/30 border-[#e11d48] text-[#fecdd3]' : 'bg-[#064e3b]/30 border-[#059669] text-[#a7f3d0]'}`}>
                  {feedback}
                </div>
              )}

              {diagnostics.length > 0 && (
                <div className="space-y-1.5">
                  <div className="text-[10px] font-bold uppercase tracking-wide text-[#94a3b8]">Diagnostics</div>
                  {diagnostics.map((d, i) => (
                    <div key={i} className="flex items-start gap-1.5 text-[11px]">
                      {d.severity === 'error'
                        ? <AlertTriangle size={12} className="text-[#f43f5e] shrink-0 mt-0.5" />
                        : <CheckCircle2 size={12} className="text-[#f59e0b] shrink-0 mt-0.5" />}
                      <span className={d.severity === 'error' ? 'text-[#fecdd3]' : 'text-[#fcd34d]'}>{d.message}</span>
                    </div>
                  ))}
                </div>
              )}

              <div>
                <div className="text-[10px] font-bold uppercase tracking-wide text-[#94a3b8] mb-1.5">
                  Nœuds ({Object.keys(graph.nodes).length})
                </div>
                <div className="space-y-1.5">
                  {Object.values(graph.nodes).map(n => {
                    const st = KIND_STYLE[kindOf(n.kind)];
                    return (
                      <div key={n.id} className="p-2 bg-[#150d27] border border-[#2d1b4e] rounded-lg">
                        <div className="flex items-center gap-1.5 font-semibold text-[#cbd5e1]">
                          <span>{st.emoji}</span>
                          {n.name}
                        </div>
                        <div className="text-[10px] text-[#94a3b8] mt-0.5">
                          {categoryLabel(n.category)} · {n.description}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center h-full p-6 text-center text-xs text-[#94a3b8] gap-2">
              <GitFork size={28} className="text-[#2d1b4e]" />
              <span className="font-semibold text-[#94a3b8]">Aucun graphe affiché</span>
              <p className="leading-relaxed">
                Choisissez une recette à gauche, remplissez 2-3 champs, puis générez une mécanique valide.
              </p>
            </div>
          )}
        </aside>
      </div>
    </div>
  );
};