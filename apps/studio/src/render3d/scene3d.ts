// Pont Rust → three.js (tranche verticale Lot 4) : reçoit les `RenderPacket` du
// moteur (commande Tauri `engine_frame_packet`) et les applique à une scène three.js.
// Hors Tauri (`npm run dev`), un fallback de démonstration JS est utilisé.

import { invoke } from '@tauri-apps/api/core';
import {
  WebGPURenderer,
  Scene,
  PerspectiveCamera,
  MeshStandardMaterial,
  InstancedMesh,
  BoxGeometry,
  SphereGeometry,
  PlaneGeometry,
  CapsuleGeometry,
  Color,
  AmbientLight,
  DirectionalLight,
  PointLight,
  SpotLight,
  Object3D,
  Matrix4,
  Vector2,
  Vector3,
  Quaternion,
  Euler,
  Raycaster,
  Plane,
} from 'three/webgpu';
import { decodePacket, type DecodedPacket } from './packet';

export interface Scene3DStats {
  backend: 'webgpu' | 'webgl2';
  fps: number;
  instances: number;
  drawCalls: number;
  source: 'rust' | 'demo';
  error?: string;
}

export type Scene3DStatsListener = (stats: Scene3DStats) => void;

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

function hsvToArgb(h: number, s: number, v: number): number {
  const c = v * s;
  const hp = ((h * 6) % 6 + 6) % 6;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  let r = 0;
  let g = 0;
  let b = 0;
  if (hp < 1) {
    r = c;
    g = x;
  } else if (hp < 2) {
    r = x;
    g = c;
  } else if (hp < 3) {
    g = c;
    b = x;
  } else if (hp < 4) {
    g = x;
    b = c;
  } else if (hp < 5) {
    r = x;
    b = c;
  } else {
    r = c;
    b = x;
  }
  const m = v - c;
  const to8 = (f: number) => Math.round((f + m) * 255);
  return (0xff << 24) | (to8(r) << 16) | (to8(g) << 8) | to8(b);
}

/** Fallback de démonstration (hors Tauri) — miroir de `render_packet::demo_3d_packet`. */
function demoPacket(time: number, frameId: number): DecodedPacket {
  const n = 10;
  const cubes: DecodedPacket['batches'][number]['instances'] = [];
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      const x = (i - (n - 1) / 2) * 1.6;
      const z = (j - (n - 1) / 2) * 1.6;
      const y = 0.6 + Math.sin(time * 1.5 + i * 0.6 + j * 0.3) * 0.25;
      const rot = time * 0.6 + (i + j) * 0.13;
      const m = new Matrix4().compose(
        new Vector3(x, y, z),
        new Quaternion().setFromEuler(new Euler(0, rot, 0, 'XYZ')),
        new Vector3(0.9, 0.9, 0.9),
      );
      const arr = new Float32Array(16);
      m.toArray(arr);
      cubes.push({
        matrix: arr,
        prevMatrix: arr,
        tint: hsvToArgb(0.5 + (i / n) * 0.9, 0.7, 0.55),
        flags: 0,
        entityId: 0,
      });
    }
  }
  const gm = new Matrix4().compose(
    new Vector3(0, 0, 0),
    new Quaternion().setFromEuler(new Euler(-Math.PI / 2, 0, 0, 'XYZ')),
    new Vector3(40, 40, 1),
  );
  const garr = new Float32Array(16);
  gm.toArray(garr);
  return {
    frameId,
    camera: { fov: 60, aspect: 1.5, near: 0.1, far: 200, pos: [0, 7, 16], target: [0, 0, 0], up: [0, 1, 0] },
    ambient: [0.35, 0.35, 0.35],
    lights: [{ kind: 0, color: [1, 1, 1], intensity: 2.2, pos: [8, 14, 6], dir: [-0.5, -0.8, -0.4], range: 0, spotCosCutoff: 0 }],
    batches: [
      { meshId: 0, materialId: 0, instances: cubes },
      { meshId: 2, materialId: 0, instances: [{ matrix: garr, prevMatrix: garr, tint: 0xff1b2130, flags: 0, entityId: 0 }] },
    ],
  };
}

export interface Scene3DController {
  pause: () => void;
  resume: () => void;
  dispose: () => void;
}

