// Aperçu 3D d'un modèle importé (C2) — même pile three.js que `scene3d.ts`
// (`three/webgpu`, repli WebGL2 automatique), mais **caméra indépendante** :
// le modèle importé est cadré sur sa bounding box et l'orbite reste libre.
//
// L'aperçu vit dans le Hub (section « Avancé ») et ne touche pas au RenderPacket
// du moteur : on ne veut ni voler la caméra de la scène de jeu, ni dépendre d'un
// projet ouvert pour prévisualiser un fichier avant de l'enregistrer.

import {
  AmbientLight,
  AnimationMixer,
  Box3,
  Color,
  DirectionalLight,
  GridHelper,
  Group,
  MathUtils,
  Mesh,
  PerspectiveCamera,
  Scene,
  Vector3,
  WebGPURenderer,
} from 'three/webgpu';
import type { Material, Object3D, Texture } from 'three/webgpu';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import type { GLTF } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';

/** Résumé affiché sous l'aperçu (valeurs lues dans la scène, pas dans le JSON). */
export interface MeshPreviewStats {
  /** Nom du fichier d'origine. */
  fileName: string;
  /** Format détecté par le magic/JSON. */
  format: 'glb' | 'gltf';
  /** Nombre de `Mesh` visibles. */
  meshes: number;
  /** Triangles cumulés (index / 3, sinon POSITION / 3). */
  triangles: number;
  /** Matériaux uniques (uuid), tableaux inclus. */
  materials: number;
  /** Nombre de clips d'animation présents. */
  animations: number;
  /** Noms des clips (pour l'affichage). */
  animationNames: string[];
  /** Textures uniques référencées par les matériaux. */
  textures: number;
  /** Nœuds du graphe glTF. */
  nodes: number;
  /** Taille du fichier en octets. */
  sizeBytes: number;
  /** Dimensions de la bounding box (unités du modèle). */
  bounds: { x: number; y: number; z: number };
}

export interface MeshPreviewController {
  /** Charge (ou remplace) le modèle affiché et renvoie ses mesures. */
  load: (buffer: ArrayBuffer, fileName: string) => Promise<MeshPreviewStats>;
  /** Retire le modèle courant sans détruire le renderer. */
  clear: () => void;
  /** Libère GPU, géométries, matériaux, textures et événements. */
  dispose: () => void;
}

/** Nombre de triangles d'un mesh (géométrie indexée ou non). */
function compterTriangles(mesh: Mesh): number {
  const geometrie = mesh.geometry;
  if (!geometrie) return 0;
  const index = geometrie.getIndex();
  if (index) return Math.floor(index.count / 3);
  const position = geometrie.getAttribute('position');
  return position ? Math.floor(position.count / 3) : 0;
}

/** Compte les textures uniques portées par un matériau (toutes propriétés). */
function compterTextures(materiau: Material, vues: Set<Texture>): void {
  for (const valeur of Object.values(materiau as unknown as Record<string, unknown>)) {
    const texture = valeur as Texture | null;
    if (texture && typeof texture === 'object' && texture.isTexture) {
      vues.add(texture);
    }
  }
}

function libererMateriau(materiau: Material, vues: Set<Texture>): void {
  compterTextures(materiau, vues);
  materiau.dispose();
}

/** Libère récursivement géométries, matériaux et textures d'un sous-arbre. */
function libererObjet(racine: Object3D): void {
  const textures = new Set<Texture>();
  const materiaux = new Set<Material>();
  racine.traverse((objet) => {
    const mesh = objet as Mesh;
    if (mesh.isMesh) {
      mesh.geometry?.dispose();
      const materiau = mesh.material;
      if (Array.isArray(materiau)) {
        for (const m of materiau) {
          if (m && !materiaux.has(m)) {
            materiaux.add(m);
            libererMateriau(m, textures);
          }
        }
      } else if (materiau && !materiaux.has(materiau)) {
        materiaux.add(materiau);
        libererMateriau(materiau, textures);
      }
    }
  });
  for (const texture of textures) texture.dispose();
}

