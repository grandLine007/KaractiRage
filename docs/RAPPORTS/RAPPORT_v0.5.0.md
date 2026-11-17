# Rapport de version — v0.5.0 « Créer un jeu en 3 gestes »

| Champ | Valeur |
|---|---|
| Version | `v0.5.0` |
| Phase | P1 (critère §3.1 « démarrer sans expertise » réparé) |
| Date | 17 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.4.5.md`](RAPPORTS/RAPPORT_v0.4.5.md) |

> Le propriétaire a dit : « je ne sais pas comment créer un jeu ». Le moteur y arrivait
> en 3,3 s ; l'interface, non. Cette version répare l'entrée du produit.

## 1. Ce qui change

| Avant | Après |
|---|---|
| Trois cartes de valeur inégale, bande Tiled, formulaire technique (genre, dimension, archétype, destination) | un CTA dominant **« Créer un jeu »**, deux actions secondaires |
| Formulaire à cinq champs | **2 écrans** : nom pré-rempli + **3 modèles** visuels → « Créer et jouer » |
| Aucun retour sur le temps | **« Prêt en N s »** dans le pied de page, mesuré par le produit |
| Arrivée dans le Studio sans repère | aide discrète « Flèches · F · E », disparaît à la première touche |
| Tiled, JSON, réglages dans le chemin principal | dans **« Avancé »**, replié par défaut |

## 2. Preuves

```text
npm run build → OK · lint → 14/0
npm run audit:ui -- --strict → 39 captures, 0 violation, 0 dérive (baseline régénérée)
parcours Playwright → CTA visible · Tiled masqué · 3 modèles · chrono « Prêt en 4 s »
cargo test --offline --workspace → 598 passed
export référence → 3 niveaux, 5109 Ko, smoke : 3/3 niveaux, 4/4 écrans, 286,8 FPS
npm run check:versions → 0.5.0 partout
```

## 3. Recette humaine

[`RECETTE_DEMARRAGE.md`](RECETTE_DEMARRAGE.md) : trois gestes, ce que l'on doit voir,
et quoi noter en cas d'échec. **C'est la validation qui manquait aux versions
précédentes** : les tests automatiques vérifiaient le résultat, pas l'accès.

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Hiérarchie de l'accueil | un seul CTA dominant | un débutant ne choisit pas entre trois chemins |
| Champs techniques | déplacés dans « Avancé » | ils ne servent pas la première création |
| Chrono | mesuré **dans l'application** | le harnais mesurait le moteur, pas le parcours vécu |
| Aide aux commandes | non bloquante, une fois par session | ne pas transformer l'aide en obstacle |

## 5. Limites inscrites

1. Le chrono mesure du Hub affiché au premier rendu jouable : il n'inclut pas le
   lancement de l'application.
2. La recette n'a pas encore été exécutée par le propriétaire — c'est la prochaine étape.
3. Le glisser-déposer Tiled n'est validé qu'en IPC simulé (pas de session GUI dans
   l'environnement d'agent).
4. L'aide aux commandes ne mentionne pas encore la souris ni le menu `Échap`.

## 6. Validation

- [x] Gates vertes (§2)
- [x] Parcours mesuré par harnais : 3 gestes, chrono affiché
- [x] Baseline d'audit régénérée sur décision motivée (4 captures Hub)
- [x] Recette humaine écrite pour le propriétaire
- [ ] Recette exécutée par un humain
- [ ] Tag `v0.5.0` créé par un humain
