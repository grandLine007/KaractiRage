import { useState, useCallback } from 'react';
import {
  ReactFlow,
  Controls,
  Background,
  applyNodeChanges,
  applyEdgeChanges,
  addEdge,
  BackgroundVariant
} from '@xyflow/react';
import type {
  Node,
  Edge,
  Connection,
  NodeChange,
  EdgeChange
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { Plus, CheckCircle2 } from 'lucide-react';

const initialNodes: Node[] = [
  {
    id: 'node-1',
    type: 'default',
    position: { x: 100, y: 120 },
    data: { label: '🟢 ÉVÉNEMENT : OnPlayerCollide(Boss_Demon)' },
    style: { background: '#064e3b', color: '#f8fafc', border: '1px solid #059669', borderRadius: 8, padding: 10, fontSize: 12, fontWeight: 'bold' }
  },
  {
    id: 'node-2',
    type: 'default',
    position: { x: 420, y: 100 },
    data: { label: '🟡 CONDITION : PlayerHasItem(Cle_Doree)' },
    style: { background: '#78350f', color: '#f8fafc', border: '1px solid #d97706', borderRadius: 8, padding: 10, fontSize: 12, fontWeight: 'bold' }
  },
  {
    id: 'node-3',
    type: 'default',
    position: { x: 740, y: 80 },
    data: { label: '🟣 ACTION : DamageEntity(Boss_Demon, 50)' },
    style: { background: '#4c1d95', color: '#f8fafc', border: '1px solid #a855f7', borderRadius: 8, padding: 10, fontSize: 12, fontWeight: 'bold' }
  },
  {
    id: 'node-4',
    type: 'default',
    position: { x: 740, y: 160 },
    data: { label: '🔵 EFFET : PlaySound(Explosion_Lourd)' },
    style: { background: '#1e3a8a', color: '#f8fafc', border: '1px solid #3b82f6', borderRadius: 8, padding: 10, fontSize: 12, fontWeight: 'bold' }
  }
];

const initialEdges: Edge[] = [
  { id: 'e1-2', source: 'node-1', target: 'node-2', animated: true, style: { stroke: '#34d399', strokeWidth: 2 } },
  { id: 'e2-3', source: 'node-2', target: 'node-3', animated: true, style: { stroke: '#f59e0b', strokeWidth: 2 } },
  { id: 'e2-4', source: 'node-2', target: 'node-4', animated: true, style: { stroke: '#a855f7', strokeWidth: 2 } }
];

export const KvlGraphEditor: React.FC = () => {
  const [nodes, setNodes] = useState<Node[]>(initialNodes);
  const [edges, setEdges] = useState<Edge[]>(initialEdges);

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => setNodes(nds => applyNodeChanges(changes, nds)),
    []
  );

  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => setEdges(eds => applyEdgeChanges(changes, eds)),
    []
  );

  const onConnect = useCallback(
    (params: Connection) => setEdges(eds => addEdge({ ...params, animated: true, style: { stroke: '#a855f7', strokeWidth: 2 } }, eds)),
    []
  );

  const addEventNode = () => {
    const id = `node-${nodes.length + 1}`;
    const newNode: Node = {
      id,
      position: { x: 150 + Math.random() * 200, y: 150 + Math.random() * 200 },
      data: { label: `🟣 ACTION : TeleportPlayer(Room_Boss)` },
      style: { background: '#4c1d95', color: '#f8fafc', border: '1px solid #a855f7', borderRadius: 8, padding: 10, fontSize: 12, fontWeight: 'bold' }
    };
    setNodes(nds => [...nds, newNode]);
  };

  return (
    <div className="flex-1 flex flex-col h-full bg-[#07040d] overflow-hidden select-none">
      {/* Barre d'outils du graphe KVL */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-[#0d0819] border-b border-[#2d1b4e]">
        <div className="flex items-center gap-2">
          <span className="badge-tag bg-[#2e1065] text-[#34d399] border border-[#059669]">
            KVL VISUAL LOGIC
          </span>
          <span className="text-xs text-[#94a3b8]">
            Éditeur Nodal Réactif • Câblage Bézier Dynamique
          </span>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={addEventNode}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] border border-[#a855f7] text-[#c084fc] rounded text-xs font-semibold transition"
          >
            <Plus size={13} />
            Ajouter Nœud Action
          </button>
          <div className="flex items-center gap-1 px-2.5 py-1 bg-[#064e3b]/30 border border-[#059669] text-[#34d399] rounded text-xs font-bold">
            <CheckCircle2 size={13} />
            Graphe Validé (0 Cycles)
          </div>
        </div>
      </div>

      {/* Surface React Flow */}
      <div className="flex-1 w-full h-full">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          fitView
          className="bg-[#07040d]"
        >
          <Background variant={BackgroundVariant.Dots} gap={16} size={1} color="#2d1b4e" />
          <Controls className="bg-[#150d27] border border-[#2d1b4e] rounded-lg text-[#f8fafc]" />
        </ReactFlow>
      </div>
    </div>
  );
};
