import { safeInvoke } from './ipc';

/**
 * Pont typé vers les commandes Tauri d'édition IA inline (plan IA inline, sprint 1).
 *
 * Contrat : `ai_edit_preview` ne modifie **rien** — il renvoie une proposition (ou une
 * demande de clarification, ou un prompt à donner à un modèle). L'application passe
 * toujours par `ai_edit_apply`, qui revalide tout côté moteur.
 */

export type AiRisk = 'safe' | 'confirm' | 'destructive';

export interface AiFieldChange {
  target: string;
  field: string;
  before: string;
  after: string;
}

export interface AiResolvedTarget {
  kind: 'entity' | 'kvl_node' | 'scene';
  label: string;
  entity_ids: number[];
  kvl_graph: string | null;
  kvl_node: string | null;
}

export interface AiEditContext {
  project_id: string;
  scene_id: string;
  dimension: 'two_d' | 'two_and_half_d' | 'three_d';
  selection: unknown[];
  active_tool: string;
}

export interface AiEditProposal {
  id: string;
  intent_id: string;
  target: AiResolvedTarget;
  ops: unknown[];
  rationale: string;
  risk: AiRisk;
  validation: { ok: boolean; messages: string[] };
  preview: { changes: AiFieldChange[] };
  provider: string;
  model: string;
  tokens: { prompt: number; completion: number };
  hash: string;
}

/** Réponse d'aperçu : proposition, clarification ou prompt à envoyer au modèle. */
export type AiEditPreview =
  | { status: 'proposal'; proposal: AiEditProposal }
  | { status: 'clarification'; message: string }
  | { status: 'llm'; prompt: string; prompt_tokens: number };

export interface AiApplyReport {
  transaction_id: string;
  intent_id: string;
  changes: AiFieldChange[];
  risk: AiRisk;
}

function messageOf(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

export function isProposal(
  preview: AiEditPreview,
): preview is { status: 'proposal'; proposal: AiEditProposal } {
  return preview.status === 'proposal';
}

/** Contexte d'édition courant (sélection, scène, dimension). */
export async function getAiEditContext(): Promise<AiEditContext | null> {
  try {
    const raw = await safeInvoke<string>('ai_context');
    return JSON.parse(raw) as AiEditContext;
  } catch {
    return null;
  }
}

/** Aperçu d'une instruction inline. Ne modifie rien. */
export async function previewAiEdit(instruction: string): Promise<AiEditPreview> {
  try {
    return await safeInvoke<AiEditPreview>('ai_edit_preview', { instruction });
  } catch (error) {
    throw new Error(messageOf(error));
  }
}

/** Proposition construite depuis les ops JSON d'un modèle (revalidées par le moteur). */
export async function proposeAiEdit(rawJson: string): Promise<AiEditPreview> {
  try {
    return await safeInvoke<AiEditPreview>('ai_edit_propose', { rawJson });
  } catch (error) {
    throw new Error(messageOf(error));
  }
}

/** Applique une proposition en attente ; renvoie la transaction d'annulation. */
export async function applyAiEdit(proposalId: string): Promise<AiApplyReport> {
  const raw = await safeInvoke<string>('ai_edit_apply', { proposalId });
  return JSON.parse(raw) as AiApplyReport;
}

/** Oublie une proposition sans rien appliquer. */
export async function cancelAiEdit(proposalId: string): Promise<boolean> {
  try {
    return await safeInvoke<boolean>('ai_edit_cancel', { proposalId });
  } catch {
    return false;
  }
}

/** Annule une édition appliquée (une intention = une transaction). */
export async function undoAiEdit(transactionId: string): Promise<boolean> {
  try {
    return await safeInvoke<boolean>('ai_edit_undo', { transactionId });
  } catch {
    return false;
  }
}
