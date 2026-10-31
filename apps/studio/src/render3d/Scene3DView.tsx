import { useEffect, useRef, useState } from 'react';
import { startScene3D } from './scene3d';
import type { Scene3DStats } from './scene3d';

export default function Scene3DView() {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [stats, setStats] = useState<Scene3DStats | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    let dispose: (() => void) | undefined;
    let active = true;

    startScene3D(container, (s) => {
      if (!active) return;
      setStats(s);
      if (s.error) setError(s.error);
    })
      .then((ctl) => {
        if (active) dispose = () => ctl.dispose();
      })
      .catch((err: unknown) => {
        if (active) setError(err instanceof Error ? err.message : String(err));
      });

    return () => {
      active = false;
      dispose?.();
    };
  }, []);

  const goBack = () => {
    window.location.search = '';
  };

  return (
    <div className="w-screen h-screen bg-[#050308] flex flex-col">
      <div className="flex items-center justify-between px-4 py-2 bg-[#0D1322] border-b border-[#1E293B]">
        <div className="flex items-center gap-2">
          <span className="text-xs font-bold uppercase tracking-wide text-[#f8fafc]">
            Scène 3D — Rust → three.js
          </span>
          <span className="text-[10px] font-mono text-[#94a3b8]">
            Lot 4 · RenderPacket
          </span>
        </div>
        <button
          onClick={goBack}
          className="px-3 py-1 text-xs font-medium rounded bg-[#1E293B] text-[#94A3B8] hover:bg-[#2d1b4e] hover:text-white transition"
        >
          ← Retour au Studio
        </button>
      </div>

      <div className="relative flex-1 overflow-hidden">
        <div ref={containerRef} className="absolute inset-0" />

        <div className="absolute top-3 left-3 flex flex-col gap-1.5 font-mono text-[11px] pointer-events-none">
          <span
            className={`px-2 py-1 rounded border font-bold ${
              stats?.backend === 'webgpu'
                ? 'bg-[#082F49] text-[#38bdf8] border-[#0284c7]'
                : 'bg-[#064E3B] text-[#a7f3d0] border-[#059669]'
            }`}
          >
            {stats
              ? `${stats.backend.toUpperCase()} · ${stats.source === 'rust' ? 'RUST' : 'DEMO JS'}`
              : 'DÉTECTION…'}
          </span>
          {stats && (
            <>
              <span className="px-2 py-1 rounded bg-black/50 text-[#f8fafc] border border-[#1E293B]">
                {stats.fps} FPS
              </span>
              <span className="px-2 py-1 rounded bg-black/50 text-[#94a3b8] border border-[#1E293B]">
                {stats.instances} instances · {stats.drawCalls} draw calls
              </span>
            </>
          )}
        </div>

        {error && (
          <div className="absolute inset-x-0 top-1/2 -translate-y-1/2 flex justify-center pointer-events-none">
            <div className="max-w-xl px-4 py-3 rounded bg-[#7f1d1d] text-[#fecaca] text-xs font-mono border border-[#f43f5e]">
              {error}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
