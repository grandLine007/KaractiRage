# Rapport de version — v0.5.2 « Le GLB entre dans la scène »

| Champ | Valeur |
|---|---|
| Version | `v0.5.2` |
| Phase | C2 (suite) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.1.md`](RAPPORTS/RAPPORT_v0.5.1.md) |

> Retour du propriétaire : « l'aperçu s'affiche mais quand on continue, il n'y a rien ».
> Un import qui ne mène nulle part n'est pas un import. Cette version le branche.

## 1. Ce qui change

| Avant | Après |
|---|---|
| Aperçu du GLB seulement, aucun effet | **« Placer dans la scène »** crée une entité réelle, sauvegardée dans le `.karacti` |
| Manifeste 3D limité aux primitives intégrées | `MeshManifest::from_project` inclut les GLB importés (`Gltf { file }`) |
| Viewport 3D : cube de repli pour tout asset | chargeur glTF à la demande (octets via Tauri, cache, clone par instance, repli si échec) |
| Licence en champ libre | **liste** de licences connues (CC0, CC-BY, CC-BY-SA, CC-BY-NC, MIT, Apache-2.0, Inconnue, Autre…) |

## 2. Preuves

```text
Playwright (mock IPC, 1440×900) :
  licence = SELECT · 8 options · CC-BY-4.0 sélectionnée
  enregistrement → « triangle_test » · provenance Imported { tool: "host" } · CC-BY-4.0
  « Placer dans la scène » visible → clic → toast « Modèle placé dans la scène (entité 42). »
  aperçu : Meshes 1 · Triangles 1 · Matériaux 1 · Animations 0 · boîte 1×1×0

cargo test --offline --workspace → 610 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run build → OK · lint → 14/0
audit:ui --strict → 39 captures, 0 violation, 0 dérive
export référence → 3 niveaux, 5114 Ko, smoke : 3/3 niveaux, 4/4 écrans, 166,4 FPS
npm run check:versions → 0.5.2 partout
```

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Placement | entité `Renderable3D { mesh: MeshRef::Asset }` | le moteur savait déjà la porter ; il manquait le manifeste et le chargeur |
| Licence | identifiant canonique en liste | permet d'afficher les obligations d'attribution dans l'export (contrat §8.1) |
| Nom d'entité | phrase case (« Robot lourd ») | lisible ; le test l'exigeait, l'implémentation forçait le Title Case |
| Chargement glTF | à la demande, mis en cache, clone par instance | pas de rechargement par frame ; repli sans casser la frame |

## 4. Corrections apportées par le meneur

L'agent a échoué deux fois sur cette tâche. J'ai repris et corrigé :
1. emprunt invalide dans `engine_place_mesh_asset` (index calculé avant l'emprunt) ;
2. import inutilisé (avertissement) ;
3. casse du nom d'entité (phrase case) ;
4. fiche d'aide contradictoire (« le bouton arrivera » alors qu'il est là) ;
5. `assets/scenes/boss_01.scene.json` modifié par l'auto-save de l'application — restauré ;
6. un artefact `base_recipe_logic` généré par erreur — supprimé.

## 5. Limites inscrites

1. Le rendu du GLB dans le **viewport 3D** est branché mais n'a pas été validé dans une fenêtre Tauri réelle (le mock ne transporte pas de paquet de rendu avec un asset).
2. Toujours **pas de rendu 3D dans le jeu exporté** (runtime 2D).
3. glTF multi-fichiers refusé ; animations comptées, non jouées.
4. Pas d'échelle interactive dans la vue : le placement utilise l'échelle 1 (le paramètre existe côté commande).

## 6. Validation

- [x] Gates vertes (§2)
- [x] Import → placement → entité, prouvé par harnais
- [x] Licence en liste (8 choix) persistée
- [x] Limites inscrites (§5)
- [ ] Tag `v0.5.2` créé par un humain
