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
  Group,
  Mesh,
  Matrix4,
  Vector2,
  Vector3,
  Quaternion,
  Euler,
  Raycaster,
  Plane,
  ArrowHelper,
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

export interface Scene3DOptions {
  /** Déplacement par le gizmo : position monde émise, throttle ~20/s + un envoi final. */
  onMoveAxis?: (entityId: number, position: [number, number, number]) => void;
}

export async function startScene3D(
  container: HTMLElement,
  onStats: Scene3DStatsListener,
  onPick?: (entityId: number | null) => void,
  options: Scene3DOptions = {},
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

  // --- Maillages glTF importés -------------------------------------------------
  // Le paquet transporte un `meshId` >= 4 pour un asset du manifeste. On charge le
  // binaire à la demande (une seule fois par id) et on clone la scène par instance.
  interface GltfEntry {
    root: Object3D | null;
    pending: boolean;
    failed: boolean;
    group: Group;
  }
  const gltfPool = new Map<number, GltfEntry>();
  /** Diagnostic pour les tests et l'UI : nombre de maillages glTF chargés. */
  (window as unknown as { __scene3dGltfAssets?: number }).__scene3dGltfAssets = 0;

  const ensureGltfLoaded = (meshId: number, entry: GltfEntry) => {
    if (entry.pending || entry.root || entry.failed) return;
    entry.pending = true;
    void (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const { GLTFLoader } = await import('three/examples/jsm/loaders/GLTFLoader.js');
        const base64 = await invoke<string>('engine_mesh_asset_bytes', { id: meshId });
        const binaire = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
        await new Promise<void>((resolve, reject) => {
          new GLTFLoader().parse(
            binaire.buffer,
            '',
            gltf => {
              entry.root = gltf.scene;
              entry.pending = false;
              (window as unknown as { __scene3dGltfAssets?: number }).__scene3dGltfAssets =
                ((window as unknown as { __scene3dGltfAssets?: number }).__scene3dGltfAssets ?? 0) + 1;
              resolve();
            },
            erreur => {
              entry.failed = true;
              entry.pending = false;
              console.warn(`Maillage 3D ${meshId} illisible : ${String(erreur)}`);
              reject(erreur);
            },
          );
        });
      } catch {
        // Repli : le cube coloré de l'instance reste affiché, la frame continue.
        entry.failed = true;
        entry.pending = false;
      }
    })();
  };

  /** Dessine les instances d'un batch de maillages glTF (clone par instance). */
  function applyGltfBatch(b: DecodedPacket['batches'][number], alpha: number): number {
    let entry = gltfPool.get(b.meshId);
    if (!entry) {
      entry = { root: null, pending: false, failed: false, group: new Group() };
      gltfPool.set(b.meshId, entry);
      scene.add(entry.group);
    }
    ensureGltfLoaded(b.meshId, entry);
    if (!entry.root) {
      // Ne jamais échouer en silence : si le binaire n'est pas encore là (ou est
      // illisible), on dessine un cube rouge à la place du maillage. L'auteur voit
      // qu'une entité existe, et la console dit pourquoi.
      if (entry.failed && entry.group.children.length !== b.instances.length) {
        for (const enfant of [...entry.group.children]) entry.group.remove(enfant);
        const cube = new Mesh(new BoxGeometry(1, 1, 1), new MeshStandardMaterial({ color: 0xf43f5e }));
        cube.matrixAutoUpdate = false;
        for (let i = 0; i < b.instances.length; i++) {
          entry.group.add(i === 0 ? cube : cube.clone());
        }
      }
      if (!entry.failed) return 0;
    }
    if (!entry.root) {
      for (let i = 0; i < b.instances.length; i++) {
        for (let k = 0; k < 16; k++) {
          lerped[k] = b.instances[i].prevMatrix[k] + (b.instances[i].matrix[k] - b.instances[i].prevMatrix[k]) * alpha;
        }
        const enfant = entry.group.children[i];
        if (enfant) enfant.matrix.fromArray(lerped);
      }
      return b.instances.length;
    }

    // Un clone par instance, reconstruit si le nombre change.
    if (entry.group.children.length !== b.instances.length) {
      for (const enfant of [...entry.group.children]) entry.group.remove(enfant);
      for (let i = 0; i < b.instances.length; i++) {
        const clone = entry.root.clone(true);
        clone.matrixAutoUpdate = false;
        // Le picking remonte du mesh touché jusqu'à ce porteur d'identifiant.
        clone.userData.entityId = b.instances[i]?.entityId;
        entry.group.add(clone);
      }
    }
    for (let i = 0; i < b.instances.length; i++) {
      const inst = b.instances[i];
      for (let k = 0; k < 16; k++) {
        lerped[k] = inst.prevMatrix[k] + (inst.matrix[k] - inst.prevMatrix[k]) * alpha;
      }
      const enfant = entry.group.children[i];
      if (enfant) enfant.matrix.fromArray(lerped);
      if (inst.entityId === selectedId) { matriceSelection.fromArray(lerped); matriceSelectionValide = true; }
    }
    return b.instances.length;
  }
  const tmpMatrix = new Matrix4();
  const tmpColor = new Color();
  const lerped = new Float32Array(16);
  let selectedId: number | null = null;
  // Gizmo de translation : trois poignées d'axe à la position de l'entité sélectionnée.
  const axesMonde = [new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(0, 0, 1)];
  const couleursAxe = [0xff5a5a, 0x5aff7a, 0x5a9bff];
  const gizmoGroupe = new Group();
  const gizmoFleches: ArrowHelper[] = axesMonde.map((dir, i) => {
    const fleche = new ArrowHelper(dir, new Vector3(), 1, couleursAxe[i], 0.2, 0.08);
    fleche.userData.axis = i;
    return fleche;
  });
  gizmoFleches.forEach(fleche => gizmoGroupe.add(fleche));
  gizmoGroupe.visible = false;
  scene.add(gizmoGroupe);
  const matriceSelection = new Matrix4();
  let matriceSelectionValide = false;
  // Glisser de poignée : axe + position initiale + plan de projection.
  let axeGlisse: Vector3 | null = null;
  let posDepartGlisse = new Vector3();
  let planGlisse = new Plane();
  let hitDepartGlisse = new Vector3();
  let dernierEnvoiGizmo = 0;
  // Orbite caméra (bouton droit) : la cible vient du paquet, l'angle vient du geste.
  const lastTarget = new Vector3();
  let orbiting = false;
  let orbitPX = 0;
  let orbitPY = 0;
  // Panoramique (bouton central) : caméra et cible glissent ensemble sur le plan de vue.
  const panDroite = new Vector3();
  const panHaut = new Vector3();
  let panning = false;
  let lastOrbitSend = 0;

  function applyPacket(p: DecodedPacket, alpha: number): number {
    matriceSelection.identity();
    matriceSelectionValide = false;
    camera.fov = p.camera.fov;
    camera.near = p.camera.near;
    camera.far = p.camera.far;
    lastTarget.set(p.camera.target[0], p.camera.target[1], p.camera.target[2]);
    if (!orbiting) {
      // Pendant une orbite, on garde la position locale : le paquet reflète encore
      // l'ancienne caméra tant que la commande n'a pas atteint le moteur.
      camera.position.set(p.camera.pos[0], p.camera.pos[1], p.camera.pos[2]);
      camera.lookAt(p.camera.target[0], p.camera.target[1], p.camera.target[2]);
      camera.updateProjectionMatrix();
    }

    applyLights(p);

    let total = 0;
    for (const b of p.batches) {
      // meshId >= 4 : maillage glTF du manifeste (les primitives sont 0-3).
      if (b.meshId >= 4) {
        total += applyGltfBatch(b, alpha);
        continue;
      }
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
        if (inst.entityId === selectedId) { matriceSelection.copy(tmpMatrix); matriceSelectionValide = true; }
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
    // Maillages glTF : intersection récursive, puis remontée vers l'entité.
    for (const entry of gltfPool.values()) {
      if (!entry.root) continue;
      const hits = raycaster.intersectObjects(entry.group.children, true);
      if (hits.length > 0 && (!best || hits[0].distance < best.distance)) {
        let objet: Object3D | null = hits[0].object;
        while (objet && typeof objet.userData.entityId !== 'number') objet = objet.parent;
        if (objet && typeof objet.userData.entityId === 'number') {
          best = { distance: hits[0].distance, entityId: objet.userData.entityId };
        }
      }
    }
    return best ? best.entityId : null;
  };

  /** Orbite autour de la cible : envoie la nouvelle caméra au moteur (throttlé). */
  const envoyerCamera = (immediat: boolean) => {
    if (!inTauri) return;
    const now = performance.now();
    if (!immediat && now - lastOrbitSend < 50) return;
    lastOrbitSend = now;
    void invoke('engine_set_camera3d', {
      position: [camera.position.x, camera.position.y, camera.position.z],
      target: [lastTarget.x, lastTarget.y, lastTarget.z],
    });
  };

  const onPointerDown = (e: PointerEvent) => {
    // Bouton droit = orbite caméra ; bouton gauche = sélection + déplacement au sol.
    if (e.button === 2) {
      orbiting = true;
      orbitPX = e.clientX;
      orbitPY = e.clientY;
      e.preventDefault();
      return;
    }
    if (e.button === 1) {
      panning = true;
      orbitPX = e.clientX;
      orbitPY = e.clientY;
      e.preventDefault();
      return;
    }
    const id = pick(e);
    selectedId = id;
    onPick?.(id);
    dragging = id !== null;
    // Gizmo : une poignée d'axe attrape le geste à la place du déplacement au sol.
    if (id !== null && matriceSelectionValide) {
      updatePointer(e);
      raycaster.setFromCamera(pointer, camera);
      const touches = raycaster.intersectObjects(gizmoFleches as unknown as Object3D[], true);
      if (touches.length > 0) {
        let objet: Object3D | null = touches[0].object;
        while (objet && typeof objet.userData.axis !== 'number') objet = objet.parent;
        if (objet && typeof objet.userData.axis === 'number') {
          const axe = axesMonde[objet.userData.axis];
          posDepartGlisse.setFromMatrixPosition(matriceSelection);
          const vue = camera.position.clone().sub(posDepartGlisse).normalize();
          const normale = new Vector3().crossVectors(axe, vue);
          if (normale.lengthSq() < 1e-8) normale.set(0, 1, 0);
          planGlisse.setFromNormalAndCoplanarPoint(normale, posDepartGlisse);
          raycaster.ray.intersectPlane(planGlisse, hitDepartGlisse);
          axeGlisse = axe;
          dragging = false;
        }
      }
    }
  };
  const onPointerMove = (e: PointerEvent) => {
    if (axeGlisse && selectedId !== null) {
      updatePointer(e);
      raycaster.setFromCamera(pointer, camera);
      const point = new Vector3();
      if (!raycaster.ray.intersectPlane(planGlisse, point)) return;
      const delta = point.sub(hitDepartGlisse).dot(axeGlisse);
      const nouvelle = posDepartGlisse.clone().addScaledVector(axeGlisse, delta);
      matriceSelection.setPosition(nouvelle);
      const maintenant = performance.now();
      if (maintenant - dernierEnvoiGizmo > 50) {
        dernierEnvoiGizmo = maintenant;
        options.onMoveAxis?.(selectedId, [nouvelle.x, nouvelle.y, nouvelle.z]);
      }
      return;
    }
    if (panning) {
      const dx = e.clientX - orbitPX;
      const dy = e.clientY - orbitPY;
      orbitPX = e.clientX;
      orbitPY = e.clientY;
      panDroite.setFromMatrixColumn(camera.matrix, 0);
      panHaut.setFromMatrixColumn(camera.matrix, 1);
      const k = camera.position.distanceTo(lastTarget) * 0.0015;
      const glissement = panDroite.multiplyScalar(-dx * k).add(panHaut.multiplyScalar(dy * k));
      camera.position.add(glissement);
      lastTarget.add(glissement);
      camera.lookAt(lastTarget);
      camera.updateProjectionMatrix();
      envoyerCamera(false);
      return;
    }
    if (orbiting) {
      const dx = (e.clientX - orbitPX) * 0.005;
      const dy = (e.clientY - orbitPY) * 0.005;
      orbitPX = e.clientX;
      orbitPY = e.clientY;
      const cible = lastTarget;
      const decalage = camera.position.clone().sub(cible);
      const rayon = Math.max(decalage.length(), 0.5);
      let azimut = Math.atan2(decalage.x, decalage.z) - dx;
      let hauteur = Math.asin(Math.min(1, Math.max(-1, decalage.y / rayon))) - dy;
      hauteur = Math.min(1.45, Math.max(-1.45, hauteur));
      camera.position.set(
        cible.x + rayon * Math.cos(hauteur) * Math.sin(azimut),
        cible.y + rayon * Math.sin(hauteur),
        cible.z + rayon * Math.cos(hauteur) * Math.cos(azimut),
      );
      camera.lookAt(cible);
      camera.updateProjectionMatrix();
      envoyerCamera(false);
      return;
    }
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
    if (axeGlisse && selectedId !== null) {
      const position = new Vector3().setFromMatrixPosition(matriceSelection);
      options.onMoveAxis?.(selectedId, [position.x, position.y, position.z]);
      axeGlisse = null;
    }
    if (orbiting || panning) {
      orbiting = false;
      panning = false;
      envoyerCamera(true);
    }
    dragging = false;
  };
  /** Molette : rapproche ou éloigne la caméra de la cible, sans changer l'angle. */
  const onWheel = (e: WheelEvent) => {
    const cible = lastTarget;
    const decalage = camera.position.clone().sub(cible);
    const facteur = Math.exp(e.deltaY * 0.001);
    const nouveau = decalage.multiplyScalar(Math.min(20, Math.max(0.2, facteur)));
    camera.position.copy(cible).add(nouveau);
    camera.lookAt(cible);
    camera.updateProjectionMatrix();
    envoyerCamera(false);
  };

  dom.addEventListener('pointerdown', onPointerDown);
  dom.addEventListener('pointermove', onPointerMove);
  dom.addEventListener('pointerup', onPointerUp);
  dom.addEventListener('wheel', onWheel, { passive: true });
  const onContextMenu = (e: Event) => e.preventDefault();
  dom.addEventListener('contextmenu', onContextMenu);
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

  /** Positionne les poignées sur l'entité sélectionnée, taille liée à la distance. */
  function mettreAJourGizmo() {
    const actif = selectedId !== null && !orbiting && !panning && !axeGlisse;
    gizmoGroupe.visible = selectedId !== null && !orbiting && !panning;
    if (!gizmoGroupe.visible) return;
    const position = new Vector3().setFromMatrixPosition(matriceSelection);
    const distance = Math.max(camera.position.distanceTo(position), 0.1);
    const taille = Math.max(0.6, distance * 0.12);
    for (const fleche of gizmoFleches) {
      fleche.position.copy(position);
      fleche.setLength(taille, taille * 0.22, taille * 0.09);
    }
    void actif;
  }

  function loop() {
    if (!running || disposed) return;
    // Hameçon de test : expose l'état du gizmo et de la sélection au harnais.
  if (typeof window !== 'undefined') {
    (window as unknown as { __scene3dDebug?: unknown }).__scene3dDebug = {
      gizmoVisible: () => gizmoGroupe.visible,
      selection: () => selectedId,
    };
  }

  rafId = requestAnimationFrame(loop);
    // Récupération non bloquante (1 requête IPC max en vol).
    void fetchPacket();
    if (!latestPacket) return;

    const now = performance.now();
    const rdt = Math.min((now - prevRenderAt) / 1000, 0.25);
    prevRenderAt = now;
    const alpha = Math.min(Math.max((now - packetAt) / (packetDt * 1000), 0), 1);

    const instances = applyPacket(latestPacket, alpha);
    mettreAJourGizmo();
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
      dom.removeEventListener('wheel', onWheel);
      dom.removeEventListener('contextmenu', onContextMenu);
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

