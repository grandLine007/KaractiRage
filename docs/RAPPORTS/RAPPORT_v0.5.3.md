# Rapport de version — v0.5.3 « Un projet 3D n'affiche plus d'outils 2D »

| Champ | Valeur |
|---|---|
| Version | `v0.5.3` |
| Phase | C2/F (cohérence 2D ↔ 3D) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.2.md`](RAPPORTS/RAPPORT_v0.5.2.md) |

> Retour du propriétaire : « quand on importe un GLB, on n'est plus dans de la 2D,
> donc les options concernant la 2D n'ont plus de sens, le rendu ainsi que le studio 2D
> aussi. »

## 1. La dimension devient un signal du produit

| Moment | Effet |
|---|---|
| Placement d'un maillage glTF | `metadata.dimension = "3D"` inscrit dans le `.karacti` |
| Ouverture / rafraîchissement | le Studio lit la dimension (`engine_project_dimension`) |
| Projet 3D | modes **Pixel** et **Vectoriel** masqués ; barre de tuiles masquée ; palette réduite à **Mesh 3D** ; viewport **3D** forcé ; badge `3D` |
| Projet 2D | interface inchangée |

Un mode 2D sélectionné avant le passage en 3D est ramené à « Créer » : on ne laisse pas
l'utilisateur dans un atelier qui n'existe plus.

## 2. Preuve — testée par le meneur, comme demandé

```text
Playwright (mock IPC, 1440×900), avant / après placement d'un GLB :

2D  → modes: 5 · onglets palette: 5 · outils tuiles: 2
3D  → modes: 3 · onglets palette: 1 · outils tuiles: 0
      badge: 3D · viewport 3D: true
```

```text
cargo test --offline --workspace → 610 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run build → OK · lint → 14/0
audit:ui --strict → 39 captures, 0 violation, 0 dérive
export référence → 3 niveaux, 5114 Ko, 167,2 FPS
npm run check:versions → 0.5.3 partout
```

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Où vit la dimension | métadonnée du projet | elle survit à la fermeture, se lit à l'ouverture |
| Qui bascule l'interface | le Studio, via une commande de lecture | pas de couplage : le moteur expose, l'interface décide |
| Onglet actif en 3D | dérivé, pas stocké | évite un `setState` dans un effet (lint + rendus en cascade) |
| Palette 3D | onglet unique « Mesh 3D » | les quatre autres onglets sont de la 2D |

## 4. Limites inscrites

1. La hiérarchie continue de lister les entités 2D existantes (un projet mixte reste
   possible) : elles ne sont pas masquées, seulement les outils qui les créent.
2. Le **jeu exporté** reste rendu en 2D ; le rendu 3D en jeu est le lot P4.
3. Aucun outil 3D dédié (caméra, lumières, échelle interactive) dans l'atelier : la
   palette ne propose que les meshes.
4. La dimension est écrite au placement, pas au simple import : importer sans placer ne
   transforme pas le projet.

## 5. Validation

- [x] Gates vertes (§2)
- [x] Test mené par le meneur (2D → 3D mesuré)
- [x] Les modes, tuiles et onglets 2D disparaissent en 3D
- [x] Limites inscrites (§4)
- [ ] Tag `v0.5.3` créé par un humain
