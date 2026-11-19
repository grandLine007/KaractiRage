# Rapport de version — v0.6.4 « Orbite et zoom de la caméra 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.6.4` |
| Phase | 3D.10 — manipulation directe de la caméra |
| Date | 19 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.3.md`](RAPPORTS/RAPPORT_v0.6.3.md) |

## 1. Ce qui change

| Geste | Effet |
|---|---|
| **Bouton droit + glisser** | orbite autour de la cible (azimut + hauteur bornée ±83°) |
| **Molette** | rapproche/éloigne la caméra (rayon borné ×0,2–×20) |
| **Clic gauche + glisser sur un objet** | le déplace sur le sol (existant) |
| Indication | « clic = déplacer · droit = orbite · molette = zoom » dans la barre 3D |

L'orbite **écrit dans le moteur** (`engine_set_camera3d`, 20 envois/s max) : le paquet de
rendu relit la même caméra au tick suivant, donc aucun état fantôme côté Studio. Pendant
le geste, la caméra locale n'est pas écrasée par le paquet (il reflète encore l'ancienne
position tant que la commande n'est pas arrivée).

## 2. Preuves

```text
tsc → 0 erreur · build OK · lint → 14/0
cargo test --offline --workspace → 616 passed, 0 failed
export référence → 3 niveaux, 5078 Ko, moteur v0.6.4
npm run check:versions → 0.6.4 partout
```

## 3. Incident à signaler — l'auto-save a encore frappé

`assets/scenes/level_01.scene.json` avait été réécrit par l'application en cours
d'exécution : **22 tests** échouaient. Fichier restauré, tests repassés à 616/0. C'est la
troisième occurrence : l'auto-save du Studio écrit dans les scènes versionnées du dépôt.

**Ce qu'il faudra trancher** : le Studio devrait ignorer l'auto-save quand le projet est
sous `assets/` du dépôt (ou écrire dans `~/.karactirage/autosave`). C'est une décision
produit, pas un correctif que je m'accorde seul.

## 4. Limites inscrites

1. **Pas de test automatisé du geste** : l'orbite est côté client (math sphérique) et le
   mock ne transporte pas de paquet 3D. Preuve = build + typage + la recette.
2. Le pan (`maj + glisser`) n'existe pas : orbite + zoom seulement.
3. L'orbite ne modifie pas la **cible**, seulement l'angle et le rayon autour d'elle.
4. Le menu contextuel est désactivé dans le viewport 3D (nécessaire pour le bouton droit).

## 5. Validation

- [x] Gates vertes (§2)
- [x] Caméra orbite/zoom sur la même commande que le panneau et l'IA
- [x] Incident d'auto-save traité et signalé (§3)
- [ ] Gestes exécutés par un humain
- [ ] Tag `v0.6.4` créé par un humain
