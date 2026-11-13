import { useState } from 'react';
import { HardDrive, Loader2, Play, Save, X } from 'lucide-react';
import {
  SAVE_SLOT_COUNT,
  loadGame,
  resumeGame,
  saveGame,
} from '../services/engineBridge';

/** Identifiant DOM du panneau (le parent s'en sert pour détecter un clic extérieur). */
export const SAVE_SLOTS_PANEL_ID = 'save-slots-panel';

const SLOTS = Array.from({ length: SAVE_SLOT_COUNT }, (_, index) => index);

interface SaveSlotsPanelProps {
  isOpen: boolean;
  onClose: () => void;
  /**
   * Appelé après une sauvegarde : `message` non vide = succès (toast de succès),
   * chaîne vide = échec (le parent affiche l'erreur).
   */
  onSaved: (slot: number, message: string) => void;
  onLoaded: (slot: number, ok: boolean) => void;
  onResumed: (ok: boolean) => void;
}

/** Dernier segment d'un chemin (affichage court dans les toasts). */
function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

/**
 * Panneau « Sauvegardes » — ancré au-dessus du pied de page, **non modal** :
 * pas de backdrop, le viewport (et le mode jeu) restent visibles et interactifs.
 *
 * Le panneau reste bête : il appelle le moteur et remonte les résultats au parent
 * (`onSaved` / `onLoaded` / `onResumed`), qui gère toasts, `refresh()` et le mode.
 */
