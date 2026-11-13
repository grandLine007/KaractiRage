import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, ArrowRight, CornerDownLeft, Loader2, Sparkles, X } from 'lucide-react';
import type { StudioEntitySummary } from '../services/engineBridge';
import {
  applyAiEdit,
  cancelAiEdit,
  isProposal,
  previewAiEdit,
  type AiApplyReport,
  type AiEditProposal,
  type AiRisk,
} from '../services/aiEditBridge';

type Phase = 'idle' | 'loading' | 'proposal' | 'clarification' | 'llm' | 'error';

interface Suggestion {
  label: string;
  /** Instruction réellement envoyée ; par défaut le libellé. */
  text?: string;
  /** `false` = pré-remplit le champ sans envoyer (ex. « Renommer en … »). */
  submit: boolean;
}

interface InlineAiBarProps {
  open: boolean;
  /** Champ d'où vient l'invocation (x, y, w, h, speed, name…), pour cibler les suggestions. */
  fieldHint: string | null;
  entity: StudioEntitySummary | null;
  sceneId: string;
  onClose: () => void;
  onApplied: (report: AiApplyReport) => void;
  onOpenAssistant: () => void;
}

const FIELD_LABELS: Record<string, string> = {
  x: 'position X',
  y: 'position Y',
  w: 'largeur',
  h: 'hauteur',
  speed: 'vitesse',
  name: 'nom',
  color: 'couleur',
};

const RISK_STYLE: Record<AiRisk, { label: string; className: string }> = {
  safe: { label: 'Sûr', className: 'bg-[#064e3b] text-[#34d399]' },
  confirm: { label: 'À confirmer', className: 'bg-[#451a03] text-[#f59e0b]' },
  destructive: { label: 'Destructif', className: 'bg-[#881337] text-[#fecdd3]' },
};

/** Suggestions déterministes, ordonnées selon le champ d'origine (jamais un menu générique). */
function suggestionsFor(entity: StudioEntitySummary | null, fieldHint: string | null): Suggestion[] {
  const list: Suggestion[] = [];
  const push = (label: string, submit = true, text?: string) => {
    if (!list.some(existing => existing.label === label)) {
      list.push({ label, submit, text });
    }
  };

  if (fieldHint === 'name') push('Renommer…', false, 'Renomme en ');
  if (fieldHint === 'x' || fieldHint === 'y') {
    push('Déplace à droite');
    push('Déplace à gauche');
    push('Déplace en haut');
    push('Déplace en bas');
  }
  if (fieldHint === 'w' || fieldHint === 'h') {
    push('Plus grand');
    push('Plus petit');
  }
  if (fieldHint === 'speed' && entity?.has_movement) {
    push('Plus rapide');
    push('Plus lent');
  }

  push('Rouge');
  push('Bleu');
  push('Vert');
  if (entity?.has_movement) push('Plus rapide');
  push('Plus grand');
  push('Dupliquer');
  push('Supprimer');
  return list.slice(0, 7);
}

/**
 * Barre d'édition IA ancrée (plan IA inline, surface B).
 *
 * Pas de backdrop, pas de plein écran : le viewport reste visible et interactif.
 * Le chemin est : instruction → **aperçu** (rien n'est appliqué) → Appliquer / Annuler.
 */
