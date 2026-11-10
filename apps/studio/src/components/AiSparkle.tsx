import { Sparkles } from 'lucide-react';

interface AiSparkleProps {
  /** Libellé accessible : décrit le champ visé (« position X », « vitesse »…). */
  label: string;
  onClick: () => void;
  /** Infobulle visible (plus explicite que l'aria-label sur les champs techniques). */
  title?: string;
  /** Identifiant stable pour les tests d'audit (un par champ). */
  testId?: string;
}

/**
 * Affordance `✦` d'édition IA (plan IA inline, surface C).
 *
 * Toujours 24 × 24 px minimum (K5, WCAG 2.5.8) et jamais un simple glyphe décoratif :
 * c'est un bouton nommé, focusable, qui ouvre la barre ancrée avec le champ en indice.
 */
export const AiSparkle: React.FC<AiSparkleProps> = ({ label, onClick, title, testId }) => (
  <button
    type="button"
    onClick={(event) => {
      event.stopPropagation();
      onClick();
    }}
    title={title ?? `Modifier ${label} avec l'IA`}
    aria-label={`Modifier ${label} avec l'IA`}
    data-testid={testId ?? 'ai-sparkle'}
    className="inline-flex items-center justify-center w-6 h-6 shrink-0 rounded-[var(--radius-4)] text-[#c084fc] hover:text-[#f8fafc] hover:bg-[#1e1338] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#a855f7] transition"
  >
    <Sparkles size={12} aria-hidden="true" />
  </button>
);
