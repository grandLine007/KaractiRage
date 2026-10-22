// Décodeur binaire RenderPacket v0 — miroir TypeScript de `render_packet.rs`.
// Pur (aucune dépendance three.js) : testable et partageable hors du chunk 3D.

export interface CameraPacket {
  fov: number;
  aspect: number;
  near: number;
  far: number;
  pos: [number, number, number];
  target: [number, number, number];
  up: [number, number, number];
}

export interface LightPacket {
  kind: number;
  color: [number, number, number];
  intensity: number;
  pos: [number, number, number];
  dir: [number, number, number];
  range: number;
  spotCosCutoff: number;
}

export interface InstancePacket {
  /** Matrice modèle, 16 floats colonne-major (ordre `Matrix4.elements`). */
  matrix: Float32Array;
  prevMatrix: Float32Array;
  /** Teinte prémultipliée `0xAARRGGBB`. */
  tint: number;
  flags: number;
  /** Identifiant de l'entité source (picking / édition). */
  entityId: number;
}

export interface BatchPacket {
  meshId: number;
  materialId: number;
  instances: InstancePacket[];
}

export interface DecodedPacket {
  frameId: number;
  camera: CameraPacket;
  ambient: [number, number, number];
  lights: LightPacket[];
  batches: BatchPacket[];
}

const MAGIC = 0x4b50524b;
const VERSION = 1;

export function decodePacket(buf: Uint8Array): DecodedPacket {
  const r = new Reader(buf);
  if (r.u32() !== MAGIC) throw new Error('RenderPacket: bad magic');
  if (r.u16() !== VERSION) throw new Error('RenderPacket: unsupported version');
  r.u16(); // flags réservés
  const frameId = r.u32();
  const batchCount = r.u32();
  const lightCount = r.u32();

  const camera: CameraPacket = {
    fov: r.f32(),
    aspect: r.f32(),
    near: r.f32(),
    far: r.f32(),
    pos: r.f32x3(),
    target: r.f32x3(),
    up: r.f32x3(),
  };
  const ambient = r.f32x3();

  const lights: LightPacket[] = [];
  for (let i = 0; i < lightCount; i++) {
    const kind = r.u8();
    r.u8(); // pad
    r.u16(); // pad
    lights.push({
      kind,
      color: r.f32x3(),
      intensity: r.f32(),
      pos: r.f32x3(),
      dir: r.f32x3(),
      range: r.f32(),
      spotCosCutoff: r.f32(),
    });
  }

  const batches: BatchPacket[] = [];
  for (let b = 0; b < batchCount; b++) {
    const meshId = r.u16();
    const materialId = r.u16();
    const instanceCount = r.u32();
    const instances: InstancePacket[] = [];
    for (let i = 0; i < instanceCount; i++) {
      instances.push({
        matrix: r.f32x(16),
        prevMatrix: r.f32x(16),
        tint: r.u32(),
        flags: r.u32(),
        entityId: r.u32(),
      });
    }
    batches.push({ meshId, materialId, instances });
  }

  return { frameId, camera, ambient, lights, batches };
}

class Reader {
  private pos = 0;
  private dv: DataView;

  constructor(buf: Uint8Array) {
    this.dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  }

  u8(): number {
    const v = this.dv.getUint8(this.pos);
    this.pos += 1;
    return v;
  }

  u16(): number {
    const v = this.dv.getUint16(this.pos, true);
    this.pos += 2;
    return v;
  }

  u32(): number {
    const v = this.dv.getUint32(this.pos, true);
    this.pos += 4;
    return v;
  }

  f32(): number {
    const v = this.dv.getFloat32(this.pos, true);
    this.pos += 4;
    return v;
  }

  f32x3(): [number, number, number] {
    return [this.f32(), this.f32(), this.f32()];
  }

  f32x(n: number): Float32Array {
    const out = new Float32Array(n);
    for (let i = 0; i < n; i++) out[i] = this.f32();
    return out;
  }
}