/** Détecte le format sans se fier à l'extension (cohérent avec le backend). */
export function detecterFormatModele(buffer: ArrayBuffer): 'glb' | 'gltf' | null {
  const octets = new Uint8Array(buffer);
  if (octets.length >= 4 && octets[0] === 0x67 && octets[1] === 0x6c && octets[2] === 0x54 && octets[3] === 0x46) {
    return 'glb';
  }
  const debut = new TextDecoder().decode(octets.subarray(0, Math.min(octets.length, 512)));
  const premiere = debut.trimStart()[0];
  return premiere === '{' ? 'gltf' : null;
}

/**
 * Démarre l'aperçu dans `container` (renderer créé une fois, modèle rechargeable).
 *
 * Le renderer peut être WebGPU ou WebGL2 : `renderer.init()` choisit le backend,
 * exactement comme la preview moteur. Une erreur d'initialisation est remontée
 * à l'appelant (le Hub affiche un message au lieu de laisser une exception filer).
 */
export async function startMeshPreview(
  container: HTMLElement,
  onDisposeError?: (message: string) => void,
): Promise<MeshPreviewController> {
  const scene = new Scene();
  scene.background = new Color(0x0b0d14);

  const camera = new PerspectiveCamera(50, 1.5, 0.01, 5000);
  camera.position.set(2.5, 2.0, 4.0);

  const renderer = new WebGPURenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setSize(1, 1, false);
  const dom = renderer.domElement;
  dom.style.display = 'block';
  dom.style.position = 'absolute';
  dom.style.inset = '0';
  dom.style.width = '100%';
  dom.style.height = '100%';
  dom.style.touchAction = 'none';
  container.appendChild(dom);

  await renderer.init();

  const controls = new OrbitControls(camera, dom);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  controls.minDistance = 0.2;
  controls.maxDistance = 5000;
  controls.target.set(0, 0.5, 0);

  const hemisphere = new AmbientLight(0xffffff, 1.15);
  const soleil = new DirectionalLight(0xffffff, 2.4);
  soleil.position.set(5, 9, 6);
  const contre = new DirectionalLight(0x93c5fd, 0.9);
  contre.position.set(-6, 4, -5);
  scene.add(hemisphere, soleil, contre);

  const grille = new GridHelper(20, 20, 0x334155, 0x1e293b);
  grille.position.y = 0;
  scene.add(grille);

  const racine = new Group();
  scene.add(racine);
  let mixer: AnimationMixer | null = null;

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

  let rafId = 0;
  let disposed = false;
  let dernier = performance.now();
  const boucle = () => {
    if (disposed) return;
    rafId = requestAnimationFrame(boucle);
    const maintenant = performance.now();
    const dt = Math.min((maintenant - dernier) / 1000, 0.1);
    dernier = maintenant;
    mixer?.update(dt);
    controls.update();
    renderer.render(scene, camera);
  };
  rafId = requestAnimationFrame(boucle);

  const vider = () => {
    mixer?.stopAllAction();
    mixer = null;
    for (const enfant of [...racine.children]) {
      racine.remove(enfant);
      libererObjet(enfant);
    }
    grille.visible = false;
  };

  return {
    async load(buffer: ArrayBuffer, fileName: string) {
      const format = detecterFormatModele(buffer);
      if (!format) {
        throw new Error(
          'Format non reconnu : le fichier doit être un GLB (magic « glTF ») ou un glTF JSON autoporteur.',
        );
      }
      const loader = new GLTFLoader();
      // Consigne C2 : `GLTFLoader.parse(arrayBuffer, '')`. Le chemin vide
      // interdit les ressources externes ; un glTF multi-fichiers échoue ici
      // avec un message explicite plutôt que de charger un modèle incomplet.
      const gltf = await new Promise<GLTF>((resolve, reject) => {
        loader.parse(buffer, '', resolve, reject);
      });

      vider();

      const modele = gltf.scene;
      let boite = new Box3().setFromObject(modele);
      if (boite.isEmpty()) {
        throw new Error(
          'Le modèle ne contient aucune géométrie visible (0 mesh) : vérifie l’export dans l’outil 3D.',
        );
      }

      // Centre horizontal + pose au sol : l'orbite tourne autour du modèle, pas
      // d'un point arbitraire, et la grille reste sous ses pieds.
      const centre = boite.getCenter(new Vector3());
      modele.position.x -= centre.x;
      modele.position.z -= centre.z;
      modele.position.y -= boite.min.y;
      modele.updateMatrixWorld(true);
      boite = new Box3().setFromObject(modele);
      const taille = boite.getSize(new Vector3());

      racine.add(modele);

      const maxDimension = Math.max(taille.x, taille.y, taille.z, 0.001);
      grille.scale.setScalar(Math.max(1, maxDimension / 10));
      grille.visible = true;

      const rayon = maxDimension / 2;
      const fovRadians = MathUtils.degToRad(camera.fov);
      const distance = (rayon / Math.sin(fovRadians / 2)) * 1.35 + 0.5;
      const cible = new Vector3(0, taille.y * 0.5, 0);
      camera.near = Math.max(distance / 2000, 0.005);
      camera.far = distance * 50;
      camera.position.set(
        cible.x + distance * 0.62,
        cible.y + distance * 0.45,
        cible.z + distance,
      );
      camera.updateProjectionMatrix();
      controls.target.copy(cible);
      controls.minDistance = Math.max(maxDimension * 0.05, 0.05);
      controls.maxDistance = Math.max(distance * 12, 10);
      controls.update();

      // Mesures lues dans la scène three.js (pas dans le JSON source).
      let meshes = 0;
      let triangles = 0;
      let noeuds = 0;
      const materiaux = new Set<Material>();
      const textures = new Set<Texture>();
      modele.traverse((objet) => {
        noeuds++;
        const mesh = objet as Mesh;
        if (!mesh.isMesh) return;
        meshes++;
        triangles += compterTriangles(mesh);
        const materiau = mesh.material;
        const liste = Array.isArray(materiau) ? materiau : [materiau];
        for (const m of liste) {
          if (!m || materiaux.has(m)) continue;
          materiaux.add(m);
          compterTextures(m, textures);
        }
      });

      const animations = gltf.animations ?? [];
      if (animations.length > 0) {
        mixer = new AnimationMixer(modele);
        // Aperçu animé : le premier clip tourne en boucle. L'utilisateur peut
        // toujours orbiter pendant la lecture (aucune caméra imposée).
        mixer.clipAction(animations[0]).play();
      }

      return {
        fileName,
        format,
        meshes,
        triangles,
        materials: materiaux.size,
        animations: animations.length,
        animationNames: animations.map((clip, index) => clip.name || `clip_${index + 1}`),
        textures: textures.size,
        nodes: noeuds,
        sizeBytes: buffer.byteLength,
        bounds: {
          x: Math.round(taille.x * 1000) / 1000,
          y: Math.round(taille.y * 1000) / 1000,
          z: Math.round(taille.z * 1000) / 1000,
        },
      } satisfies MeshPreviewStats;
    },
    clear() {
      vider();
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      cancelAnimationFrame(rafId);
      ro.disconnect();
      controls.dispose();
      vider();
      scene.remove(grille);
      grille.geometry.dispose();
      const grilleMateriau = grille.material;
      if (Array.isArray(grilleMateriau)) {
        for (const m of grilleMateriau) m.dispose();
      } else {
        grilleMateriau.dispose();
      }
      scene.clear();
      dom.remove();
      try {
        renderer.dispose();
      } catch (erreur) {
        onDisposeError?.(erreur instanceof Error ? erreur.message : String(erreur));
      }
    },
  };
}
