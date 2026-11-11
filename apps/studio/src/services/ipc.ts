import { invoke } from '@tauri-apps/api/core';

export interface IpcError {
  command: string;
  message: string;
  cause: unknown;
}

const DEFAULT_TIMEOUT_MS = 15_000;

/**
 * Enveloppe typée autour de `invoke` (UX-1002) :
 * - timeout (15 s par défaut) → pas de promesse suspendue ;
 * - erreurs normalisées (IpcError) ;
 * - journalisation (commande + durée) en dev.
 */
export async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  timeoutMs: number = DEFAULT_TIMEOUT_MS,
): Promise<T> {
  const started = performance.now();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`Délai dépassé (${timeoutMs} ms)`)), timeoutMs);
  });
  try {
    return await Promise.race([invoke<T>(command, args), timeout]);
  } catch (cause) {
    const err: IpcError = {
      command,
      message: cause instanceof Error ? cause.message : String(cause),
      cause,
    };
    console.warn(`[ipc] ${command} a échoué`, err);
    throw err;
  } finally {
    if (timer) clearTimeout(timer);
    if (import.meta.env.DEV) {
      console.debug(`[ipc] ${command} — ${Math.round(performance.now() - started)} ms`);
    }
  }
}

/**
 * Lis une réponse IPC sérialisée en JSON (chaîne) avec repli sûr.
 * Corrige le piège `JSON.parse(null) → null` : si la réponse n'est pas une
 * chaîne ou si le parse échoue, on renvoie `fallback` (jamais `null`/`undefined`
 * pour un tableau — UX-1002).
 */
export async function safeInvokeJson<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  fallback: T,
  timeoutMs?: number,
): Promise<T> {
  try {
    const raw = await safeInvoke<string>(command, args, timeoutMs);
    if (typeof raw !== 'string') return fallback;
    const parsed = JSON.parse(raw) as T;
    return parsed ?? fallback;
  } catch {
    return fallback;
  }
}
