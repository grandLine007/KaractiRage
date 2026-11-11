# Versions

> Source de vérité de la séquence de versions. **Aucun tag sans rapport** :
> `docs/RAPPORTS/RAPPORT_vX.Y.Z.md` est écrit avant le tag, sinon la release est refusée.

| Version | Phase | Thème | Rapport | Statut |
|---|---|---|---|---|
| v0.2.0 | — | Édition IA inline (contrat, ops, prompt, barre + `✦`) | [RAPPORT_v0.2.0](RAPPORTS/RAPPORT_v0.2.0.md) | rétroactif (non taguée) |
| v0.3.0 | P1 | Boucle « jeu 2D complet » | [RAPPORT_v0.3.0](RAPPORTS/RAPPORT_v0.3.0.md) | **prêt, non taguée** |
| v0.3.1 | fin P1 + P2.1 | AC2 (replay hashé), UI sauvegarde, pré-vol UI, manifeste d'assets | [RAPPORT_v0.3.1](RAPPORTS/RAPPORT_v0.3.1.md) | **prêt, non taguée** |
| v0.4.0 | P2 | Import Tiled, audio par fichiers, rechargement à chaud | à écrire | à venir |
| v0.5.0 | P3 | Packs image/audio locaux (endpoint compatible OpenAI) | à écrire | à venir |
| v0.6.0 | P4 | glTF, 3D déclarative, 2.5D, Blender MCP | à écrire | à venir |
| v0.7.0 | P5 | Qualité IA mesurée (40 × 10, seuils) | à écrire | à venir |
| v1.0.0 | P6 | Exports HTML5 / desktop / WASM | à écrire | à venir |

## Documents de phase

| Phase | Document de référence |
|---|---|
| P1 | [P1_JEU_2D_COMPLET.md](P1_JEU_2D_COMPLET.md) (figé) |
| P2 → P6 | [PLAN_INTEGRATION_OUTILS.md](PLAN_INTEGRATION_OUTILS.md) |
| Cadre général | [STRATEGIE_CREATION_2D_3D_IA.md](STRATEGIE_CREATION_2D_3D_IA.md) |
| Modèle de rapport | [RAPPORTS/TEMPLATE_RAPPORT_VERSION.md](RAPPORTS/TEMPLATE_RAPPORT_VERSION.md) |

## Règle de versionnement

- **Une version = une phase**, pas un lot de commits.
- Le numéro suit SemVer : `0.x` tant que l'objectif final n'est pas atteint, `1.0.0` quand
  les 3 exports et les 40 instructions sont prouvés.
- Les versions de code (`Cargo.toml`, `package.json`, badge UI) **doivent** afficher la
  version du tableau : c'est une dette connue de la v0.2.0, corrigée en v0.3.0.
- Un écart entre le plan et la réalité ne change pas le numéro : il est inscrit dans le
  rapport de la version (§6, registre de flexibilité).
