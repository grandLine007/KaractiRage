// Spike Lot 0 — banc d'essai three.js (WebGPURenderer + TSL), chargé en import()
// dynamique et totalement isolé du bundle principal du Studio.
//
// Mesure : backend retenu (WebGPU/WebGL2), FPS soutenu, temps de frame, draw calls /
// triangles, et validation que les matériaux nœuds (TSL) compilent sur les deux backends
// (WGSL et GLSL). Toute erreur est remontée via onStats pour le HUD du spike.

import {
  WebGPURenderer,
  MeshStandardNodeMaterial,
  Scene,
  PerspectiveCamera,
  Mesh,
  InstancedMesh,
  BoxGeometry,
  PlaneGeometry,
  MeshStandardMaterial,
  Color,
  AmbientLight,
  DirectionalLight,
  Matrix4,
  Vector3,
  MathUtils,
} from 'three/webgpu';
import { float, uniform } from 'three/tsl';

export interface SpikeStats {
  backend: 'webgpu' | 'webgl2';
  fps: number;
  frameMs: number;
  drawCalls: number;
  triangles: number;
  instanceCount: number;
  tslOk: boolean;
  error?: string;
}

export type SpikeStatsListener = (stats: SpikeStats) => void;

export interface SpikeOptions {
  /** Force le backend WebGL2 (fallback), même si WebGPU est disponible. */
  forceWebGL?: boolean;
}

const INSTANCE_COUNT = 1000;
const GRID = Math.round(Math.cbrt(INSTANCE_COUNT));

