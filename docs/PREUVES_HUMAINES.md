# Preuves humaines

> Ce que les tests automatiques ne peuvent pas prouver : l'expérience réelle.
> Chaque entrée cite la personne, la date, le geste testé et la réponse littérale.

## 1. Parcours de démarrage — 17 novembre 2026

**Testeur** : propriétaire du projet.
**Contexte** : Studio lancé, fenêtre ouverte, aucun mode d'emploi lu.

| Question | Réponse |
|---|---|
| Vois-tu le bouton « Créer un jeu » ? | **oui** |
| Le jeu s'affiche-t-il vite après le choix du type ? | **oui**, « ça s'affiche rapidement » |
| Le chrono a-t-il été relevé ? | non demandé |
| Un accroc rencontré ? | aucun signalé |

**Conclusion** : le parcours en 3 gestes est trouvé sans documentation. C'est la
validation du critère §3.1 du contrat global — celle que les 598 tests et le harnais
AC1 (3,3 s) ne pouvaient pas fournir.

**Ce qui reste à mesurer** : la valeur du chrono affichée dans le pied de page, et le
temps ressenti (2 s ? 8 s ?) : « rapidement » n'est pas un chiffre.

## 2. Historique

| Date | Sujet | Résultat |
|---|---|---|
| 17/11/2026 | Parcours « Créer un jeu » | validé (trouvé, rapide) |
