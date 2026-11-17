# Recette de démarrage — 2 minutes

> **But** : vérifier qu'une personne qui n'a jamais vu KaractiRage crée un jeu jouable
> sans lire de documentation. C'est le critère §3.1 du contrat global, et le seul qu'un
> test automatique ne peut pas vraiment valider.

## Ce qu'il faut

- L'application ouverte (`./target/debug/karactirage-studio`).
- Un chronomètre (celui du téléphone suffit).

## Les 3 gestes

| # | Geste | Ce que tu dois voir | Temps |
|---|---|---|---|
| 1 | Clique sur le gros bouton violet **« Créer un jeu »** | un écran avec un nom déjà rempli et **trois modèles** (Plateforme, Aventure, Boss) | ~2 s |
| 2 | Garde le nom, choisis un modèle, clique **« Créer et jouer »** | le Studio s'ouvre en mode **Jouer** | ~3 s |
| 3 | Regarde le pied de page | **« Prêt en N s »**, et en bas à gauche l'aide « Flèches pour bouger · F pour tirer · E pour interagir » | immédiat |

**Total attendu : moins de 10 secondes.** Le contrat vise moins de 2 minutes, avec un
débutant qui lit les écrans.

## Ce qu'il faut vérifier au passage

| Point | Attendu |
|---|---|
| Le bouton « Avancé » | replié par défaut ; il contient Tiled, l'import JSON et les réglages techniques |
| Rien de technique visible | ni genre, ni dimension, ni archétype, ni dossier de destination |
| L'aide aux commandes | disparaît à la première touche, ne bloque jamais le jeu |
| Le chrono | une seule fois par session, et seulement après une création réelle |
| La sortie | `Échap` ferme le Hub ; le projet reste ouvert |

## Si ça ne marche pas

Note **l'étape** (1, 2 ou 3), **ce que tu vois à la place**, et une capture d'écran.

| Symptôme | À me dire |
|---|---|
| Pas de bouton violet | ce qui s'affiche à la place (Studio vide ? écran de chargement ?) |
| « Créer et jouer » ne fait rien | un message d'erreur apparaît-il ? |
| Le jeu s'ouvre mais on ne peut pas bouger | la fenêtre a-t-elle le focus ? la touche `Entrée` fait-elle quelque chose ? |
| Pas de chrono | le pied de page affiche-t-il « OK » ou autre chose ? |

## Ce que cette recette ne teste pas

- Le glisser-déposer Tiled (section Avancé, et seulement avec un vrai fichier `.tmx`).
- Le rechargement à chaud (modifier un fichier de scène pendant que le Studio est ouvert).
- L'export HTML (bouton de téléchargement du pied de page).

Ces trois points ont leurs propres preuves automatisées ; cette recette ne couvre que
**l'entrée dans le produit**.
