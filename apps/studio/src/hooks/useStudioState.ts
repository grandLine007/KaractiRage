import { useCallback, useEffect, useRef, useState } from 'react';
import { getEditorState, type StudioEditorState } from '../services/engineBridge';

const POLL_MS = 160; // ~6 Hz — léger pour ne pas concurrencer le framebuffer 60 FPS

/**
 * S'abonne à l'état d'édition natif (entités, sélection, outil, tuile, historique).
 * Le polling est suspendu quand `enabled` est faux (mode Jeu ou drag en cours).
 */
export function useStudioState(enabled: boolean) {
  const [state, setState] = useState<StudioEditorState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timerRef = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await getEditorState();
      setState(next);
      setError(null);
    } catch (e) {
      setError((e as Error)?.message ?? 'Impossible de joindre le moteur natif');
    }
  }, []);

  useEffect(() => {
    if (!enabled) {
      if (timerRef.current !== null) {
        window.clearInterval(timerRef.current);
        timerRef.current = null;
      }
      return;
    }

    void refresh();
    timerRef.current = window.setInterval(() => {
      void refresh();
    }, POLL_MS);

    return () => {
      if (timerRef.current !== null) {
        window.clearInterval(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [enabled, refresh]);

  return { state, refresh, error };
}
