---
name: engine-ui-ux-pedagogy
description: >-
  Design guidelines, visual ergonomics, and pedagogical UI/UX architecture for the KaractiRage game engine and KVL studio.
  Use when redesigning or polishing the user interface, improving visual hierarchy, adding pedagogical interactive guides,
  enhancing bitmap font rendering, optimizing color palettes, and creating stunning low-overhead HUDs.
---

# Engine UI/UX & Pedagogical Design Skill

Ce skill définit les principes directeurs pour transformer l'interface utilisateur de **KaractiRage** en un environnement à la fois **hautement pédagogique** (guidant l'utilisateur étape par étape sans jargon obscur) et **esthétiquement remarquable** (*wow factor*, cyberpunk/dark studio moderne, zéro-overhead).

---

## 1. Principes Pédagogiques Clés

### A. Clarté Contextuelle Immédiate
* **Toujours afficher l'état courant et l'action recommandée** :
  * En Mode Jeu : afficher l'objectif actuel en surbrillance (ex: *"🎯 Objectif : Collectez 50 points d'or pour déverrouiller la porte de sécurité"*).
  * En Mode Studio : afficher le rôle du studio et un guide en 3 étapes claires :
    1. *Sélectionner* : Cliquez sur une entité pour inspecter ses composants ou son script KVL.
    2. *Créer & Composer* : Appuyez sur `P` pour spawner une entité ou attachez des composants dans l'inspecteur.
    3. *Tester* : Appuyez sur `TAB` pour passer instantanément en mode jeu.
* **Décodage Visuel du Langage KVL** :
  * Remplacer les noms techniques de nœuds par des explications limpides :
    * `EVENT` $\rightarrow$ *"QUAND CET ÉVÉNEMENT ARRIVE"* (Vert / Cyan).
    * `CONDITION` $\rightarrow$ *"SI LA CONDITION SUIVANTE EST VRAIE"* (Ambre).
    * `ACTION` $\rightarrow$ *"ALORS EXÉCUTER CET EFFET"* (Violet / Émeraude).
  * Les broches de sortie conditionnelles doivent afficher explicitement : `[Vrai:Out]` (Vert) et `[Faux:Out]` (Rouge).

### B. Écran d'Aide & Raccourcis Interactif (`H`)
* Fournir une fenêtre modale d'aide complète accessible à tout moment avec la touche **H** (ou bouton d'aide dédié) résumant :
  * Les principes de l'architecture Entité-Composant (ECS).
  * La signification des couleurs et des symboles de la minicarte.
  * Les raccourcis clavier essentiels pour jouer et pour éditer.

---

## 2. Design System & Esthétique Visuelle

### A. Palette de Couleurs (Dark Tech & Cyber Neon)
* **Arrière-plans et Panneaux** :
  * Fond principal : Bleu nuit profond `0x000B0F19`.
  * Panneaux latéraux et modales : Ardoise sombre `0x00111827` avec bordures contrastées `0x001F2937` ou `0x00334155`.
  * Zone de jeu : Délimitée par un cadre technologique renforcé avec coins biseautés.
* **Accents Thématiques** :
  * **Cyan Cyber** (`0x0038BDF8` / `0x000284C7`) : Sélections, connecteurs actifs, information générale.
  * **Violet Électrique** (`0x00A855F7` / `0x007C3AED`) : Mode Studio, entités spéciales, auras.
  * **Vert Émeraude** (`0x0010B981` / `0x00059669`) : Actions positives, validation, sauvegarde, succès.
  * **Ambre Doré** (`0x00F59E0B` / `0x00FBBF24`) : Éléments interactifs, conditions KVL, score, mise en valeur.
  * **Rouge Rubis** (`0x00EF4444` / `0x00DC2626`) : Alertes ennemis, suppressions, échecs de conditions.

### B. Typographie Bitmap & Glyphes Complètes
* Garantir la présence de tous les caractères spéciaux utiles (`?`, `!`, `_`, `|`, `/`, `:`, `*`, `+`, `-`, `=`, `#`, `>`, `<`).
* Remplacer les caractères manquants par des glyphes lisibles plutôt que par des carrés de substitution.
* Hiérarchiser visuellement les tailles de texte :
  * Échelle 1 (5x7 px) : Sous-titres, coordonnées techniques, légendes de pins.
  * Échelle 2 (10x14 px) : Titres de sections, boutons d'action, libellés de nœuds.
  * Échelle 3 (15x21 px) : Logos majeurs et en-têtes d'écran.

### C. Éléments Graphiques Enrichis
* **Radar Tactique** : Cadre néon circulaire/rectangulaire, réticule central, balayage et légendes d'entités colorées.
* **Boutons & Badges** : Rendu en pilules ou rectangles avec liseré contrasté, surbrillance d'activation et état pressé/survolé.
* **Traçabilité KVL** : Console dynamique avec horodatage relatif, badge d'événement et message explicite.
