import { useEffect, useRef } from 'react';

interface FirstRunHintProps {
  /** Affiché une seule fois par session, juste après la création du premier jeu. */
  show: boolean;
  onDismiss: () => void;
}

/**
 * Aide au premier lancement — surcouche discrète en mode Jouer.
 *
 * Contrat :
 * - **jamais bloquante** : `pointer-events-none`, aucun élément focusable, et le
 *   listener clavier est en phase capture SANS `preventDefault`/`stopPropagation`
 *   (la touche continue vers le moteur de jeu) ;
 * - disparaît à la première touche, au premier clic, ou après 10 s ;
 * - le parent garantit un seul affichage par session (`App.tsx`).
 */
export function FirstRunHint({ show, onDismiss }: FirstRunHintProps) {
  // Évite de relancer l'effet si le parent fournit un jour un callback instable.
  const dismissRef = useRef(onDismiss);
  useEffect(() => {
    dismissRef.current = onDismiss;
  }, [onDismiss]);

  useEffect(() => {
    if (!show) return;
    const dismiss = () => dismissRef.current();
    // Capture : on observe la touche avant le viewport, mais on la laisse passer.
    window.addEventListener('keydown', dismiss, true);
    window.addEventListener('pointerdown', dismiss, true);
    const timer = window.setTimeout(dismiss, 10_000);
    return () => {
      window.removeEventListener('keydown', dismiss, true);
      window.removeEventListener('pointerdown', dismiss, true);
      window.clearTimeout(timer);
    };
  }, [show]);

  if (!show) return null;

  return (
    <div
      data-testid="first-run-hint"
      role="status"
      aria-live="polite"
      className="pointer-events-none fixed bottom-12 left-4 z-40 max-w-[calc(100vw-2rem)]"
    >
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 rounded-lg border border-[#a855f7] bg-[#150d27]/95 px-3.5 py-2 shadow-lg shadow-black/50 backdrop-blur-sm">
        <span className="flex items-center gap-1.5 text-xs text-[#f8fafc]">
          <kbd className="rounded border border-[#2d1b4e] bg-[#0d0819] px-1.5 py-0.5 font-mono text-[11px] text-[#f8fafc]">
            ← ↑ ↓ →
          </kbd>
          Flèches pour bouger
        </span>
        <span aria-hidden="true" className="text-[#94a3b8]">·</span>
        <span className="flex items-center gap-1.5 text-xs text-[#f8fafc]">
          <kbd className="rounded border border-[#2d1b4e] bg-[#0d0819] px-1.5 py-0.5 font-mono text-[11px] text-[#f8fafc]">
            F
          </kbd>
          pour tirer
        </span>
        <span aria-hidden="true" className="text-[#94a3b8]">·</span>
        <span className="flex items-center gap-1.5 text-xs text-[#f8fafc]">
          <kbd className="rounded border border-[#2d1b4e] bg-[#0d0819] px-1.5 py-0.5 font-mono text-[11px] text-[#f8fafc]">
            E
          </kbd>
          pour interagir
        </span>
      </div>
    </div>
  );
}
