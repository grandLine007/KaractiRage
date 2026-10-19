import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Bot, X, Sparkles, CheckCircle2, AlertCircle, RefreshCw, Send, Shield, Package, Flame, Zap } from 'lucide-react';

interface AiAssistantModalProps {
  isOpen: boolean;
  onClose: () => void;
  onApplyGeneratedLevel?: (levelPayloadJson: string) => void;
}

const PROVIDERS = [
  { id: 'lmstudio', name: 'LM STUDIO', defaultEndpoint: 'http://127.0.0.1:1234', defaultModel: 'nvidia/nemotron-3-nano-4b' },
  { id: 'ollama', name: 'OLLAMA', defaultEndpoint: 'http://localhost:11434', defaultModel: 'llama3:latest' },
  { id: 'claude', name: 'CLAUDE', defaultEndpoint: 'https://api.anthropic.com/v1/messages', defaultModel: 'claude-3-5-sonnet' },
  { id: 'gemini', name: 'GEMINI', defaultEndpoint: 'https://generativelanguage.googleapis.com', defaultModel: 'gemini-1.5-pro' },
  { id: 'deepseek', name: 'DEEPSEEK', defaultEndpoint: 'https://api.deepseek.com/v1', defaultModel: 'deepseek-chat' },
  { id: 'openai', name: 'OPENAI', defaultEndpoint: 'https://api.openai.com/v1', defaultModel: 'gpt-4o' },
  { id: 'offline', name: 'OFFLINE', defaultEndpoint: 'Local In-Engine', defaultModel: 'Règles Déterministes' }
];

const PRESETS = [
  {
    id: 'guard',
    title: 'Garde Patrouilleur A* & Alerte',
    desc: 'Ajoute un garde hostile avec vision conique, alerte sonore et poursuite A* vers le joueur.',
    prompt: "Ajouter un garde patrouilleur hostile avec vision conique, alerte sonore et algorithme A* de poursuite du joueur."
  },
  {
    id: 'chest',
    title: 'Coffre au Trésor & Clé de Donjon',
    desc: 'Génère un coffre scellé nécessitant la clé dorée et conférant 150 points de score.',
    prompt: "Créer un coffre au trésor scellé nécessitant une clé dorée pour s'ouvrir et attribuer 150 points de score."
  },
  {
    id: 'hazard',
    title: 'Zone de Lave & Piège Sismique',
    desc: 'Place une zone de lave ardente infligeant 35 dégâts au contact avec étincelles continues.',
    prompt: "Générer une zone de lave ardente infligeant 35 dégâts par seconde avec étincelles de braises et plateformes mobiles."
  },
  {
    id: 'portal',
    title: 'Portail Dimensionnel Quantique',
    desc: 'Crée un portail téléportant le joueur avec distorsion arcanique et audio spatial.',
    prompt: "Ajouter un portail quantique de téléportation avec vortex visuel violet et son d'activation arcanique."
  }
];

