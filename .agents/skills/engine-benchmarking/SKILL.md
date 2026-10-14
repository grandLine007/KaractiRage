---
name: engine-benchmarking
description: >-
  Performance profiling, microbenchmarks with Criterion, frame latency budgets, and memory allocation tracking for KaractiRage.
  Use when conducting benchmark campaigns, measuring CPU microseconds per frame, stress testing particle systems or collision systems,
  and verifying compliance with the Low-End / 60 FPS profile.
---

# Engine Benchmarking & Performance Profiling Skill

Ce skill fournit les directives et méthodologies pour mesurer avec précision les performances de KaractiRage, identifier les goulets d'étranglement et certifier la conformité avec le cahier des charges *Low-End*.

---

## 1. Budgets de Performance (Temps de Frame)

Pour atteindre un 60 FPS constant sans saccade (*stutter*) :
* **Budget global par frame** : $16.66\text{ ms} = 16\ 666\ \mu\text{s}$.
* **Décomposition cible KaractiRage (sur machine modeste)** :
  * Traitement des entrées & logique de jeu : $\le 200\ \mu\text{s}$
  * Exécution de l'interpréteur KVL : $\le 300\ \mu\text{s}$
  * Physique & résolution AABB : $\le 400\ \mu\text{s}$
  * Rastérisation logicielle (Canvas Softbuffer) : $\le 2\ 500\ \mu\text{s}$
  * Synthèse & streaming audio : $\le 50\ \mu\text{s}$
  * **Marge de sécurité disponible** : $> 12\ 000\ \mu\text{s}$ (soit $> 70\%$ de marge CPU libre pour préserver la batterie).

---

## 2. Système de Télémétrie Interne (`Telemetry`)

Accessible en temps réel en appuyant sur `F1` en jeu :
* `current_fps` : Fréquence d'images calculée sur une fenêtre glissante.
* `avg_frame_time_us` : Moyenne mobile du temps CPU de traitement de la frame en microsecondes.
* `kvl_steps_last_frame` : Nombre total de pas d'exécution KVL franchis lors de la frame courante.
* Détection automatique d'anomalie : alerte visuelle si `avg_frame_time_us > target_budget_us`.

---

## 3. Protocole de Stress-Testing

Avant toute validation majeure :
1. **Test Particules Max** : Pousser le générateur à 500 particules simultanées pour vérifier la réactivité du profil d'adaptation.
2. **Test Charge KVL** : Déclencher 50 nœuds d'actions simultanés pour vérifier l'absence d'allocation sauvage et le respect du seuil anti-boucle.
3. **Test Clamping Physique** : Déplacer le joueur à vitesse maximale ($> 500\text{ px/s}$) contre les murs et portes fermées pour garantir l'absence totale de traversée intempestive (*tunneling*).
