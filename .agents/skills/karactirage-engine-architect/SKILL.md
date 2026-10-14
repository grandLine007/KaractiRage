---
name: karactirage-engine-architect
description: >-
  Advanced architecture, software rendering, memory optimization, and physics design for the KaractiRage 2D game engine.
  Use when designing or optimizing core engine systems, 2D physics (AABB/SAT), software rasterization pipelines,
  dynamic adaptation profiles (Battery/Low-End/Ultra), spatial partitioning (Spatial Hash Grid/QuadTree), and zero-allocation per frame game loops.
---

# KaractiRage Engine Architect Skill

Ce skill formalise les règles architecturales, les invariants d'optimisation et les techniques de pointe pour élever **KaractiRage** à un niveau technique et visuel exceptionnel, tout en respectant scrupuleusement la contrainte matérielle *Low-End* (tourner avec fluidité sur PC modeste, machine ancienne ou profil batterie).

---

## 1. Principes Fondamentaux de Conception

### A. Règle du Zéro-Allocation par Frame (Zero Alloc Hot Path)
* **Interdiction stricte** : Aucune allocation heap (`Vec::new()`, `String::from()`, `format!()`) dans la boucle critique `update()` et `render()`, sauf événement exceptionnel (chargement, coupure de câble).
* **Buffers réutilisables** : Pré-allouer les vecteurs de particules, de boîtes solides et de listes d'entités avec `reserve()` ou `Vec::clear()`.
* **Traçabilité microseconde** : Mesurer chaque frame CPU via le système `Telemetry` interne. Le budget frame de 60 FPS est de **16 666 µs** ; le hot path de KaractiRage doit demeurer **inférieur à 1 000 µs (1 ms)** sur machine moderne pour tourner à 60 FPS sur hardware 10x plus lent.

### B. Partitionnement Spatial (Spatial Hash Grid)
Quand le nombre d'entités (murs, pièces, portes, ennemis, projectiles) dépasse 50 :
1. Découper le monde en cellules régulières de taille $C \times C$ (ex: $64 \times 64$ pixels).
2. Clé de hachage entière : `hash = (grid_x * 73856093) ^ (grid_y * 19349663)`.
3. Réduire la complexité des tests de collision de $\mathcal{O}(N^2)$ à $\mathcal{O}(N)$ amorti.

### C. Rendu Logiciel 2D Haute Performance (`softbuffer`)
* Accès direct au buffer mémoire linéaire `&mut [u32]` en format `0x00RRGGBB`.
* Algorithme de tracé de lignes Bresenham optimisé en arithmétique entière.
* Éviter les calculs flottants `sin`/`cos` par pixel : précalculer en tables de correspondances (LUT) ou utiliser des approximations polynomiales.
* Utiliser le découpage d'écran rectangulaire (Scissor clipping) pour ne rastériser que la zone modifiée ou visible.

---

## 2. Physique 2D & Glissement Continu

* Séparation stricte des axes : toujours résoudre l'axe X (`player_x`), puis l'axe Y (`player_y`) séparément pour permettre un glissement naturel et sans à-coups le long des parois.
* Épaisseur des parois : dimensions minimales de 4 px pour éviter tout effet de *tunneling* lors de vitesses élevées.
* Interfaçage KVL : chaque entité physique possède un état de solidité dynamique (ex: `door.is_open` désactive la boîte solide).

---

## 3. Profils d'Adaptation Dynamique

* **Ultra Low-End** : Rendu minimaliste, limite de 20 particules, pas d'effets de halo, fréquence KVL réduite.
* **Low-End** (Profil cible standard) : Particules à 60 unités, grille d'édition active, physique complète.
* **Équilibré / Élevé** : Jusqu'à 200 particules, effets scintillants, télémétrie complète, audio multi-voix.
