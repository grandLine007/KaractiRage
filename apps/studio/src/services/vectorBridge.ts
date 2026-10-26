import { invoke } from '@tauri-apps/api/core';

export interface VectorPath {
  points: [number, number][];
  closed: boolean;
}

export interface VectorLayer {
  color: string;
  paths: VectorPath[];
}

export interface VectorModel {
  width: number;
  height: number;
  layers: VectorLayer[];
}

export interface VectorAssetSummary {
  id: string;
  file: string;
}

export interface TraceOptions {
  colors: number;
  alpha_threshold: number;
  downscale_max: number;
  simplify: number;
}

export const DEFAULT_TRACE_OPTIONS: TraceOptions = {
  colors: 8,
  alpha_threshold: 32,
  downscale_max: 256,
  simplify: 1,
};

export async function importRaster(id: string, pngBase64: string): Promise<[number, number]> {
  return await invoke<[number, number]>('engine_import_raster', { id, pngBase64 });
}

export async function importSvg(id: string, svgText: string): Promise<[number, number]> {
  return await invoke<[number, number]>('engine_import_svg', { id, svgText });
}

export async function vectorizeRaster(id: string, pngBase64: string, opts: TraceOptions): Promise<string> {
  return await invoke<string>('engine_vectorize_raster', { id, pngBase64, optsJson: JSON.stringify(opts) });
}

export async function listVectorAssets(): Promise<VectorAssetSummary[]> {
  const raw = await invoke<string>('engine_list_vector_assets');
  try {
    return JSON.parse(raw) as VectorAssetSummary[];
  } catch {
    return [];
  }
}

export async function vectorLoad(id: string): Promise<VectorModel> {
  const raw = await invoke<string>('engine_vector_load', { id });
  return JSON.parse(raw) as VectorModel;
}

export async function vectorSave(id: string, model: VectorModel): Promise<string> {
  return await invoke<string>('engine_vector_save', { id, modelJson: JSON.stringify(model) });
}

export async function vectorSpawn(id: string): Promise<number> {
  return await invoke<number>('engine_vector_spawn', { id });
}

export function modelToSvg(model: VectorModel): string {
  let s = `<svg xmlns="http://www.w3.org/2000/svg" width="${model.width}" height="${model.height}" viewBox="0 0 ${model.width} ${model.height}">\n`;
  for (const layer of model.layers) {
    if (layer.paths.length === 0) continue;
    let d = '';
    for (const path of layer.paths) {
      if (path.points.length < 2) continue;
      d += `M ${path.points[0][0]} ${path.points[0][1]} `;
      for (const [x, y] of path.points.slice(1)) d += `L ${x} ${y} `;
      if (path.closed) d += 'Z ';
    }
    s += `  <path fill="${layer.color}" fill-rule="evenodd" d="${d.trim()}"/>\n`;
  }
  s += '</svg>\n';
  return s;
}

/** Décore un fichier image (PNG/JPEG/WebP/GIF) en PNG base64 via canvas. */
export async function fileToPngBase64(file: File): Promise<string> {
  const bmp = await createImageBitmap(file);
  const canvas = document.createElement('canvas');
  canvas.width = bmp.width;
  canvas.height = bmp.height;
  const ctx = canvas.getContext('2d')!;
  ctx.drawImage(bmp, 0, 0);
  const blob = await new Promise<Blob>((res, rej) => canvas.toBlob(b => (b ? res(b) : rej(new Error('encode'))), 'image/png'));
  return await new Promise<string>((res, rej) => {
    const r = new FileReader();
    r.onload = () => res((r.result as string).split(',')[1]);
    r.onerror = () => rej(r.error);
    r.readAsDataURL(blob);
  });
}
