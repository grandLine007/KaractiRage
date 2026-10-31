// Préchauffage paresseux de la 3D — Lot 8 bis (boot non bloquant).
//
// Ne charge rien au montage : App.tsx appelle `prewarm3D()` après le premier paint.
// Le chunk three.js est importé (cache Vite réchauffé) et la sonde GPU est lancée
// une seule fois — sans monter de renderer ni rendre aucune frame.

import { detectRenderCapabilities } from './capabilities';

let started = false;

export function prewarm3D(): void {
  if (started || typeof window === 'undefined') return;
  started = true;

  const warm = () => {
    // Réchauffe le cache du chunk three.js (aucun montage).
    void import('./scene3d');
    // Lance la sonde GPU mémoïsée (badge et choix du backend en profitent).
    void detectRenderCapabilities();
  };

  const w = window as Window & {
    requestIdleCallback?: (cb: () => void, opts?: { timeout?: number }) => number;
  };
  if (typeof w.requestIdleCallback === 'function') {
    w.requestIdleCallback(warm, { timeout: 2500 });
  } else {
    window.setTimeout(warm, 1500);
  }
}