export const AiAssistantModal: React.FC<AiAssistantModalProps> = ({ isOpen, onClose, onApplyGeneratedLevel }) => {
  const [selectedProvider, setSelectedProvider] = useState('lmstudio');
  const [promptText, setPromptText] = useState('');
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:1234');
  const [testStatus, setTestStatus] = useState<{ loading: boolean; success?: boolean; message?: string } | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [generationOutput, setGenerationOutput] = useState<{ success: boolean; message: string; json?: string } | null>(null);

  if (!isOpen) return null;

  const handleProviderSelect = (providerId: string) => {
    setSelectedProvider(providerId);
    const p = PROVIDERS.find(item => item.id === providerId);
    if (p) {
      setEndpoint(p.defaultEndpoint);
    }
    setTestStatus(null);
  };

  const handleTestConnection = async () => {
    setTestStatus({ loading: true });
    try {
      const res: any = await invoke('test_llm_connection', {
        providerName: selectedProvider,
        endpoint: endpoint.trim() || undefined
      });
      setTestStatus({
        loading: false,
        success: res.success,
        message: res.message
      });
    } catch (err: any) {
      setTestStatus({
        loading: false,
        success: false,
        message: err?.toString() || 'Erreur lors du test de connexion'
      });
    }
  };

  const handleGenerate = async () => {
    if (!promptText.trim()) return;
    setIsGenerating(true);
    setGenerationOutput(null);

    try {
      const res: any = await invoke('generate_ai_level', {
        prompt: promptText,
        providerName: selectedProvider,
        endpoint: endpoint.trim() || undefined
      });

      setGenerationOutput({
        success: res.success,
        message: res.message,
        json: res.payload_json
      });
    } catch (err: any) {
      setGenerationOutput({
        success: false,
        message: err?.toString() || 'Erreur lors de la génération IA'
      });
    } finally {
      setIsGenerating(false);
    }
  };

  const handleSelectPreset = (p: typeof PRESETS[0]) => {
    setPromptText(p.prompt);
  };

  const charsLeft = 400 - promptText.length;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md select-none">
      <div className="relative flex flex-col w-full max-w-3xl max-h-[90vh] bg-[#0d0819] border border-[#a855f7]/50 rounded-xl shadow-2xl shadow-purple-950/80 overflow-hidden">
        {/* En-tête */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#150d27] border-b border-[#2d1b4e]">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-lg bg-[#2e1065] border border-[#a855f7] text-[#c084fc]">
              <Bot size={20} />
            </div>
            <div>
              <h2 className="text-base font-bold text-[#f8fafc] tracking-wide">
                ASSISTANT IA UNIVERSEL (LM STUDIO, OLLAMA, CLOUD)
              </h2>
              <p className="text-xs text-[#94a3b8]">
                Pilotez la génération de gameplay, univers 2D et scripts KVL par intention naturelle
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 text-[#94a3b8] hover:text-[#f8fafc] hover:bg-[#1e1338] rounded-lg transition"
          >
            <X size={18} />
          </button>
        </div>

        {/* Corps défilable */}
        <div className="flex-1 overflow-y-auto p-6 space-y-5">
          {/* 1. Sélecteur de Fournisseurs */}
          <div>
            <label className="block text-xs font-semibold text-[#94a3b8] uppercase tracking-wider mb-2">
              Fournisseur IA (Local ou Distant) :
            </label>
            <div className="grid grid-cols-4 sm:grid-cols-7 gap-2">
              {PROVIDERS.map(p => (
                <button
                  key={p.id}
                  onClick={() => handleProviderSelect(p.id)}
                  className={`py-2 px-2 text-center text-xs font-bold rounded-lg border transition ${
                    selectedProvider === p.id
                      ? 'bg-[#3b0764] text-[#f3e8ff] border-[#a855f7] shadow-lg shadow-purple-950/40'
                      : 'bg-[#150d27] text-[#94a3b8] border-[#2d1b4e] hover:bg-[#1e1338] hover:text-[#f8fafc]'
                  }`}
                >
                  {p.name}
                </button>
              ))}
            </div>
          </div>

          {/* 2. Ligne de Connexion & Test */}
          <div className="p-3.5 bg-[#150d27] border border-[#2d1b4e] rounded-lg flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
            <div className="flex items-center gap-2 flex-1">
              <span className="text-xs text-[#94a3b8]">Endpoint :</span>
              <input
                type="text"
                value={endpoint}
                onChange={e => setEndpoint(e.target.value)}
                className="flex-1 text-xs font-mono text-[#38bdf8] bg-[#07040d] border border-[#2d1b4e] rounded px-2.5 py-1"
                placeholder="http://127.0.0.1:1234"
              />
            </div>
            <button
              onClick={handleTestConnection}
              disabled={testStatus?.loading}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1e1338] hover:bg-[#2d1b4e] border border-[#a855f7]/60 text-[#c084fc] rounded text-xs font-semibold transition shrink-0"
            >
              <RefreshCw size={13} className={testStatus?.loading ? 'animate-spin' : ''} />
              {testStatus?.loading ? 'Test en cours...' : 'Tester la connexion'}
            </button>
          </div>

          {/* Résultat du test */}
          {testStatus && (
            <div className={`p-3 rounded-lg border text-xs flex items-center gap-2 ${
              testStatus.success
                ? 'bg-[#064e3b]/30 border-[#059669] text-[#34d399]'
                : 'bg-[#881337]/30 border-[#e11d48] text-[#f43f5e]'
            }`}>
              {testStatus.success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
              <span>{testStatus.message}</span>
            </div>
          )}

          {/* 3. Champ de saisie d'intention libre */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs font-semibold text-[#f8fafc] uppercase tracking-wider flex items-center gap-2">
                <Sparkles size={14} className="text-[#a855f7]" />
                Votre Intention en Langage Naturel (Français) :
              </label>
              <span className={`text-xs font-mono font-medium ${charsLeft < 20 ? 'text-[#f43f5e]' : 'text-[#94a3b8]'}`}>
                {promptText.length} / 400 car.
              </span>
            </div>
            <textarea
              rows={3}
              maxLength={400}
              value={promptText}
              onChange={e => setPromptText(e.target.value)}
              placeholder="Exemple : Crée un donjon de pierre avec 3 salles piégées de lave, un golem protecteur et un coffre au trésor verrouillé..."
              className="w-full p-3 text-sm bg-[#07040d] border border-[#2d1b4e] rounded-lg focus:border-[#a855f7] focus:ring-1 focus:ring-[#a855f7] transition resize-none leading-relaxed text-[#f8fafc]"
            />
          </div>

          {/* 4. Suggestions rapides de génération */}
          <div>
            <span className="block text-xs font-semibold text-[#64748b] uppercase tracking-wider mb-2.5">
              Ou sélectionnez une suggestion rapide du Studio :
            </span>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
              {PRESETS.map(p => (
                <div
                  key={p.id}
                  onClick={() => handleSelectPreset(p)}
                  className="p-3 bg-[#150d27] hover:bg-[#1e1338] border border-[#2d1b4e] hover:border-[#a855f7] rounded-lg cursor-pointer transition group"
                >
                  <h4 className="font-semibold text-xs text-[#f8fafc] group-hover:text-[#c084fc] flex items-center gap-1.5 mb-1">
                    {p.id === 'guard' && <Shield size={13} className="text-[#38bdf8]" />}
                    {p.id === 'chest' && <Package size={13} className="text-[#f59e0b]" />}
                    {p.id === 'hazard' && <Flame size={13} className="text-[#f43f5e]" />}
                    {p.id === 'portal' && <Zap size={13} className="text-[#a855f7]" />}
                    {p.title}
                  </h4>
                  <p className="text-[11px] text-[#94a3b8] line-clamp-2 leading-relaxed">
                    {p.desc}
                  </p>
                </div>
              ))}
            </div>
          </div>

          {/* Sortie de génération si disponible */}
          {generationOutput && (
            <div className={`p-4 rounded-lg border text-xs space-y-2 ${
              generationOutput.success
                ? 'bg-[#064e3b]/20 border-[#059669] text-[#34d399]'
                : 'bg-[#881337]/20 border-[#e11d48] text-[#f43f5e]'
            }`}>
              <div className="font-bold flex items-center gap-2">
                {generationOutput.success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                <span>{generationOutput.message}</span>
              </div>
              {generationOutput.json && onApplyGeneratedLevel && (
                <div className="pt-2 flex justify-end">
                  <button
                    onClick={() => onApplyGeneratedLevel(generationOutput.json!)}
                    className="px-3 py-1.5 bg-[#059669] hover:bg-[#047857] text-white rounded font-bold text-xs shadow transition"
                  >
                    Appliquer le niveau dans le Viewport
                  </button>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Pied de page */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#150d27] border-t border-[#2d1b4e]">
          <span className="text-xs text-[#64748b]">
            Clavier Français AZERTY & Touches de raccourci supportés nativement
          </span>
          <div className="flex items-center gap-3">
            <button
              onClick={onClose}
              className="px-4 py-2 bg-[#1e1338] hover:bg-[#2d1b4e] text-[#f8fafc] rounded-lg text-xs font-medium transition"
            >
              Fermer [Échap]
            </button>
            <button
              onClick={handleGenerate}
              disabled={isGenerating || !promptText.trim()}
              className="flex items-center gap-2 px-5 py-2 bg-gradient-to-r from-[#9333ea] to-[#7e22ce] hover:from-[#a855f7] hover:to-[#9333ea] disabled:opacity-50 text-white font-bold text-xs rounded-lg shadow-lg shadow-purple-950/60 transition"
            >
              {isGenerating ? (
                <>
                  <RefreshCw size={14} className="animate-spin" />
                  Génération en cours...
                </>
              ) : (
                <>
                  <Send size={14} />
                  Générer mon jeu avec l'IA
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
