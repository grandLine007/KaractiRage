// Détection des capacités de rendu 3D — Lot 1.
// Échelle de dégradation : webgpu → webgl2 → software (repli 2D logiciel actuel).
//
// Ce module est volontairement SANS dépendance three.js : il reste dans le bundle
// principal du Studio et ne déclenche pas le chargement du chunk 3D.

export type RenderTier = 'webgpu' | 'webgl2' | 'software';

export interface RenderCapabilities {
  tier: RenderTier;
  webgpu: boolean;
  webgl2: boolean;
  maxTextureSize: number;
  halfFloat: boolean;
  backend: 'WEBGPU' | 'WEBGL2' | 'LOGICIEL';
}

/** Classe un niveau à partir de deux sondes booléennes (fonction pure). */
export function classifyTier(webgpu: boolean, webgl2: boolean): RenderTier {
  if (webgpu) return 'webgpu';
  if (webgl2) return 'webgl2';
  return 'software';
}

interface WebGPUAdapterLike {
  isFallbackAdapter?: boolean;
}

/** Accès typé à `navigator.gpu`, sans dépendre des types DOM WebGPU du lib. */
function webgpuApi(): { requestAdapter: () => Promise<WebGPUAdapterLike | null> } | null {
  const nav = navigator as Navigator & {
    gpu?: { requestAdapter: () => Promise<WebGPUAdapterLike | null> };
  };
  return nav.gpu ?? null;
}

async function probeWebGPU(timeoutMs = 800): Promise<boolean> {
  const gpu = webgpuApi();
  if (!gpu) return false;
  try {
    const adapter = await Promise.race([
      gpu.requestAdapter(),
      new Promise<null>((resolve) => setTimeout(() => resolve(null), timeoutMs)),
    ]);
    return adapter !== null;
  } catch {
    return false;
  }
}

interface GLProbe {
  webgl2: boolean;
  maxTextureSize: number;
  halfFloat: boolean;
}

function probeWebGL2(): GLProbe {
  try {
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl2', { failIfMajorPerformanceCaveat: false });
    if (!gl) return { webgl2: false, maxTextureSize: 0, halfFloat: false };
    const maxTextureSize = (gl.getParameter(gl.MAX_TEXTURE_SIZE) as number) ?? 0;
    const halfFloat =
      gl.getExtension('EXT_color_buffer_float') !== null ||
      gl.getExtension('EXT_color_buffer_half_float') !== null;
    return { webgl2: true, maxTextureSize, halfFloat };
  } catch {
    return { webgl2: false, maxTextureSize: 0, halfFloat: false };
  }
}

let cachedResult: RenderCapabilities | null = null;
let cachedPromise: Promise<RenderCapabilities> | null = null;

async function detect(): Promise<RenderCapabilities> {
  const webgpu = await probeWebGPU();
  const gl = probeWebGL2();
  const tier = classifyTier(webgpu, gl.webgl2);
  return {
    tier,
    webgpu,
    webgl2: gl.webgl2,
    maxTextureSize: gl.maxTextureSize,
    halfFloat: gl.halfFloat,
    backend: tier === 'webgpu' ? 'WEBGPU' : tier === 'webgl2' ? 'WEBGL2' : 'LOGICIEL',
  };
}

/**
 * Détection mémoïsée : une seule sonde GPU par session (coût d'ouverture amorti),
 * partagée entre le préchauffage (`prewarm3D`) et le badge du viewport.
 */
export function detectRenderCapabilities(): Promise<RenderCapabilities> {
  if (!cachedPromise) {
    cachedPromise = detect().then((c) => {
      cachedResult = c;
      return c;
    });
  }
  return cachedPromise;
}

/** Résultat déjà connu (null si la détection n'a pas encore abouti). */
export function peekRenderCapabilities(): RenderCapabilities | null {
  return cachedResult;
}
