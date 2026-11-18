# Recette 3D — 3 minutes

> **But** : vérifier qu'un auteur peut importer un modèle, le sélectionner, le déplacer,
> annuler, et régler caméra/lumières/matériau — sans documentation.

## Ce qu'il faut

- Le Studio ouvert avec un projet (le Hub ouvre sur « Créer un jeu »).
- Le fichier `Chateau_Gothique_Cour_Royale.glb` (ou n'importe quel `.glb`).

## Les gestes

| # | Geste | Ce que tu dois voir |
|---|---|---|
| 1 | Bascule en **3D** (le projet doit être 3D), puis barre du viewport → **Importer un modèle 3D** | le modèle apparaît **cadré**, pas de scène de démonstration |
| 2 | **Clique sur le modèle** | badge « Sélection #n », inspecteur à droite avec **X/Y/Z, RX/RY/RZ, SX/SY/SZ** et le bloc **Matériau 3D** |
| 3 | Change **X** (par ex. 2) et appuie sur Entrée | le modèle se déplace ; `⌘Z` le remet à sa place |
| 4 | **Caméra** → modifie Pos Z, Entrée | le point de vue change ; **Recadrer** revient au cadrage initial |
| 5 | **Lumières** → **Ajouter une lumière** | la liste affiche une lumière point ; ✕ la retire ; `⌘Z` la remet |
| 6 | Change la **couleur** du matériau | le modèle change de teinte ; `⌘Z` revient |

## Ce qui doit être vrai

| Point | Attendu |
|---|---|
| Modes 2D | Pixel et Vectoriel **absents** en projet 3D |
| Outils de tuiles | **absents** |
| Palette | un seul onglet **Mesh 3D** |
| Annulation | `⌘Z` annule transformée, matériau, lumière |
| Rétablissement | `⇧⌘Z` (ou bouton) remet la modification |

## Si ça ne marche pas

Note le **geste** (1 à 6), ce que tu vois à la place, et une capture.

| Symptôme | À me dire |
|---|---|
| Rien n'apparaît après l'import | message rouge dans la barre du viewport ? |
| Un cube rouge apparaît | les octets du GLB n'arrivent pas au renderer |
| Le clic ne sélectionne pas | l'inspecteur reste vide, ou le badge ne s'affiche pas ? |
| `⌘Z` ne fait rien | est-ce en 3D ou en 2D ? un message d'erreur ? |

## Ce que cette recette ne couvre pas

- L'export HTML (le runtime reste 2D).
- Le glisser-déposer Tiled (section Avancé du Hub).
- L'orbite à la souris (pas encore implémentée).
- Le gizmo (pas encore implémenté).
