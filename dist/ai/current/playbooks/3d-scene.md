# Playbook — Scène 3D / 2.5D (déclaratif)

Rendu 3D, logique/collisions 2D. **Jamais** de code three.js/TSL.

## Structure type

1. Entités avec `mesh_primitive` (`cube`, `sphere`, `plane`, `capsule`) ou `mesh_asset`.
2. `position3d: [x, y, z]`, `rotation_euler_deg: [x, y, z]`, `scale3d: [sx, sy, sz]`.
3. KVL identique au 2D (mêmes événements/actions).

## Limites

- `MaterialRef` figé sur `Solid` (roughness 0.4) dans le payload IA.
- Pas de caméra/lumières paramétrables exposées au LLM dans le dictionnaire.
- 2.5D d'abord : collisions 2D, rendu 3D. Pas de physique 3D.

## Anti-modèles

- Ne pas générer de code de rendu, de shaders ou de matériaux arbitraires.
