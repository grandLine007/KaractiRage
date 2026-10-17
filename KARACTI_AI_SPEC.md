# KARACTIRAGE — SPÉCIFICATION UNIVERSELLE DE CRÉATION & GUIDE RAG IA
# (KaractiRage AI System Reference & Creation Engine Guide)

> **Version** : 1.0.0 (Édition Complète Déterministe)  
> **Audience** : Modèles d'IA (Claude, Gemini, DeepSeek, Ollama, OpenAI), Agents Autonomes, Développeurs & Concepteurs de Niveaux.  
> **Rôle Fondateur** : Ce document est la **vérité terrain (Ground Truth)** pour toute génération procédurale ou pilotée par LLM dans le moteur **KaractiRage**. Toute entité, tout terrain, toute mécanique et tout comportement décrit scripturalement dans ce document est projeté de manière déterministe en rendu graphique, physique, sonore et logique dans le moteur 2D.

---

## SOMMAIRE

1. [Philosophie du Moteur & Système de Coordonnées](#1-philosophie-du-moteur--système-de-coordonnées)
2. [Terrain, Décor & Zones de Péril (Level Design)](#2-terrain-décor--zones-de-péril-level-design)
3. [Graphismes & Pixel-Art 2D (12 Archétypes & Matrice ASCII)](#3-graphismes--pixel-art-2d-12-archétypes--matrice-ascii)
4. [Architecture ECS (Composants Disponibles & Paramètres)](#4-architecture-ecs-composants-disponibles--paramètres)
5. [Catalogue Officiel des Nœuds KVL (Événements, Conditions, Actions)](#5-catalogue-officiel-des-nœuds-kvl)
6. [Boucle de Gameplay, Objectifs & Conditions de Victoire/Défaite](#6-boucle-de-gameplay-objectifs--conditions-de-victoire-défaite)
7. [Schéma JSON Strict d'Échange avec le Moteur (Payloads & Exemples)](#7-schéma-json-strict-déchange-avec-le-moteur)

---

## 1. PHILOSOPHIE DU MOTEUR & SYSTÈME DE COORDONNÉES

### Principes Clés
1. **Low-End First & Déterminisme 60 FPS** :
   - Boucle de mise à jour fixe à 60 Hz ($\Delta t = 0.0166\text{s}$).
   - Aucun calcul GPU lourd requis : rasterizer logiciel pur (`Canvas`), tracé 2D optimisé sans allocation mémoire dynamique pendant la frame.
2. **Architecture Scripturale Découplée** :
   - Rien n'est codé "en dur" dans le moteur : le joueur, les plateformes, les boss, les énigmes et les quêtes sont décrits par des structures de données déclaratives JSON et des graphes nodaux KVL.
3. **Repère Cartésien du Monde** :
   - **Origine $(X=0, Y=0)$** : Position par défaut de spawn du joueur.
   - **Axe X** : Horizontal ($+X$ vers la droite, $-X$ vers la gauche).
   - **Axe Y** : Vertical ($+Y$ vers le bas pour le rendu écran standard, avec gravité orientée vers le bas $+Y$).
   - **Échelle conventionnelle** :
     - 1 tuile standard = $32 \times 32$ pixels.
     - Taille joueur type = $32 \times 32$ ou $36 \times 48$ pixels.
     - Vitesse de déplacement joueur type = $180$ à $260$ pixels/seconde.
     - Vitesse de saut typique = $-380$ à $-450$ pixels/seconde.
     - Gravité typique = $+900$ pixels/$\text{s}^2$.

---

## 2. TERRAIN, DÉCOR & ZONES DE PÉRIL (LEVEL DESIGN)

Un niveau complet généré par l'IA doit structurer l'espace de jeu à travers l'objet `terrain` composé de **Plateformes** et de **Gouffres/Périls**.

### A. Plateformes (`platforms`)
Représentent la géométrie solide sur laquelle le joueur et les ennemis peuvent marcher, sauter et glisser.
* `name` (String) : Nom descriptif (ex: `"Sol_Depart"`, `"Passerelle_Centrale"`, `"Plafond_Arche"`).
* `offset_x` / `offset_y` (Float) : Position relative par rapport au point d'ancrage du joueur.
* `width` / `height` (Float) : Largeur et épaisseur en pixels (ex: sol principal $800 \times 40$, marche haute $160 \times 20$).
* `is_solid` (Boolean) : `true` pour bloquer les entités avec collider physique AABB.
* `color_hex` (String) : Teinte stylisée (ex: `"#1E293B"` pour sol sombre, `"#334155"` pour pierre d'ardoise, `"#475569"` pour corniches).

### B. Zones de Péril & Gouffres (`hazards`)
Zones dangereuses nécessitant précision ou réflexes pour être évitées.
* `name` (String) : Nom du danger (ex: `"Gouffre_Acide"`, `"Fosse_Pointes"`).
* `offset_x` / `offset_y` (Float) : Emplacement.
* `width` / `height` (Float) : Zone de déclenchement.
* `damage` (Float) : Dégâts infligés au contact (ex: $25.0$ à $50.0$).
* `hazard_type` (Option<String>) :
  * `"spikes"` : Pointes métalliques acérées avec dents de scie visibles.
  * `"acid"` / `"poison"` : Cuve toxique bouillonnante verte/jaune.
  * `"lava"` : Fosse de lave incandescente rouge/orange.

---

## 3. GRAPHISMES & PIXEL-ART 2D (12 ARCHÉTYPES & MATRICE ASCII)

Dans KaractiRage, les entités ne sont pas limitées à de simples rectangles plats. Elles s'affichent sous forme de **matrices pixel-art 16×16** stylisées via la forme ECS `ShapeKind::PixelMatrix`.

### A. Les 12 Archétypes Procéduraux Intégrés
L'IA peut invoquer directement l'un des 12 archétypes via le champ `"sprite_archetype"` :

1. `"ninja"` : Héros agile en capuche bleu nuit `#1E3A8A`, fente oculaire cyan `#38BDF8`, écharpe et ombre portée.
2. `"knight"` : Paladin en armure d'argent `#94A3B8`, visière dorée `#F59E0B`, plastron et gantelets.
3. `"drone"` : Sentinelle technologique avec noyau oculaire rouge pulsant `#EF4444` et ailettes ambrées `#F59E0B`.
4. `"slime"` : Créature gélatineuse bondissante vert émeraude translucide `#10B981` avec noyau cyan `#06B6D4`.
5. `"skull"` : Crâne spectral d'outre-tombe blanc osseux `#F8FAFC`, orbites ténébreuses et rictus denté.
6. `"boss"` : Titan colossal cornu d'obsidienne `#450A0A` et d'énergie cramoisie `#DC2626`.
7. `"chest"` : Coffre au trésor en chêne massif cerclé de ferrures d'or `#D97706` et serrure ouvragée `#F59E0B`.
8. `"key"` : Clé ancienne de donjon en laiton étincelant `#FBBF24` avec anneau ciselé et panneton cranté.
9. `"potion"` : Fiole alchimique de verre translucide `#38BDF8` renfermant un élixir de soin rubis `#F43F5E`.
10. `"portal"` : Vortex dimensionnel circulaire violet `#8B5CF6` et cyan `#06B6D4` marquant la sortie du niveau.
11. `"coin"` : Pièce d'or gravée `#F59E0B` avec tranche biseautée et éclat de lumière `#FEF08A`.
12. `"spikes"` : Rangée de dents d'acier `#94A3B8` et de pointes mortelles sur socle noir.

### B. Matrice ASCII Personnalisée (Pixel-Art sur-mesure)
L'IA peut également concevoir un sprite 100% inédit en fournissant une matrice de 16 chaînes de 16 caractères (`sprite_ascii`) associée à une table de palettes (`sprite_palette`) :

```json
{
  "sprite_ascii": [
    "....XXXXXXXX....",
    "...X........X...",
    "..X..YY..YY..X..",
    "..X..YY..YY..X..",
    "..X..........X..",
    "..X...RRRR...X..",
    "...X........X...",
    "....XXXXXXXX...."
  ],
  "sprite_palette": {
    ".": "#00000000",
    "X": "#1E293B",
    "Y": "#38BDF8",
    "R": "#EF4444"
  }
}
```
* Note : Le caractère `.` est réservé pour la transparence (`#00000000`).

---

## 4. ARCHITECTURE ECS (COMPOSANTS DISPONIBLES & PARAMÈTRES)

Chaque entité du monde est un conteneur d'identifiant unique recevant des composants parmi les modules natifs :

| Composant | Rôle & Détection | Paramètres Clés |
| :--- | :--- | :--- |
| `Transform2D` | Position spatiale, échelle, orientation | `x`, `y`, `rotation`, `scale_x`, `scale_y` |
| `Renderable2D` | Rendu visuel 2D | `shape`: `PixelMatrix`, `Rect`, `Circle`, `Text` |
| `Collider2D` | Boîte de collision AABB | `width`, `height`, `is_solid`, `is_trigger` |
| `Physics2D` | Dynamique newtonienne | `velocity_x/y`, `friction`, `bounce`, `gravity_scale` |
| `Health2D` | Points de vie et invulnérabilité | `current_health`, `max_health`, `invulnerability_timer` |
| `NavAgent2D` | Traque de cible et pathfinding A* | `speed`, `target_x/y`, `waypoint_index`, `is_active` |
| `ItemPickup2D` | Objet collectable au contact | `item_name`, `item_id`, `count`, `is_collected` |
| `BossFight2D` | Combat épique à phases | `phase` (1, 2, 3), `nova_cooldown`, `is_enraged` |
| `VisionCone2D` | Cône de vue et détection d'intrus | `fov_angle`, `vision_range`, `alert_state` |
| `BoidFlock2D` | Comportement de groupe / meute | `flock_id`, `cohesion_weight`, `alignment_weight` |
| `Tag2D` | Classification d'entité | `"player"`, `"enemy"`, `"boss"`, `"item"`, `"door"`, `"hazard"` |

---

## 5. CATALOGUE OFFICIEL DES NŒUDS KVL

Le système logique **KVL (KaractiRage Visual Logic)** permet de définir les comportements sans écrire de code impératif. Les nœuds possèdent des broches d'entrée/sortie (`Pin`) de type **Trigger** (flux d'exécution) ou **Donnée** (`Int`, `Float`, `Bool`, `String`).

### A. Nœuds d'Événements (`NodeKind::Event`)
Points d'entrée déclenchés par le moteur de jeu :
* `"OnStart"` / `"OnLevelStart"` : Exécuté dès le chargement du niveau (initialisation de variables, affichage d'objectifs).
* `"OnPlayerOverlap"` : Déclenché lorsqu'une entité entre en collision avec le joueur.
* `"OnPlayerLeave"` : Déclenché lorsque le joueur quitte la zone de trigger.
* `"OnDamage"` : Déclenché lorsqu'une entité subit des dégâts.
* `"OnDeath"` / `"OnDestroy"` : Déclenché lorsque la santé tombe à 0.
* `"OnKey:<Touche>"` : Déclenché par une entrée clavier (ex: `"OnKey:Space"`, `"OnKey:E"`).
* `"OnTimer:<secondes>"` : Déclenchement périodique ou différé.
* `"OnItemCollect"` : Déclenché au ramassage d'un item dans l'inventaire.

### B. Nœuds de Conditions (`NodeKind::Condition`)
Nœuds logiques avec deux sorties Trigger : `out_true` et `out_false`.
* `"IsPlayerAlive"` : Vérifie si les PV du joueur sont $> 0$.
* `"HealthLessThan:<seuil>"` / `"HealthGte:<seuil>"` : Test de santé (ex: `"HealthLessThan:20"` pour déclencher une phase d'enrage).
* `"ScoreAtLeast:<seuil>"` / `"ScoreGte:<seuil>"` : Vérifie si le score atteint la valeur requise.
* `"CounterAtLeast:<compteur>:<seuil>"` : Vérifie un compteur nommé (ex: `"CounterAtLeast:cle_ramasse:3"`).
* `"HasItem:<item_id>"` : Vérifie si l'inventaire contient l'objet (ex: `"HasItem:Cle_Or"`).
* `"ItemCountGte:<item_id>:<quantite>"` : Vérifie la quantité d'un objet donné.
* `"VarEquals:<nom>:<valeur>"` / `"VarBool:<nom>"` : Test générique sur une variable de contexte.
* `"IsBossEnraged"` / `"BossPhaseEquals:<phase>"` : Interrogation de l'état du boss.
* `"IsDoorOpen"` / `"IsAlarmActive"` : Vérification d'état d'infrastructure.

### C. Nœuds d'Actions (`NodeKind::Action`)
Instructions opérationnelles modifiant le monde ou les entités :

#### Progression & Score
* `"AddScore"` : Ajoute +1 au score.
* `"BonusPoints"` : Ajoute +50 points avec feedback visuel.
* `"ResetScore"` : Réinitialise le score à 0.

#### Santé & Survie
* `"DamagePlayer"` : Inflige 1 dégât au joueur.
* `"DamagePlayer:<montant>"` : Inflige $N$ points de dégâts (ex: `"DamagePlayer:25"`).
* `"HealPlayer"` : Restaure 1 PV au joueur.
* `"HealPlayer:<montant>"` : Restaure $N$ PV sans dépasser la santé max.

#### Déplacement & Espace
* `"TeleportPlayer"` : Téléporte le joueur aux coordonnées par défaut.
* `"TeleportTo:<X>,<Y>"` : Téléporte l'entité ciblée aux coordonnées exactes (ex: `"TeleportTo:450,200"`).
* `"BoostSpeed"` : Accélère l'entité (+120 px/s).
* `"SlowSpeed"` : Ralentit l'entité (-80 px/s).
* `"ModifySpeed:<delta>"` : Ajuste la vitesse de déplacement (ex: `"ModifySpeed:50"`).

#### Portes & Accès
* `"ToggleDoor"` : Inverse l'état d'une porte (ouvre si fermée, ferme si ouverte).
* `"LockAllDoors"` : Verrouillage de sécurité immédiat.
* `"DenyAccess"` : Refuse l'accès et joue le son de verrouillage.

#### Navigation & IA
* `"NavChasePlayer"` / `"ChasePlayer"` : Active la traque A* automatique du joueur par l'agent.
* `"NavStop"` / `"StopNavigation"` : Fige la navigation de l'agent.
* `"NavGoTo:<X>,<Y>"` : Ordonne à l'agent de naviguer vers des coordonnées cibles en évitant les obstacles.
* `"TriggerAlarm"` / `"ClearAlarm"` : Active ou désactive le statut d'alerte générale.

#### Boss & Combats Épiques
* `"SetBossPhase:<1|2|3|Defeated>"` : Bascule la phase de combat du Boss.
* `"BossEnrage"` : Active l'état d'enrage (vitesse accrue, attaques nova rapprochées).
* `"SetBossDefeated"` : Marque le boss comme vaincu et déclenche la fanfare de triomphe.
* `"TriggerBossTelegraph:<attaque>"` : Affiche un halo d'avertissement visuel avant une attaque dévastatrice.
* `"TriggerBossRoar"` : Fait rugir le Boss avec onde de choc et secousse d'écran.

#### Dialogues & Quêtes
* `"ShowDialogue:<speaker>:<texte>"` : Affiche une bulle de dialogue narrative.
* `"ShowDialogueChoice:<speaker>:<texte>:<choix1>:<choix2>"` : Ouvre un dialogue interactif à embranchements.
* `"CloseDialogue"` : Ferme la fenêtre de dialogue.
* `"SetQuest:<id>:<étape>"` : Met à jour l'étape d'une quête dans le journal.
* `"AdvanceQuest:<id>"` : Incrémente l'étape de quête.
* `"CompleteQuest:<id>"` : Marque la quête comme accomplie avec succès.

#### Inventaire & Objets
* `"GiveItem:<item_id>:<quantite>"` : Dépose l'objet dans la Hotbar ou l'inventaire.
* `"RemoveItem:<item_id>:<quantite>"` : Retire l'objet requis.
* `"UseItem:<item_id>"` : Consomme l'objet sélectionné (ex: potion).
* `"SelectHotbarSlot:<0..4>"` : Sélectionne l'emplacement actif de la barre d'action.

#### Effets & Particules
* `"EmitBurst"` / `"EmitBurst:<nombre>"` : Déclenche une gerbe d'étincelles ou de confettis.
* `"ToggleEmitter"` / `"ToggleEmitter:<true|false>"` : Active ou éteint un émetteur continu.
* `"DestroySelf"` : Supprime immédiatement l'entité du monde.

#### Variables & Compteurs
* `"IncrementCounter:<nom>"` : Incrémente un compteur de passage ou de collecte.
* `"AddVar:<nom>:<delta>"` : Ajuste une variable entière.
* `"SetVar:<nom>:<valeur>"` : Fixe une variable de contexte.

---

## 6. BOUCLE DE GAMEPLAY, OBJECTIFS & CONDITIONS DE VICTOIRE/DÉFAITE

Pour qu'un jeu soit captivant et autonome, il ne suffit pas de poser des entités : il faut articuler une **boucle de gameplay complète** (`gameplay_loop`) :

1. **Objectif Narratif (`objective`)** :  
   Une consigne claire et percutante affichée en haut à gauche du HUD (ex: `"Traversez les gouffres, éliminez le Boss Squelette et rejoignez le Portail"`).
2. **Condition de Victoire (`win_condition`)** :
   * `"CollectAllKeysThenExit"` : Trouver la clé ou le trésor, puis atteindre la porte/portail.
   * `"DefeatBoss"` : Abaisser les PV du Boss à 0.
   * `"ReachGoal"` : Atteindre l'extrémité droite ou le point de téléportation sans mourir.
   * `"SolveSokoban"` : Placer tous les blocs mobiles sur les plaques de pression.
3. **Condition de Défaite (`lose_condition`)** :
   * `"PlayerHealthZero"` : Défaite si les PV du joueur tombent à 0 (chute dans un gouffre ou dégâts ennemis).
   * `"TimeLimit"` : Compte à rebours écoulé.
4. **Fanfare & Récompense (`win_fanfare`)** :
   * Message de glorification et jingle audio triomphal lors de la réussite.

---

## 7. SCHÉMA JSON STRICT D'ÉCHANGE AVEC LE MOTEUR

Lorsqu'un LLM génère un niveau pour KaractiRage, il doit répondre **EXCLUSIVEMENT** avec un objet JSON pur validant ce schéma :

```json
{
  "title": "Nom Épique du Niveau",
  "description": "Explication en français des mécaniques de jeu, du level design et des intentions artistiques",
  "primary_genre": "Platformer" | "Aventure" | "ShootEmUp" | "RPG" | "Puzzle" | "Action",
  "terrain": {
    "theme": "dungeon" | "cyber" | "forest" | "lava_cave" | "ruins",
    "platforms": [
      {
        "name": "Plateforme_Sol",
        "offset_x": -100.0,
        "offset_y": 120.0,
        "width": 600.0,
        "height": 40.0,
        "is_solid": true,
        "color_hex": "#334155"
      }
    ],
    "hazards": [
      {
        "name": "Pointes_Piege",
        "offset_x": 180.0,
        "offset_y": 140.0,
        "width": 80.0,
        "height": 40.0,
        "damage": 35.0,
        "hazard_type": "spikes"
      }
    ]
  },
  "gameplay_loop": {
    "objective": "Traversez le donjon, terrassez le boss et emparez-vous de la clé mystique",
    "win_condition": "DefeatBoss",
    "lose_condition": "PlayerHealthZero",
    "target_item_name": "Cle_Mystique",
    "win_fanfare": "Victoire Magistrale !"
  },
  "entities": [
    {
      "name": "Joueur_Ninja",
      "kind": "player",
      "offset_x": 0.0,
      "offset_y": 60.0,
      "width": 36.0,
      "height": 48.0,
      "health": 6.0,
      "color_hex": "#38BDF8",
      "sprite_archetype": "ninja",
      "movement": "none",
      "has_collider": true,
      "has_vision": false,
      "is_flocking": false
    },
    {
      "name": "Boss_Squelette",
      "kind": "boss",
      "offset_x": 380.0,
      "offset_y": 40.0,
      "width": 56.0,
      "height": 72.0,
      "health": 50.0,
      "color_hex": "#EF4444",
      "sprite_archetype": "skull",
      "movement": "patrol",
      "has_collider": true,
      "has_vision": true,
      "is_flocking": false
    },
    {
      "name": "Coffre_Tresor",
      "kind": "item",
      "offset_x": 520.0,
      "offset_y": 70.0,
      "width": 32.0,
      "height": 32.0,
      "health": 1.0,
      "color_hex": "#F59E0B",
      "sprite_archetype": "chest",
      "movement": "none",
      "has_collider": true,
      "has_vision": false,
      "is_flocking": false
    }
  ],
  "kvl_actions": [
    {
      "event": "OnDeath",
      "action": "SetBossDefeated",
      "target_entity": "Boss_Squelette"
    },
    {
      "event": "OnPlayerOverlap",
      "action": "GiveItem:Cle_Mystique:1",
      "target_entity": "Coffre_Tresor"
    }
  ]
}
```

---

## 8. RÈGLES DE VALIDATION EN BAC À SABLE (SANDBOX COMPLIANCE)

Tout patch ou niveau généré par l'IA subit une **simulation pré-vol de 100 pas** dans le bac à sable virtuel hermétique de KaractiRage avant d'être proposé à l'utilisateur :
1. **Zéro Panique Mémoire** : Aucune référence nulle, coordonnées infinies (`NaN`), ou division par zéro autorisée.
2. **Cohérence Topologique** : Le joueur doit pouvoir se poser sur au moins une plateforme solide sans tomber instantanément dans le vide infini.
3. **Résolution de Dépendances** : Toute condition de victoire (`target_item_name`) doit correspondre à une entité existante dans le monde.
4. **Validation de Graphe KVL** : Tout pin d'entrée doit recevoir un type compatible (les erreurs de typage `Trigger` $\leftrightarrow$ `Value` sont rejetées d'office).

---

> Ce document fait foi pour l'ensemble du projet **KaractiRage**. Toute extension de fonctionnalités au moteur Rust doit être immédiatement répercutée dans ce fichier pour maintenir l'alignement parfait entre l'intelligence artificielle et le runtime.