export async function startSpike(
  container: HTMLElement,
  onStats: SpikeStatsListener,
  options: SpikeOptions = {},
): Promise<() => void> {
  const scene = new Scene();
  scene.background = new Color(0x0b0d14);

  const camera = new PerspectiveCamera(60, 1, 0.1, 200);
  camera.position.set(0, 7, 16);

  const renderer = new WebGPURenderer({ antialias: true, forceWebGL: options.forceWebGL });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  container.appendChild(renderer.domElement);

  const backend: 'webgpu' | 'webgl2' =
    (renderer.backend as { isWebGPUBackend?: boolean }).isWebGPUBackend === true
      ? 'webgpu'
      : 'webgl2';
  console.info('[spike3d] backend =', backend);

  // Sol : matériau nœud TSL — prouve la compilation WGSL (WebGPU) et GLSL (WebGL2).
  const groundMaterial = new MeshStandardNodeMaterial({
    colorNode: uniform(new Color(0x1b2130)),
    roughnessNode: float(0.9),
    metalnessNode: float(0.0),
  });
  const ground = new Mesh(new PlaneGeometry(80, 80), groundMaterial);
  ground.rotation.x = -Math.PI / 2;
  scene.add(ground);

  // 1 000 cubes instanciés (chemin instancing, matrices ré-uploadées chaque frame).
  const geometry = new BoxGeometry(0.6, 0.6, 0.6);
  const material = new MeshStandardMaterial({ roughness: 0.4, metalness: 0.15 });
  const meshes = new InstancedMesh(geometry, material, INSTANCE_COUNT);

  const dummy = new Matrix4();
  const pos = new Vector3();
  const color = new Color();
  for (let i = 0; i < INSTANCE_COUNT; i++) {
    const x = (i % GRID) - (GRID - 1) / 2;
    const y = (Math.floor(i / GRID) % GRID) - (GRID - 1) / 2;
    const z = Math.floor(i / (GRID * GRID)) - (GRID - 1) / 2;
    pos.set(x * 1.4, y * 1.4 + 0.4, z * 1.4);
    dummy.makeRotationY((i % 16) * 0.1);
    dummy.setPosition(pos);
    meshes.setMatrixAt(i, dummy);
    color.setHSL((i / INSTANCE_COUNT) * 0.9 + 0.5, 0.7, 0.55);
    meshes.setColorAt(i, color);
  }
  meshes.instanceMatrix.needsUpdate = true;
  if (meshes.instanceColor) meshes.instanceColor.needsUpdate = true;
  scene.add(meshes);

  scene.add(new AmbientLight(0xffffff, 0.35));
  const sun = new DirectionalLight(0xffffff, 2.2);
  sun.position.set(8, 14, 6);
  scene.add(sun);

  // Caméra orbitale minimale (drag = orbite, molette = zoom).
  let yaw = 0.6;
  let pitch = 0.35;
  let dist = 18;
  let dragging = false;
  let lastX = 0;
  let lastY = 0;

  const onPointerDown = (e: PointerEvent) => {
    dragging = true;
    lastX = e.clientX;
    lastY = e.clientY;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: PointerEvent) => {
    if (!dragging) return;
    yaw -= (e.clientX - lastX) * 0.005;
    pitch = MathUtils.clamp(pitch - (e.clientY - lastY) * 0.005, -1.2, 1.2);
    lastX = e.clientX;
    lastY = e.clientY;
  };
  const onPointerUp = (e: PointerEvent) => {
    dragging = false;
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
  };
  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    dist = MathUtils.clamp(dist * (1 + e.deltaY * 0.001), 4, 60);
  };

  const dom = renderer.domElement;
  dom.style.display = 'block';
  dom.style.width = '100%';
  dom.style.height = '100%';
  dom.style.touchAction = 'none';
  dom.addEventListener('pointerdown', onPointerDown);
  dom.addEventListener('pointermove', onPointerMove);
  dom.addEventListener('pointerup', onPointerUp);
  dom.addEventListener('wheel', onWheel, { passive: false });

  const resize = () => {
    const w = Math.max(1, container.clientWidth);
    const h = Math.max(1, container.clientHeight);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  };
  resize();
  const ro = new ResizeObserver(resize);
  ro.observe(container);

  let last = performance.now();
  let acc = 0;
  let frames = 0;
  let statTimer = 0;
  let tslOk = false;
  const tmp = new Matrix4();

  renderer.setAnimationLoop(() => {
    const now = performance.now();
    const dt = Math.min((now - last) / 1000, 0.1);
    last = now;

    // Stress du chemin d'upload des matrices d'instance (équivalent RenderPacket).
    for (let i = 0; i < INSTANCE_COUNT; i++) {
      meshes.getMatrixAt(i, tmp);
      const x = tmp.elements[12];
      const y = tmp.elements[13];
      const z = tmp.elements[14];
      tmp.makeRotationY((i % 16) * 0.1 + now * 0.0003 + i * 0.0001);
      tmp.elements[12] = x;
      tmp.elements[13] = y + Math.sin(now * 0.001 + i * 0.2) * 0.01;
      tmp.elements[14] = z;
      meshes.setMatrixAt(i, tmp);
    }
    meshes.instanceMatrix.needsUpdate = true;

    const cp = dist * Math.cos(pitch);
    camera.position.set(cp * Math.sin(yaw), dist * Math.sin(pitch), cp * Math.cos(yaw));
    camera.lookAt(0, 0, 0);

    try {
      renderer.render(scene, camera);
      tslOk = true;
    } catch (err) {
      onStats({
        backend,
        fps: 0,
        frameMs: 0,
        drawCalls: 0,
        triangles: 0,
        instanceCount: INSTANCE_COUNT,
        tslOk: false,
        error: err instanceof Error ? err.message : String(err),
      });
      renderer.setAnimationLoop(null);
      return;
    }

    acc += dt;
    frames++;
    statTimer += dt;
    if (statTimer >= 0.5) {
      const info = renderer.info;
      onStats({
        backend,
        fps: Math.round(frames / acc),
        frameMs: Math.round((acc / frames) * 1000 * 10) / 10,
        drawCalls: info.render.calls,
        triangles: info.render.triangles,
        instanceCount: INSTANCE_COUNT,
        tslOk,
      });
      frames = 0;
      acc = 0;
      statTimer = 0;
    }
  });

  return () => {
    renderer.setAnimationLoop(null);
    ro.disconnect();
    dom.removeEventListener('pointerdown', onPointerDown);
    dom.removeEventListener('pointermove', onPointerMove);
    dom.removeEventListener('pointerup', onPointerUp);
    dom.removeEventListener('wheel', onWheel);
    dom.remove();
    renderer.dispose();
    geometry.dispose();
    material.dispose();
    ground.geometry.dispose();
    groundMaterial.dispose();
  };
}
