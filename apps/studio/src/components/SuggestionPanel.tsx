import { useMemo, useState } from 'react';
import { Lightbulb, ArrowRight, CheckCircle2, Circle, ChevronDown, ChevronUp } from 'lucide-react';
import type { StudioEntitySummary } from '../services/engineBridge';

interface SuggestionPanelProps {
  entities: StudioEntitySummary[];
  selectedEntityId: number | null;
  onSpawn: (preset: string) => void;
  onOpenKvl: () => void;
  onTestGame: () => void;
}

interface Suggestion {
  id: string;
  title: string;
  detail: string;
  action: () => void;
  tone: 'purple' | 'amber' | 'cyan' | 'rose';
}

const TONE = {
  purple: 'border-[#a855f7] bg-[#2e1065]/40 text-[#c084fc]',
  amber: 'border-[#f59e0b] bg-[#78350f]/40 text-[#fcd34d]',
  cyan: 'border-[#38bdf8] bg-[#0c4a6e]/40 text-[#7dd3fc]',
  rose: 'border-[#f43f5e] bg-[#881337]/40 text-[#fda4af]',
};

const LS_KEY = 'karacti.checklist.v1';

/**
 * Suggestions contextuelles déterministes (aucun LLM) : guide l'utilisateur
 * vers la prochaine action utile et tient une checklist de prise en main.
 */
export const SuggestionPanel: React.FC<SuggestionPanelProps> = ({
  entities,
  selectedEntityId,
  onSpawn,
  onOpenKvl,
  onTestGame,
}) => {
  const [collapsed, setCollapsed] = useState(false);
  const [done, setDone] = useState<Record<string, boolean>>(() => {
    try {
      return JSON.parse(localStorage.getItem(LS_KEY) ?? '{}');
    } catch {
      return {};
    }
  });

  const markDone = (key: string) => {
    setDone(prev => {
      const next = { ...prev, [key]: true };
      localStorage.setItem(LS_KEY, JSON.stringify(next));
      return next;
    });
  };

  const hasAny = (tags: string[]) => entities.some(e => tags.includes(e.tag));

  const nextStep = useMemo<Suggestion>(() => {
    if (entities.length === 0) {
      return { id: 'spawn-first', title: 'Ajoutez un premier objet', detail: 'La scène est vide : placez une pièce pour démarrer.', action: () => onSpawn('coin'), tone: 'purple' };
    }
    if (!hasAny(['collectible', 'coin'])) {
      return { id: 'spawn-coin', title: 'Ajoutez des pièces à ramasser', detail: 'Elles donnent du score et structurent le niveau.', action: () => onSpawn('coin'), tone: 'amber' };
    }
    if (!hasAny(['enemy', 'boss'])) {
      return { id: 'spawn-boss', title: 'Ajoutez un ennemi ou un boss', detail: 'Créez du défi pour le joueur.', action: () => onSpawn('boss'), tone: 'rose' };
    }
    if (!hasAny(['door', 'portal', 'plate'])) {
      return { id: 'spawn-door', title: 'Ajoutez une sortie ou un mécanisme', detail: 'Une porte, un portail ou une dalle complète le niveau.', action: () => onSpawn('door'), tone: 'cyan' };
    }
    if (!hasAny(['hazard'])) {
      return { id: 'spawn-hazard', title: 'Ajoutez un piège', detail: 'Un danger rend la progression plus intéressante.', action: () => onSpawn('spikes'), tone: 'rose' };
    }
    return { id: 'kvl', title: 'Ajoutez une règle de logique', detail: 'Reliez les objets avec une mécanique KVL en 1 clic.', action: onOpenKvl, tone: 'purple' };
  }, [entities, onSpawn, onOpenKvl]);

  const checklist = useMemo(() => {
    return [
      { key: 'add-object', label: 'Ajouter un objet', done: entities.length > 0 },
      { key: 'select-object', label: 'Sélectionner un objet', done: selectedEntityId !== null },
      { key: 'challenge', label: 'Créer du défi (ennemi/piège)', done: hasAny(['enemy', 'boss', 'hazard']) },
    ];
  }, [entities, selectedEntityId]);

  return (
    <div className="border-t border-[#2d1b4e] bg-[#0d0819] select-none">
      <button
        onClick={() => setCollapsed(c => !c)}
        className="w-full flex items-center justify-between px-3 py-2 text-[#94a3b8] hover:text-[#f8fafc] transition"
      >
        <span className="flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider">
          <Lightbulb size={12} className="text-[#f59e0b]" />
          Conseils
        </span>
        {collapsed ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
      </button>

      {!collapsed && (
        <div className="px-2 pb-2 space-y-2">
          <button
            onClick={() => { nextStep.action(); if (nextStep.id === 'kvl') markDone('kvl-rule'); }}
            className={`w-full text-left p-2.5 rounded-lg border transition ${TONE[nextStep.tone]} hover:brightness-110`}
          >
            <div className="text-[10px] font-bold uppercase tracking-wide mb-0.5">Prochaine étape</div>
            <div className="text-xs font-bold flex items-center gap-1.5">
              {nextStep.title}
              <ArrowRight size={12} className="shrink-0" />
            </div>
            <div className="text-[10px] mt-0.5 opacity-80 leading-relaxed">{nextStep.detail}</div>
          </button>

          <div className="space-y-1">
            {checklist.map(item => (
              <div key={item.key} className="flex items-center gap-1.5 text-[11px]">
                {item.done ? (
                  <CheckCircle2 size={13} className="text-[#34d399] shrink-0" />
                ) : (
                  <Circle size={13} className="text-[#2d1b4e] shrink-0" />
                )}
                <span className={item.done ? 'text-[#94a3b8]' : 'text-[#94a3b8]'}>{item.label}</span>
              </div>
            ))}

            <div className="flex items-center gap-1.5 text-[11px]">
              {done['kvl-rule'] ? (
                <CheckCircle2 size={13} className="text-[#34d399] shrink-0" />
              ) : (
                <Circle size={13} className="text-[#2d1b4e] shrink-0" />
              )}
              <button
                onClick={() => { onOpenKvl(); markDone('kvl-rule'); }}
                className={done['kvl-rule'] ? 'text-[#94a3b8]' : 'text-[#c084fc] underline-offset-2 hover:underline'}
              >
                Ajouter une règle KVL
              </button>
            </div>

            <div className="flex items-center gap-1.5 text-[11px]">
              {done['test'] ? (
                <CheckCircle2 size={13} className="text-[#34d399] shrink-0" />
              ) : (
                <Circle size={13} className="text-[#2d1b4e] shrink-0" />
              )}
              <button
                onClick={() => { onTestGame(); markDone('test'); }}
                className={done['test'] ? 'text-[#94a3b8]' : 'text-[#38bdf8] underline-offset-2 hover:underline'}
              >
                Tester le jeu [Tab]
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};