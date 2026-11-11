import { Component, type ReactNode } from 'react';
import { AlertTriangle, RotateCcw, Home } from 'lucide-react';

interface ErrorBoundaryProps {
  children: ReactNode;
  label?: string;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Garde-fou React : capture toute erreur de rendu d'un panneau et affiche un
 * fallback récupérable (recharger le panneau / revenir à l'accueil) au lieu de
 * laisser un écran blanc (UX-1001, K13).
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  private reset = () => this.setState({ error: null });

  render() {
    if (this.state.error) {
      return (
        <div className="flex flex-col items-center justify-center h-full min-h-[220px] p-6 text-center bg-[#0d0819] border border-[#3b0a0a] rounded-lg select-none">
          <AlertTriangle size={28} className="text-[#f43f5e] mb-2" />
          <h2 className="font-bold text-sm text-[#f8fafc] mb-1">Ce panneau a rencontré une erreur</h2>
          <p className="text-xs text-[#94a3b8] mb-4 max-w-sm break-words">
            {this.props.label ? `${this.props.label} — ` : ''}
            {this.state.error.message || 'Erreur inattendue'}
          </p>
          <div className="flex items-center gap-2">
            <button
              onClick={this.reset}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] border border-[#3b0764] rounded-md text-xs font-medium transition"
            >
              <RotateCcw size={13} /> Recharger le panneau
            </button>
            <button
              onClick={() => window.location.reload()}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] border border-[#3b0764] rounded-md text-xs font-medium transition"
            >
              <Home size={13} /> Revenir à l'accueil
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