export const InlineAiBar: React.FC<InlineAiBarProps> = ({
  open,
  fieldHint,
  entity,
  sceneId,
  onClose,
  onApplied,
  onOpenAssistant,
}) => {
  const [instruction, setInstruction] = useState('');
  const [phase, setPhase] = useState<Phase>('idle');
  const [proposal, setProposal] = useState<AiEditProposal | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [prompt, setPrompt] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const reset = useCallback(() => {
    setInstruction('');
    setPhase('idle');
    setProposal(null);
    setNote(null);
    setPrompt(null);
  }, []);

  // L'état est réinitialisé par remontage (clé côté App) : l'effet ne fait que le focus,
  // qui est une synchronisation DOM et non un setState en cascade.
  useEffect(() => {
    if (!open) return;
    const timer = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [open]);

  const suggestions = useMemo(() => suggestionsFor(entity, fieldHint), [entity, fieldHint]);

  const runPreview = useCallback(async (text: string) => {
    const clean = text.trim();
    if (!clean) return;
    setInstruction(clean);
    setPhase('loading');
    setProposal(null);
    setNote(null);
    setPrompt(null);
    try {
      const preview = await previewAiEdit(clean);
      if (isProposal(preview)) {
        setProposal(preview.proposal);
        setPhase('proposal');
      } else if (preview.status === 'clarification') {
        setNote(preview.message);
        setPhase('clarification');
      } else {
        setPrompt(preview.prompt);
        setPhase('llm');
      }
    } catch (error) {
      setNote(error instanceof Error ? error.message : String(error));
      setPhase('error');
    }
  }, []);

  const applyProposal = useCallback(async () => {
    if (!proposal) return;
    setPhase('loading');
    try {
      const report = await applyAiEdit(proposal.id);
      onApplied(report);
    } catch (error) {
      setNote(error instanceof Error ? error.message : String(error));
      setPhase('error');
    }
  }, [proposal, onApplied]);

  const dismiss = useCallback(() => {
    if (proposal) void cancelAiEdit(proposal.id);
    reset();
    onClose();
  }, [proposal, reset, onClose]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) {
        event.preventDefault();
        dismiss();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open, dismiss]);

  if (!open) return null;

  const targetLabel = entity ? `${entity.name} #${entity.id}` : 'Aucune sélection';
  const fieldLabel = fieldHint ? (FIELD_LABELS[fieldHint] ?? fieldHint) : null;
  const changes = proposal?.preview.changes ?? [];
  const risk = proposal ? RISK_STYLE[proposal.risk] : null;

  return (
    <div
      data-testid="inline-ai-bar"
      role="region"
      aria-label="Édition IA ciblée"
      aria-busy={phase === 'loading'}
      className="absolute inset-x-0 bottom-0 z-30 border-t border-[#a855f7]/60 bg-[#0d0819]/98 backdrop-blur-sm shadow-[var(--shadow-e3)]"
    >
      {/* 1. Chips de contexte : la cible est explicite, jamais devinée. */}
      <div className="flex items-center gap-2 px-3 pt-2 text-[11px]">
        <span className="flex items-center gap-1 text-[#c084fc] font-semibold">
          <Sparkles size={12} aria-hidden="true" />
          Édition IA
        </span>
        <span
          data-testid="inline-ai-target"
          className="px-2 py-0.5 rounded-full border border-[#2d1b4e] bg-[#150d27] text-[#f8fafc] font-medium truncate max-w-[40%]"
        >
          {targetLabel}
        </span>
        <span className="px-2 py-0.5 rounded-full border border-[#2d1b4e] text-[#94a3b8] truncate max-w-[30%]">
          {sceneId}
        </span>
        {fieldLabel && (
          <span className="px-2 py-0.5 rounded-full border border-[#2d1b4e] text-[#38bdf8]">
            champ : {fieldLabel}
          </span>
        )}
        <span className="flex-1" />
        {risk && (
          <span className={`px-2 py-0.5 rounded-full font-semibold ${risk.className}`}>{risk.label}</span>
        )}
        <button
          type="button"
          onClick={dismiss}
          title="Fermer sans appliquer (Échap)"
          aria-label="Fermer l'édition IA (Échap)"
          className="inline-flex items-center justify-center w-6 h-6 rounded-[var(--radius-4)] text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] transition"
        >
          <X size={13} aria-hidden="true" />
        </button>
      </div>

      {/* 2. Saisie : Entrée = aperçu, jamais application directe. */}
      <div className="flex items-center gap-2 px-3 py-2">
        <input
          ref={inputRef}
          value={instruction}
          onChange={event => setInstruction(event.target.value)}
          onKeyDown={event => {
            if (event.key === 'Enter') {
              event.preventDefault();
              if (phase === 'proposal') void applyProposal();
              else void runPreview(instruction);
            }
          }}
          disabled={phase === 'loading'}
          aria-label="Instruction d'édition"
          placeholder={
            entity
              ? 'Que faire de cet élément ? (ex. « plus rapide et rouge »)'
              : 'Sélectionnez un élément dans la scène'
          }
          className="flex-1 min-w-0 text-sm bg-[#07040d] border border-[#2d1b4e] rounded-[var(--radius-6)] px-3 py-2 text-[#f8fafc] placeholder:text-[#64748b] focus:border-[#a855f7] focus:outline-none disabled:opacity-60"
        />
        <button
          type="button"
          onClick={() => void runPreview(instruction)}
          disabled={phase === 'loading' || instruction.trim().length === 0}
          className="inline-flex items-center gap-1.5 px-3 h-9 rounded-[var(--radius-6)] bg-[#7c3aed] hover:bg-[#6d28d9] disabled:bg-[#2d1b4e] disabled:text-[#64748b] text-white text-xs font-semibold transition"
        >
          {phase === 'loading' ? (
            <Loader2 size={13} className="animate-spin" aria-hidden="true" />
          ) : (
            <CornerDownLeft size={13} aria-hidden="true" />
          )}
          Aperçu
        </button>
      </div>

      {/* 3. Suggestions contextuelles (déjà résolues sans modèle). */}
      {phase === 'idle' && suggestions.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5 px-3 pb-2">
          {suggestions.map(suggestion => (
            <button
              key={suggestion.label}
              type="button"
              onClick={() => {
                if (suggestion.submit) {
                  void runPreview(suggestion.text ?? suggestion.label);
                } else {
                  setInstruction(suggestion.text ?? suggestion.label);
                  inputRef.current?.focus();
                }
              }}
              className="min-h-6 px-2 py-0.5 rounded-full border border-[#2d1b4e] text-[11px] text-[#94a3b8] hover:text-[#f8fafc] hover:border-[#a855f7]/60 hover:bg-[#150d27] transition"
            >
              {suggestion.label}
            </button>
          ))}
        </div>
      )}

      {/* 4. Résultat : diff sémantique, jamais un diff JSON. */}
      <div aria-live="polite" className="px-3 pb-3">
        {phase === 'loading' && (
          <p className="text-[11px] text-[#94a3b8]">Analyse de la demande…</p>
        )}

        {phase === 'proposal' && proposal && (
          <div className="rounded-[var(--radius-8)] border border-[#2d1b4e] bg-[#150d27] p-2.5">
            <p className="text-[11px] text-[#cbd5e1] mb-1.5">{proposal.rationale}</p>
            <ul className="space-y-0.5">
              {changes.slice(0, 6).map((change, index) => (
                <li key={`${change.field}-${index}`} className="flex items-baseline gap-1.5 text-[11px]">
                  <span className="text-[#94a3b8] font-mono">{change.field}</span>
                  {change.before && (
                    <span className="text-[#64748b] line-through">{change.before}</span>
                  )}
                  <ArrowRight size={10} className="text-[#64748b]" aria-hidden="true" />
                  <span className="text-[#34d399] font-semibold">{change.after}</span>
                </li>
              ))}
              {changes.length > 6 && (
                <li className="text-[11px] text-[#94a3b8]">+ {changes.length - 6} autre(s)…</li>
              )}
            </ul>
            <div className="flex items-center gap-2 mt-2">
              <button
                type="button"
                onClick={() => void applyProposal()}
                className="inline-flex items-center gap-1.5 px-3 h-8 rounded-[var(--radius-6)] bg-[#059669] hover:bg-[#047857] text-white text-xs font-semibold transition"
              >
                Appliquer
                <kbd className="text-[10px] opacity-80">↵</kbd>
              </button>
              <button
                type="button"
                onClick={dismiss}
                className="px-3 h-8 rounded-[var(--radius-6)] border border-[#2d1b4e] text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] text-xs transition"
              >
                Annuler <kbd className="text-[10px] opacity-70">Échap</kbd>
              </button>
              <span className="text-[10px] text-[#64748b]">
                Rien n'est appliqué avant « Appliquer ».
              </span>
            </div>
          </div>
        )}

        {phase === 'clarification' && note && (
          <div className="flex items-start gap-2 rounded-[var(--radius-8)] border border-[#78350f] bg-[#1c1407] p-2.5">
            <AlertTriangle size={13} className="text-[#f59e0b] mt-0.5 shrink-0" aria-hidden="true" />
            <p className="text-[11px] text-[#fcd34d] leading-relaxed">{note}</p>
          </div>
        )}

        {phase === 'llm' && (
          <div className="flex flex-wrap items-center gap-2 rounded-[var(--radius-8)] border border-[#2d1b4e] bg-[#150d27] p-2.5">
            <p className="text-[11px] text-[#94a3b8] flex-1 min-w-[16rem]">
              Cette demande sort des modèles déterministes. L'Atelier IA la traitera avec le
              modèle choisi{prompt ? ` (prompt ${prompt.length} car.)` : ''}.
            </p>
            <button
              type="button"
              onClick={onOpenAssistant}
              className="px-3 h-8 rounded-[var(--radius-6)] border border-[#a855f7]/60 text-[#c084fc] hover:bg-[#1e1338] text-xs font-semibold transition"
            >
              Ouvrir l'Atelier IA
            </button>
          </div>
        )}

        {phase === 'error' && note && (
          <div className="flex items-start gap-2 rounded-[var(--radius-8)] border border-[#881337] bg-[#1c0a10] p-2.5">
            <AlertTriangle size={13} className="text-[#f43f5e] mt-0.5 shrink-0" aria-hidden="true" />
            <p className="text-[11px] text-[#fecdd3] leading-relaxed">{note}</p>
          </div>
        )}
      </div>
    </div>
  );
};
