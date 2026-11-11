import { CheckCircle2, AlertCircle, Info, X } from 'lucide-react';

export interface Toast {
  id: number;
  message: string;
  kind: 'info' | 'success' | 'error';
}

interface ToastCenterProps {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}

const KIND_STYLE = {
  success: { border: '#059669', text: '#34d399', icon: <CheckCircle2 size={16} /> },
  error: { border: '#e11d48', text: '#f43f5e', icon: <AlertCircle size={16} /> },
  info: { border: '#a855f7', text: '#c084fc', icon: <Info size={16} /> },
};

/**
 * Centre de notifications non bloquantes (remplace les alert() natifs).
 */
export const ToastCenter: React.FC<ToastCenterProps> = ({ toasts, onDismiss }) => {
  if (toasts.length === 0) return null;

  return (
    <div className="absolute bottom-12 left-1/2 -translate-x-1/2 z-50 flex flex-col items-center gap-2 pointer-events-none">
      {toasts.map(t => {
        const s = KIND_STYLE[t.kind];
        return (
          <div
            key={t.id}
            className="flex items-center gap-2.5 px-4 py-2.5 bg-[#150d27]/95 border rounded-lg shadow-2xl shadow-black/50 backdrop-blur-sm pointer-events-auto max-w-md"
            style={{ borderColor: s.border }}
          >
            <span style={{ color: s.text }}>{s.icon}</span>
            <span className="text-xs text-[#f8fafc] leading-relaxed">{t.message}</span>
            <button
              onClick={() => onDismiss(t.id)}
              className="ml-1 p-0.5 text-[#94a3b8] hover:text-[#f8fafc] transition"
              title="Fermer"
            >
              <X size={13} />
            </button>
          </div>
        );
      })}
    </div>
  );
};