export async function startScene3D(
  container: HTMLElement,
  onStats: Scene3DStatsListener,
  onPick?: (entityId: number | null) => void,
): Promise<Scene3DController> {
  const scene = new Scene();
  scene.background = new Color(0x0b0d14);

  const camera = new PerspectiveCamera(60, 1.5, 0.1, 200);
  camera.position.set(0, 7, 16);
  camera.lookAt(0, 0, 0);

  const renderer = new WebGPURenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  container.appendChild(renderer.domElement);
  const dom = renderer.domElement;
  dom.style.display = 'block';
  dom.style.width = '100%';
  dom.style.height = '100%';

  // L'initialisation choisit le backend (WebGPU, sinon repli WebGL2) et compile le
  // pipeline. Le backend n'est fiable qu'APRÈS cet await (le badge ne peut plus mentir).
  await renderer.init();

  const backend: 'webgpu' | 'webgl2' =
    (renderer.backend as { isWebGPUBackend?: boolean }).isWebGPUBackend === true
      ? 'webgpu'
      : 'webgl2';

  const ambient = new AmbientLight(0xffffff, 0.35);
  const sun = new DirectionalLight(0xffffff, 2.2);
  scene.add(ambient, sun);

  // Lumières ponctuelles / spot gérées dynamiquement (pool aligné sur `p.lights`).
  const dynamicLights: { kind: number; light: PointLight | SpotLight; target: Object3D | null }[] = [];

  function ensureDynamicLight(index: number, kind: number): (typeof dynamicLights)[number] {
    if (index < dynamicLights.length && dynamicLights[index].kind !== kind) {
      const old = dynamicLights[index];
      scene.remove(old.light);
      if (old.target) scene.remove(old.target);
    }
    if (index >= dynamicLights.length || dynamicLights[index].kind !== kind) {
      const entry =
        kind === 2
          ? (() => {
              const spot = new SpotLight(0xffffff, 1, 0, Math.PI / 4, 0.5, 2);
              return { kind: 2, light: spot, target: spot.target };
            })()
          : { kind: 1, light: new PointLight(0xffffff, 1, 0, 2), target: null };
      scene.add(entry.light);
      if (entry.target) scene.add(entry.target);
      if (index < dynamicLights.length) dynamicLights[index] = entry;
      else dynamicLights.push(entry);
    }
    return dynamicLights[index];
  }

  function applyLights(p: DecodedPacket) {
    // Ambiant : couleur × intensité déjà composée côté Rust (0..1 par canal).
    ambient.color.setRGB(p.ambient[0], p.ambient[1], p.ambient[2]);
    ambient.intensity = 1.0;

    // Directionnel : première lumière de type 0 (sinon soleil masqué).
    const dir = p.lights.find(l => l.kind === 0);
    if (dir) {
      sun.visible = true;
      sun.intensity = dir.intensity;
      sun.color.setRGB(dir.color[0], dir.color[1], dir.color[2]);
      sun.position.set(dir.pos[0], dir.pos[1], dir.pos[2]);
      if (dir.dir[0] !== 0 || dir.dir[1] !== 0 || dir.dir[2] !== 0) {
        sun.target.position.set(dir.pos[0] + dir.dir[0], dir.pos[1] + dir.dir[1], dir.pos[2] + dir.dir[2]);
        sun.target.updateMatrixWorld();
      }
    } else {
      sun.visible = false;
    }

    // Point / Spot : pool dynamique aligné sur les lumières de type 1 et 2.
    const others = p.lights.filter(l => l.kind === 1 || l.kind === 2);
    for (let i = 0; i < others.length; i++) {
      const lp = others[i];
      const entry = ensureDynamicLight(i, lp.kind);
      entry.light.color.setRGB(lp.color[0], lp.color[1], lp.color[2]);
      entry.light.intensity = lp.intensity;
      entry.light.position.set(lp.pos[0], lp.pos[1], lp.pos[2]);
      entry.light.visible = true;
      if (lp.range > 0) {
        entry.light.distance = lp.range;
        entry.light.decay = 2;
      }
      if (entry.target) {
        const spot = entry.light as SpotLight;
        entry.target.position.set(
          lp.pos[0] + lp.dir[0],
          lp.pos[1] + (lp.dir[1] !== 0 ? lp.dir[1] : -1),
          lp.pos[2] + lp.dir[2],
        );
        entry.target.updateMatrixWorld();
        const cutoff = Math.min(Math.max(lp.spotCosCutoff, 0.0), 1.0);
        spot.angle = Math.acos(cutoff) || Math.PI / 4;
      }
    }
    // Retire les lumières en surplus.
    while (dynamicLights.length > others.length) {
      const removed = dynamicLights.pop()!;
      scene.remove(removed.light);
      if (removed.target) scene.remove(removed.target);
    }
  }

  const geometryOf = (meshId: number) => {
    switch (meshId) {
      case 1:
        return new SphereGeometry(0.5, 24, 16);
      case 2:
        return new PlaneGeometry(1, 1);
      case 3:
        return new CapsuleGeometry(0.4, 1.0, 8, 16);
      default:
        return new BoxGeometry(1, 1, 1);
    }
  };

  const material = new MeshStandardMaterial({ roughness: 0.4, metalness: 0.15 });
  const pool = new Map<string, { mesh: InstancedMesh; entityIds: number[] }>();
  const tmpMatrix = new Matrix4();
  const tmpColor = new Color();
  const lerped = new Float32Array(16);
  let selectedId: number | null = null;

  function applyPacket(p: DecodedPacket, alpha: number): number {
    camera.fov = p.camera.fov;
    camera.near = p.camera.near;
    camera.far = p.camera.far;
    camera.position.set(p.camera.pos[0], p.camera.pos[1], p.camera.pos[2]);
    camera.lookAt(p.camera.target[0], p.camera.target[1], p.camera.target[2]);
    camera.updateProjectionMatrix();

    applyLights(p);

    let total = 0;
    for (const b of p.batches) {
      const key = `${b.meshId}:${b.materialId}`;
      let entry = pool.get(key);
      if (!entry) {
        entry = { mesh: new InstancedMesh(geometryOf(b.meshId), material, b.instances.length), entityIds: [] };
        pool.set(key, entry);
        scene.add(entry.mesh);
      }
      if (entry.mesh.count < b.instances.length) {
        entry.mesh.dispose();
        entry = { mesh: new InstancedMesh(geometryOf(b.meshId), material, b.instances.length), entityIds: [] };
        pool.set(key, entry);
        scene.add(entry.mesh);
      }
      entry.entityIds = new Array(b.instances.length);
      for (let i = 0; i < b.instances.length; i++) {
        const inst = b.instances[i];
        // Interpolation entre la frame précédente et la frame courante (lissage du
        // rendu quand la cadence IPC dérive de la cadence d'affichage).
        for (let k = 0; k < 16; k++) {
          lerped[k] = inst.prevMatrix[k] + (inst.matrix[k] - inst.prevMatrix[k]) * alpha;
        }
        tmpMatrix.fromArray(lerped);
        entry.mesh.setMatrixAt(i, tmpMatrix);
        tmpColor.setHex(inst.entityId === selectedId ? 0xffff00 : inst.tint & 0xffffff);
        entry.mesh.setColorAt(i, tmpColor);
        entry.entityIds[i] = inst.entityId;
      }
      entry.mesh.instanceMatrix.needsUpdate = true;
      if (entry.mesh.instanceColor) entry.mesh.instanceColor.needsUpdate = true;
      total += b.instances.length;
    }
    return total;
  }

  const resize = () => {
    const w = Math.max(1, container.clientWidth);
    const h = Math.max(1, container.clientHeight);
    if (w <= 1 || h <= 1) return; // conteneur masqué (display:none) — on ignore.
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  };
  resize();
  const ro = new ResizeObserver(resize);
  ro.observe(container);

  // Picking (sélection par clic) + déplacement sur le plan XZ (édition 3D).
  const raycaster = new Raycaster();
  const pointer = new Vector2();
  const groundPlane = new Plane(new Vector3(0, 1, 0), 0);
  const hitPoint = new Vector3();
  let dragging = false;

  const updatePointer = (e: PointerEvent) => {
    const rect = dom.getBoundingClientRect();
    pointer.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    pointer.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
  };

  const pick = (e: PointerEvent): number | null => {
    updatePointer(e);
    raycaster.setFromCamera(pointer, camera);
    let best: { distance: number; entityId: number } | null = null;
    for (const entry of pool.values()) {
      const hits = raycaster.intersectObject(entry.mesh, false);
      if (hits.length > 0 && hits[0].instanceId !== undefined) {
        const d = hits[0].distance;
        if (!best || d < best.distance) {
          const entityId = entry.entityIds[hits[0].instanceId];
          if (entityId !== undefined) best = { distance: d, entityId };
        }
      }
    }
    return best ? best.entityId : null;
  };

  const onPointerDown = (e: PointerEvent) => {
    const id = pick(e);
    selectedId = id;
    onPick?.(id);
    dragging = id !== null;
  };
  const onPointerMove = (e: PointerEvent) => {
    if (!dragging || selectedId === null) return;
    updatePointer(e);
    raycaster.setFromCamera(pointer, camera);
    if (raycaster.ray.intersectPlane(groundPlane, hitPoint)) {
      if (inTauri) {
        void invoke('engine_move_entity_3d', { id: selectedId, x: hitPoint.x, z: hitPoint.z });
      }
    }
  };
  const onPointerUp = () => {
    dragging = false;
  };

  dom.addEventListener('pointerdown', onPointerDown);
  dom.addEventListener('pointermove', onPointerMove);
  dom.addEventListener('pointerup', onPointerUp);
  dom.style.touchAction = 'none';

  let last = performance.now();
  let jsTime = 0;
  let frameId = 0;
  let acc = 0;
  let frames = 0;
  let statTimer = 0;
  let rafId = 0;
  let running = true;
  let disposed = false;

  // Découplage producteur (IPC) / consommateur (RAF) : la cadence d'affichage ne
  // dépend plus de la latence `invoke`. Le dernier paquet reçu est interpolé.
  let latestPacket: DecodedPacket | null = null;
  let packetDt = 1 / 60;
  let packetAt = performance.now();
  let prevRenderAt = performance.now();
  let fetching = false;

  async function nextPacket(dt: number): Promise<DecodedPacket> {
    if (inTauri) {
      const buf = await invoke<ArrayBuffer>('engine_frame_packet', { dt });
      return decodePacket(new Uint8Array(buf));
    }
    return demoPacket(jsTime, frameId);
  }

  async function fetchPacket(): Promise<void> {
    if (fetching) return;
    fetching = true;
    const now = performance.now();
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    jsTime += dt;
    frameId++;
    try {
      latestPacket = await nextPacket(dt);
      packetDt = dt;
      packetAt = performance.now();
    } catch (err) {
      onStats({
        backend,
        fps: 0,
        instances: 0,
        drawCalls: 0,
        source: inTauri ? 'rust' : 'demo',
        error: err instanceof Error ? err.message : String(err),
      });
      running = false;
    } finally {
      fetching = false;
    }
  }

  function loop() {
    if (!running || disposed) return;
    rafId = requestAnimationFrame(loop);
    // Récupération non bloquante (1 requête IPC max en vol).
    void fetchPacket();
    if (!latestPacket) return;

    const now = performance.now();
    const rdt = Math.min((now - prevRenderAt) / 1000, 0.25);
    prevRenderAt = now;
    const alpha = Math.min(Math.max((now - packetAt) / (packetDt * 1000), 0), 1);

    const instances = applyPacket(latestPacket, alpha);
    renderer.render(scene, camera);

    acc += rdt;
    frames++;
    statTimer += rdt;
    if (statTimer >= 0.5) {
      onStats({
        backend,
        fps: Math.round(frames / acc),
        instances,
        drawCalls: renderer.info.render.calls,
        source: inTauri ? 'rust' : 'demo',
      });
      frames = 0;
      acc = 0;
      statTimer = 0;
    }
  }
  rafId = requestAnimationFrame(loop);

  return {
    pause() {
      running = false;
      cancelAnimationFrame(rafId);
    },
    resume() {
      if (running || disposed) return;
      resize();
      running = true;
      last = performance.now();
      prevRenderAt = performance.now();
      rafId = requestAnimationFrame(loop);
    },
    dispose() {
      disposed = true;
      running = false;
      cancelAnimationFrame(rafId);
      ro.disconnect();
      dom.removeEventListener('pointerdown', onPointerDown);
      dom.removeEventListener('pointermove', onPointerMove);
      dom.removeEventListener('pointerup', onPointerUp);
      dom.remove();
      renderer.dispose();
      material.dispose();
      for (const entry of pool.values()) {
        entry.mesh.geometry.dispose();
        entry.mesh.dispose();
      }
    },
  };
}