export const SaveSlotsPanel: React.FC<SaveSlotsPanelProps> = ({
  isOpen,
  onClose,
  onSaved,
  onLoaded,
  onResumed,
}) => {
  const [savingSlot, setSavingSlot] = useState<number | null>(null);
  const [loadingSlot, setLoadingSlot] = useState<number | null>(null);
  const [isResuming, setIsResuming] = useState(false);
  const [note, setNote] = useState<string>('Chaque slot conserve le niveau et les variables.');
  const [noteTone, setNoteTone] = useState<'info' | 'success' | 'error'>('info');

  if (!isOpen) return null;

  const busy = isResuming || savingSlot !== null || loadingSlot !== null;

  const handleResume = async () => {
    if (busy) return;
    setIsResuming(true);
    setNote('Reprise de la dernière sauvegarde…');
    setNoteTone('info');
    const resumed = await resumeGame();
    setIsResuming(false);
    setNote(resumed ? 'Dernière sauvegarde reprise.' : 'Aucune sauvegarde récente à reprendre.');
    setNoteTone(resumed ? 'success' : 'error');
    onResumed(resumed);
  };

  const handleSave = async (slot: number) => {
    if (busy) return;
    setSavingSlot(slot);
    setNote(`Sauvegarde du slot ${slot + 1}…`);
    setNoteTone('info');
    const path = await saveGame(slot);
    setSavingSlot(null);
    if (path) {
      setNote(`Slot ${slot + 1} sauvegardé.`);
      setNoteTone('success');
      onSaved(slot, `Slot ${slot + 1} sauvegardé (${fileName(path)}).`);
    } else {
      setNote(`Échec de la sauvegarde du slot ${slot + 1}.`);
      setNoteTone('error');
      onSaved(slot, '');
    }
  };

  const handleLoad = async (slot: number) => {
    if (busy) return;
    setLoadingSlot(slot);
    setNote(`Chargement du slot ${slot + 1}…`);
    setNoteTone('info');
    const loaded = await loadGame(slot);
    setLoadingSlot(null);
    setNote(loaded ? `Slot ${slot + 1} chargé.` : `Slot ${slot + 1} vide ou illisible.`);
    setNoteTone(loaded ? 'success' : 'error');
    onLoaded(slot, loaded);
  };

  const noteClass = noteTone === 'success'
    ? 'text-[#34d399]'
    : noteTone === 'error'
      ? 'text-[#fda4af]'
      : 'text-[#94a3b8]';

  return (
    <div
      id={SAVE_SLOTS_PANEL_ID}
      role="dialog"
      aria-modal="false"
      aria-labelledby="save-slots-title"
      className="absolute bottom-full right-4 z-50 mb-2 w-72 overflow-hidden rounded-[var(--radius-12)] border border-[#2d1b4e] bg-[#0d0819] shadow-[var(--shadow-e3)]"
    >
      <div className="flex items-center justify-between border-b border-[#2d1b4e] bg-[#150d27] px-3 py-2">
        <h2 id="save-slots-title" className="flex items-center gap-2 text-sm font-bold text-[#f8fafc]">
          <Save size={14} className="text-[#a855f7]" aria-hidden="true" />
          Sauvegardes
        </h2>
        <button
          type="button"
          onClick={onClose}
          title="Fermer le panneau (Échap)"
          aria-label="Fermer le panneau Sauvegardes"
          className="flex h-6 w-6 items-center justify-center rounded-[var(--radius-4)] text-[#94a3b8] transition hover:brightness-150"
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>

      <div className="space-y-2 p-3">
        <button
          type="button"
          onClick={() => void handleResume()}
          disabled={busy}
          aria-busy={isResuming}
          title="Reprendre la sauvegarde la plus récente"
          className="flex h-7 w-full items-center justify-center gap-2 rounded-[var(--radius-6)] bg-[var(--accent-primary)] text-[var(--bg-deep)] text-xs font-bold transition hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {isResuming
            ? <Loader2 size={13} className="animate-spin" aria-hidden="true" />
            : <Play size={13} aria-hidden="true" />}
          {isResuming ? 'Reprise…' : 'Reprendre la dernière'}
        </button>

        <div className="space-y-2">
          {SLOTS.map(slot => {
            const saving = savingSlot === slot;
            const loading = loadingSlot === slot;
            return (
              <div
                key={slot}
                className="rounded-[var(--radius-8)] border border-[#2d1b4e] bg-[#150d27] p-2"
              >
                <div className="mb-2 flex items-center justify-between">
                  <span className="text-xs font-semibold text-[#f8fafc]">Slot {slot + 1}</span>
                  <HardDrive size={12} className="text-[#94a3b8]" aria-hidden="true" />
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => void handleSave(slot)}
                    disabled={busy}
                    aria-busy={saving}
                    aria-label={`Sauvegarder dans le slot ${slot + 1}`}
                    title={`Sauvegarder la partie dans le slot ${slot + 1}`}
                    className="flex h-6 flex-1 items-center justify-center gap-1 rounded-[var(--radius-4)] border border-[var(--border-subtle)] bg-[var(--bg-panel)] text-[var(--text-main)] text-xs font-semibold transition hover:brightness-125 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {saving
                      ? <Loader2 size={12} className="animate-spin" aria-hidden="true" />
                      : <Save size={12} aria-hidden="true" />}
                    {saving ? 'Sauvegarde…' : 'Sauvegarder'}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleLoad(slot)}
                    disabled={busy}
                    aria-busy={loading}
                    aria-label={`Charger le slot ${slot + 1}`}
                    title={`Charger la partie du slot ${slot + 1}`}
                    className="flex h-6 flex-1 items-center justify-center gap-1 rounded-[var(--radius-4)] border border-[var(--border-subtle)] bg-[var(--bg-panel)] text-[var(--text-main)] text-xs font-semibold transition hover:brightness-125 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {loading
                      ? <Loader2 size={12} className="animate-spin" aria-hidden="true" />
                      : <Play size={12} aria-hidden="true" />}
                    {loading ? 'Chargement…' : 'Charger'}
                  </button>
                </div>
              </div>
            );
          })}
        </div>

        <p role="status" aria-live="polite" className={`min-h-4 pt-1 text-xs ${noteClass}`}>
          {note}
        </p>
      </div>
    </div>
  );
};
