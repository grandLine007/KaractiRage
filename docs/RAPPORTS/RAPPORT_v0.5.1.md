# Rapport de version — v0.5.1 « Importer un modèle 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.5.1` |
| Phase | C2 du contrat global (§2.1, §8.3) |
| Date | 17 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.0.md`](RAPPORTS/RAPPORT_v0.5.0.md) |

> Première porte d'entrée 3D : un `.glb` / `.gltf` s'importe, s'affiche et s'enregistre
> avec sa provenance. Le rendu 3D dans le jeu exporté reste hors périmètre (runtime 2D).

## 1. Ce qui est livré

| Étape | Détail |
|---|---|
| Importer | `.glb` / `.gltf` depuis la section **Avancé** du Hub, jamais dans le parcours principal |
| Voir | chargement dans la prévisualisation three.js existante, orbite et zoom, caméra cadrée |
| Résumer | meshes, triangles, matériaux, animations, nœuds, textures, boîte englobante, format, taille |
| Enregistrer | fichier copié dans `<projet>/assets/meshes/<id>.glb`, entrée de manifeste (`kind: Mesh`, hash, provenance `Imported { tool: "host" }`, licence) |
| Hors Tauri | l'aperçu fonctionne, l'enregistrement est désactivé avec un message explicite |

## 2. Preuve

```text
GLB de test : triangle, 444 octets (JSON + BIN, glTF 2.0)
browser (1440×900, mock IPC) :
  Meshes 1 · Triangles 1 · Matériaux 1 · Animations 0
  triangle.glb · 444 o · 2 nœuds · 0 texture · boîte 1 × 1 × 0
  canvas three.js présent, hint « glisser pour orbiter · molette pour zoomer »
```

```text
npm run build → OK · lint → 14/0
npm run audit:ui -- --strict → 39 captures, 0 violation, 0 dérive
cargo test --offline --workspace → 605 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
export référence → 3 niveaux, 5114 Ko, 216,7 FPS
npm run check:versions → 0.5.1 partout
```

## 3. Décision : placement non proposé, et pourquoi

Le `RenderPacket` ne transporte que des **identifiants de primitives** (cube, sphère,
plan, capsule). Un GLB importé n'a donc pas de représentation dans le paquet de rendu :
proposer « Placer dans la scène » aurait créé une entité qui n'aurait rien affiché dans
le jeu ni dans l'aperçu du viewport. L'interface le dit explicitement plutôt que de
laisser croire que le modèle est utilisable en jeu.

C'est la règle du contrat §2.2 : le Studio orchestre, il ne promet pas ce que le runtime
ne sait pas faire.

## 4. Limites inscrites

1. **Pas de rendu en jeu** : le runtime exporté est 2D ; le 2.5D/3D déclaratif est un lot
   séparé (P4 du plan).
2. **Placement en scène** non proposé (voir §3) : il faudra étendre le `RenderPacket` aux
   meshes glTF.
3. **glTF multi-fichiers** (`.gltf` + `.bin` + textures externes) : l'interface le signale
   comme non supporté ; seul l'autoporteur (GLB, ou glTF avec données embarquées) est
   promis.
4. **Animations** : comptées et annoncées, non jouées dans l'aperçu.
5. **Corrections d'un agent interrompu** : deux erreurs de syntaxe et un nettoyage
   manquant ont été réparés par le meneur avant intégration.

## 5. Validation

- [x] Gates vertes (§2)
- [x] GLB réel chargé et résumé mesuré
- [x] Provenance et licence enregistrées
- [x] Limites inscrites (§4)
- [ ] Tag `v0.5.1` créé par un humain
