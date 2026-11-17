# Contrat global de création 2D/3D et d’animation assistées par IA

> **Document unique de cadrage produit, technique et agentique**  
> **Version :** 0.2 — cadrage consolidé après réponses du propriétaire  
> **Date :** 17 novembre 2026  
> **Statut :** cadrage confirmé par le propriétaire le 17 novembre 2026; détails ouverts indiqués au §15  
> **Projet :** KaractiRage Studio  
> **Langue de travail :** français; identifiants, formats et schémas techniques stables en anglais

## 0. Comment lire ce contrat

Ce fichier consolide les objectifs de l’utilisateur et les éléments pertinents des six documents fournis : `ACTIONS_HUMAINES.md`, `CHANTIER_ORCHESTRATION.md`, `CONTRAT_EDITION_IA.md`, `DECISIONS_OUVERTES_UX.md`, `SCENARIO_DEMARRAGE_ET_USAGE.md` et `STRATEGIE_CREATION_2D_3D_IA.md`.

Les six documents sont des **sources de contexte**. Leurs consignes opérationnelles, exemples de commandes, mesures de versions, recommandations ou états de chantier ne deviennent pas automatiquement des exigences de ce contrat. En particulier, les actions humaines de publication/sécurité, les vagues de chantier terminées et les dates d’état du dépôt sont historiques. L’agent qui implémente doit vérifier l’état réel du dépôt avant de planifier du travail.

Les réponses du propriétaire priment sur les documents de contexte. Les choix confirmés figurent au §15. Les valeurs **[À DÉCIDER]** restantes sont des détails non bloquants ou des engagements qui nécessitent une décision explicite avant toute dépense, publication, collecte de données ou incompatibilité majeure.

### 0.1 Intention de l’utilisateur

Construire un logiciel de création de jeux vidéo et d’animations 2D/3D qui soit facile d’accès, où un utilisateur ou des agents puissent avancer de l’idée au jeu ou à l’animation livrable. Le logiciel peut utiliser des modèles locaux peu puissants (notamment LM Studio et Ollama) ou des fournisseurs en ligne (notamment ChatGPT/OpenAI, Claude, DeepSeek et Kimi), sans rendre un fournisseur, une connexion Internet ou un modèle puissant obligatoire. Le produit doit s’appuyer sur des formats ouverts, des projets libres et des catalogues/API d’assets lorsque leurs conditions permettent l’intégration et la distribution.

Le système doit permettre l’import, la préparation, l’assemblage, l’édition, le test et l’export des scènes, logiques, assets, sons, caméras, rigs et animations dans une portée définie ci-dessous. Des MCP peuvent relier les outils d’authoring externes; ils ne doivent pas être une dépendance des jeux exportés.

### 0.2 Cadrage confirmé et valeurs de départ

| Sujet | Valeur retenue | Origine |
|---|---|---|
| Cible produit | jeu 2D complet puis 2.5D/3D déclarative, avec accès à des catalogues et projets ouverts | confirmé par le propriétaire |
| Modélisation/rigging avancés | retouches mineures des modèles dans le Studio; outils libres externes intégrables pour rigging/animation avancés | confirmé par le propriétaire |
| Plateformes | v1 : desktop et HTML5; v2 : mobile et certaines consoles | confirmé par le propriétaire |
| IA | fonctionne sans IA; API compatible OpenAI et adaptateurs; modèle local ou cloud facultatif | confirmé par le propriétaire |
| Édition IA | propositions typées, validation, aperçu, transaction réversible | contrat IA fourni |
| MCP | capacités d’authoring optionnelles, confinées et désactivables | stratégie fournie |
| Licences d’assets | licence détectée et provenance attachée; inconnue = pas de téléchargement automatique | contrat IA fourni |
| Autonomie des agents | garde-fous configurables par l’utilisateur final, avec risques présentés avant activation | confirmé par le propriétaire |
| Licence du produit | licence permissive MIT ou Apache-2.0; choix final et compatibilité des dépendances à établir | confirmé par le propriétaire |
| Génération d’assets | SVG/vectoriels et chiptune 8/16-bit; imports et services externes facultatifs | confirmé par le propriétaire |
| Public cible | débutants/créateurs sans code, français d’abord | confirmé par le propriétaire |
| Projet et secrets | fichiers locaux versionnables; secrets dans le trousseau OS | confirmé par le propriétaire |
| Génération | SVG/vectoriels et sons chiptune 8/16-bit; import et plateformes de génération externes facultatifs | confirmé par le propriétaire |

## 1. Mission et définition du succès

### 1.1 Promesse

Un créateur formule un objectif en langage naturel ou choisit un modèle de départ. KaractiRage crée un projet structuré, permet de le modifier visuellement ou par agents, rend chaque modification observable et annulable, puis produit un résultat jouable/visionnable et exportable.

Un agent doit pouvoir réaliser une demande complète à partir d’un brief, du dépôt et des outils déjà autorisés, sans poser de question de détail à chaque étape. Il doit déduire les choix réversibles à partir de profils et de valeurs par défaut, consigner ses hypothèses, tester son résultat et fournir un rapport reproductible. Il s’arrête seulement lorsqu’une information indispensable est absente, qu’un risque/droit requiert l’utilisateur, ou qu’une capacité externe est indisponible.

### 1.2 Résultats observables

L’utilisateur doit pouvoir :

1. créer un projet jouable/animable depuis un archétype ou une description;
2. éditer scènes, entités, logique, interface, caméra, sons et animations depuis le Studio;
3. importer ses fichiers ou rechercher des assets distribuables avec licence affichée;
4. utiliser zéro IA, un petit modèle local, ou un fournisseur distant configuré;
5. demander à un agent de réaliser une tâche multi-étapes, suivre son avancement et annuler ses changements;
6. tester dans le Studio et exporter un jeu ou une animation avec les ressources requises embarquées ou copiées;
7. rouvrir, modifier et réexporter le projet sans perte de provenance ou de contenu.

### 1.3 Profil de qualité

Les chiffres de démarrage/latence/performance historiques du scénario fourni sont des objectifs à vérifier sur les plateformes réellement retenues. Ils ne constituent pas des résultats garantis. Toute cible numérique du produit final doit préciser matériel, mode de build, scénario, statistique (p50/p95) et procédure de mesure.

## 2. Périmètre produit

### 2.1 V1 — cadrage confirmé

- Création et export d’un jeu 2D solo complet : scènes/niveaux, entités, collisions, logique visuelle, interface, audio, sauvegarde de jeu et menus.
- Édition et génération assistée d’assets 2D, notamment SVG/vectoriels et sons chiptune 8/16-bit; import d’images, sons et cartes; recherche et téléchargement d’assets libres sous licence vérifiable.
- Capacité 2.5D/3D déclarative : scène, transformations, caméra, lumières, matériaux simples, modèles glTF/GLB et lecture de clips compatibles.
- Téléchargement et intégration de modèles listés depuis des plateformes ouvertes; modifications mineures et réversibles des modèles importés depuis le Studio.
- Animation/rigging avancés par outils libres existants (notamment Blender lorsque retenu), intégrés via formats ouverts et/ou connecteurs isolés; le Studio orchestre ces capacités sans devenir un DCC complet.
- Export desktop et HTML5 en v1; mobile et certaines consoles relèvent de la v2 et nécessitent une étude de faisabilité par plateforme.
- Import/export de projets et manifeste d’assets versionnés.
- Assistance IA locale ou distante facultative avec adaptateurs; outils MCP optionnels d’authoring.

### 2.2 Limites de la V1

Le Studio n’a pas à reconstruire Blender. Il permet des retouches mineures de modèles importés (position, rotation, échelle, nommage, placement, matériaux simples et paramètres explicitement supportés). Modélisation polygonale avancée, sculpture, retopologie, UV complexes, rigging sophistiqué, retargeting professionnel, simulation de fluides, rendu photoréaliste et shaders arbitraires passent par des outils libres existants intégrés au flux, puis sont importés. Chaîne exigée : génération ou outil externe → conversion/validation → manifeste → preview → scène → export.

La priorité d’animation est l’animation sprite et la lecture de clips glTF/GLB. Le rigging 2D/3D et les éditions avancées passent d’abord par des outils libres externes intégrables. Des fonctions simples de timeline, keyframes ou réglage de clips dans le Studio peuvent être ajoutées si elles restent adaptées aux machines modestes et réutilisent des formats ouverts. Le format de projet doit permettre leur évolution sans migration destructrice.

### 2.3 Hors périmètre par défaut

- Multijoueur en ligne, services hébergés obligatoires ou comptes obligatoires.
- Par défaut : aucun achat/abonnement, téléchargement en arrière-plan ou envoi de projet à distance; l’utilisateur peut autoriser précisément certaines de ces actions dans un profil de permissions après affichage des risques.
- Éditeur 3D propriétaire complet qui remplace les outils spécialisés; seules les retouches mineures sont promises dans le Studio.
- IA/MCP requis à l’exécution des jeux exportés.
- Dépendance à un format propriétaire sans voie d’import/export documentée.

**V2 ciblée :** étendre les exports vers mobile et certaines consoles. La liste exacte de consoles et les contraintes de SDK, comptes développeur, certification et frais restent à évaluer avant toute promesse de plateforme.

Un changement de périmètre nécessite une décision explicite enregistrée dans ce fichier.

## 3. Parcours et expériences exigés

### 3.1 Démarrer sans expertise technique

- Au premier lancement, présenter des choix clairs : créer, ouvrir un projet, lancer une démo ou suivre un parcours guidé.
- Permettre d’obtenir une scène jouable sans lire une documentation technique.
- Garder visibles l’état du moteur, le chargement d’assets, les erreurs et la possibilité de réessayer.
- Fournir des modèles éditables; signaler clairement le contenu provisoire généré.

### 3.2 Éditer

- Vue scène/hiérarchie, sélection, inspecteur, outils de placement, duplication, suppression, grilles et caméra.
- Éditeur de logique visuel dont les nœuds sont issus d’un catalogue typé et versionné.
- Éditeurs de niveaux et d’interface; import de cartes Tiled dans la portée supportée.
- Contrôles clavier/souris cohérents, undo/redo et sauvegarde; les raccourcis doivent être documentés et configurables.
- Preview à la demande et retour clair après chaque action d’agent.

### 3.3 Animation

- 2D : frames, spritesheets/atlas, clips, durée/bouclage, aperçu et export.
- 3D : importer et jouer les animations/squelettes inclus dans les fichiers glTF/GLB conformes; les workflows de création avancée passent par DCC externe.
- La scène doit permettre d’associer un clip à une entité et de déclencher lecture, arrêt, boucle et transitions selon les capacités du runtime.
- Le rigging et l’animation avancés sont accessibles via des outils libres existants intégrés au workflow; une timeline ou des fonctions natives plus poussées restent à prioriser selon les coûts et la demande.

### 3.4 Export

- Vérification préalable bloquante sur références manquantes, licences/provenances requises, formats non supportés, ressources hors budget et scripts/graphes invalides.
- Rapport d’export compréhensible, avec la liste des problèmes et actions correctives.
- Export autonome ou paquet contenant les dépendances; indiquer clairement les cas qui requièrent réseau.
- Le build exporté n’embarque ni connecteur LLM ni serveur MCP, sauf demande distincte et documentée.
- À contenu et versions identiques, produire une sortie reproductible lorsque la cible le permet; publier les exclusions au hachage (horodatage, métadonnées de build, etc.).

## 4. Architecture de référence

Les documents décrivent un cœur moteur Rust, un Studio desktop Tauri v2/React et un renderer 3D utilisant three.js. On conserve ces briques tant que l’audit du dépôt montre qu’elles restent adaptées; aucun agent ne remplace l’architecture au cours d’une tâche ciblée sans ADR (Architecture Decision Record) avec coûts, bénéfices, migration et retour arrière.

### 4.1 Couches

1. **Format de projet** : données versionnées, scènes/niveaux, logique, settings, manifeste et historique de migration.
2. **Moteur runtime** : simulation, rendu, entrées, audio, chargement des ressources et mécanismes déterministes requis.
3. **Studio** : création visuelle, inspecteurs, gestion d’assets, preview, validation et exports.
4. **AI Gateway** : fournisseurs LLM, modèles, schémas de sortie, limites, confidentialité et coûts.
5. **Tool Broker** : adaptateurs de commandes MCP/outils locaux, permissions, isolation, délais et quotas.
6. **Adaptateurs de catalogues** : recherche, fiches de licence, téléchargement autorisé, empreinte et attribution.
7. **Pipelines d’import/export** : validation, conversion, budgets, packaging et rapports.

Les couches 4 à 6 ne sont pas nécessaires au runtime de jeu. Une absence, panne ou désinstallation de ces capacités n’empêche pas d’ouvrir et jouer un projet déjà complet.

### 4.2 Contrat de données

- IDs stables, lisibles et sans collision pour entités, scènes, assets, clips, matériaux, caméras et nœuds de logique.
- Schémas versionnés; migrations explicites, testables et sans suppression silencieuse de champs.
- Fichiers projet diffables/versionnables lorsque possible; éviter un blob monolithique opaque.
- Manifeste d’assets avec ID, chemin relatif, type, format, taille, hash, origine, auteur, URL d’origine, licence et version, attribution exigée, outil/modèle de création lorsque connu, paramètres/seed lorsque réutilisables, validations et budget.
- Chemins confinés au projet et normalisés; refuser traversal, liens de sortie du projet et archives piégées.
- Chaque import conserve le fichier source ou une trace permettant de le reconstruire, selon la taille et le choix utilisateur.

## 5. Contrat d’édition IA et d’autonomie des agents

### 5.1 Modes

| Mode | Usage | Règle |
|---|---|---|
| N0 — règles/templates | gestes connus | local, déterministe, sans appel LLM |
| N1 — micro-édition | petite modification ciblée | contexte minimal, proposition structurée, opérations limitées |
| N2 — plan | création/modification multi-opérations | plan découpé en étapes vérifiables et transactions réversibles |
| N3 — agent outillé | tâche longue avec fichiers, Blender ou MCP | graphe d’étapes, journal, périmètres, permissions et reprise |

L’absence de fournisseur IA ne doit pas rendre l’édition manuelle impossible.

### 5.2 Cycle transactionnel obligatoire

`Intention → clarification interne par défauts/profil → plan → propositions typées → bac à sable → validation → aperçu → commit atomique → historique/undo → résumé et preuves`.

- L’IA ne modifie pas directement l’état en mémoire ni des fichiers hors périmètre.
- Le catalogue d’opérations est fermé, typé et versionné; opérations inconnues refusées.
- Toute action porte sa cible stable et ses préconditions. Rejeter une cible ambiguë ou inexistante sans « deviner ».
- Valider au minimum le schéma, les références, les invariants du projet, le rechargement, les assets, puis la simulation/rendu selon les zones affectées.
- Toute opération échouée restaure l’état exact; tout commit est annulable et inscrit à l’historique.
- Limiter les tentatives LLM (valeur de départ : 3), détecter les réponses répétées et les no-op. Le modèle ne reçoit qu’un diagnostic borné et exploitable.
- Séparer le résumé utilisateur des retours machine compacts destinés au modèle.
- Toute étape qui prend du temps expose son état, ses sorties, sa possibilité d’annulation et sa cause d’échec.

### 5.3 Autonomie en pratique

**Autorisées selon le profil de permissions choisi par l’utilisateur final**, une fois les capacités installées/configurées : lire le projet, rechercher dans les catalogues autorisés, télécharger/importer un asset dans le projet, modifier les fichiers du périmètre, lancer les outils locaux autorisés, générer des previews, corriger les erreurs, lancer les validations prévues et créer un résultat local réversible. Avant activation d’un niveau de permission, l’interface expose les risques et conséquences, précise les accès accordés (fichiers, réseau, outils, comptes, publication) et offre un moyen de révocation.

**Exigent une permission explicite par défaut**, sauf si l’utilisateur final a activé le niveau adéquat après présentation des risques : paiement/abonnement, action sur compte tiers, publication ou push distant, partage de données personnelles/projet à un fournisseur, installation d’un exécutable ou MCP, suppression irréversible, modification de secrets, licence commerciale non établie, ou action non annulable hors dépôt. Les permissions sont nominatives, limitées à la capacité utile et au projet lorsque possible, journalisées et révocables. Un agent ne peut jamais élever ses propres permissions.

Si une ambiguïté est réversible, l’agent choisit une valeur de profil et la signale. S’il manque une contrainte qui peut invalider droits, cible de distribution, dépense ou format final, il suspend uniquement l’étape dépendante et formule une seule question précise.

### 5.4 Règles de coordination multi-agent

- Une mission est découpée en livrables et critères d’acceptation observables.
- Un agent = un périmètre de fichiers explicite dans une même vague; éviter les collisions.
- Le coordinateur garde l’intégration des fichiers partagés, les décisions d’architecture, la compilation globale, les migrations et le rapport final.
- Chaque agent rapporte les chemins touchés, commandes réellement lancées, sorties pertinentes, échecs connus et hypothèses.
- Ne jamais déclarer « terminé » sans preuve correspondant au critère. Si une commande n’a pas été exécutée, le dire.
- Les changements de portée, l’ajout d’une dépendance, les formats publics et les migrations nécessitent une justification et une mise à jour de la documentation pertinente.

## 6. Fournisseurs de modèles et confidentialité

### 6.1 Couche fournisseur

- Définir une interface de fournisseur et des adaptateurs, plutôt qu’un couplage à un seul modèle.
- Priorité à une API compatible OpenAI pour les backends compatibles, puis adaptateurs spécifiques lorsque les différences empêchent les fonctions nécessaires.
- Évaluer Ollama, LM Studio et LocalAI pour local; OpenAI, Anthropic/Claude, DeepSeek, Kimi et autres seulement selon une implémentation et des conditions actuelles vérifiées.
- Détecter les capacités réelles du modèle : JSON/schema contraint, longueur de contexte, vision, appels d’outils, streaming. Ne pas supposer la compatibilité à partir du nom du fournisseur.
- Mode « sans modèle », choix du modèle par tâche, test de connexion, endpoint personnalisable, délai maximal, annulation et message d’erreur intelligible.
- Stocker les secrets dans le trousseau du système si disponible; ne pas les commiter, envoyer dans les logs ou inclure dans les exports.

### 6.2 Contrôle des données

- Le choix local/distant est visible par projet et par requête.
- Avant un appel distant, indiquer le contenu transmis; ne joindre que les données nécessaires et masquer chemins privés/secrets.
- Pas d’envoi de projet/asset par défaut. L’envoi du prompt au fournisseur distant est une action réseau explicite de l’utilisateur ou couverte par un profil qu’il a établi.
- Documenter rétention, télémétrie, diagnostics, désactivation et conséquences par fournisseur; ne pas prétendre qu’un appel cloud est privé au-delà de ses conditions.
- N’autoriser aucune dépendance réseau dans le runtime exporté, sauf ressource Web expressément choisie par le créateur et révélée par le pré-vol.

### 6.4 Génération et préparation d’assets

- Génération locale légère : priorité aux assets vectoriels SVG et aux sons chiptune 8-bit/16-bit, éditables et reproductibles (paramètres et seed conservés quand possible).
- Import manuel : images, sprites, sons et modèles déjà détenus par le créateur, via les formats pris en charge.
- Génération par plateforme tierce : connecteurs optionnels sélectionnés par l’utilisateur. Afficher modèle/service utilisé, coûts potentiels, données transmises, licence/conditions de sortie et provenance. Aucune génération distante ou facturée sans permission configurée.
- Génération 3D : capacité facultative à étudier; priorité initiale à la recherche/téléchargement de modèles existants libres, puis à l’import et aux retouches mineures.
- Tous les résultats générés suivent le même pipeline de validation, manifeste, droits, budget, aperçu et undo que les imports.

### 6.3 Modèles locaux modestes

Réduire le problème avant d’augmenter la taille du modèle : contexte de scène restreint, cible et voisins pertinents, catalogue d’opérations fermé, noms explicites, contraintes JSON, calculs réalisés par le moteur et plan en petites étapes. Le Studio adapte ses options au matériel détecté : privilégier règles/templates, petits modèles quantifiés et traitements peu gourmands sur PC modeste sans GPU dédié; proposer les traitements lourds seulement si la machine les supporte ou via un service externe choisi. Chaque capacité coûteuse peut être désactivée. Les modèles trop petits peuvent classer une intention et laisser un template exécuter.

Le plan de benchmark de départ des documents (40 consignes × 10 cibles, précision des cibles/opérations, no-op silencieux, latence locale) devient une proposition de harnais; le jeu de tests, seuils et modèles testés doivent être publiés avec version, machine, quantification et paramètres.

## 7. MCP, outils et plugins

### 7.1 Contrat d’un outil

Chaque pack d’outil déclare : identifiant/version, dépôt/source, licence, commandes et schémas, permissions requises, logiciels/données/GPU nécessaires, répertoires accessibles, fichiers créés, taille maximale, durée maximale, accès réseau, sortie/erreurs, santé, méthode d’arrêt et niveau de confiance.

### 7.2 Sécurité et comportement

- MCP facultatif, désactivable et séparé du runtime.
- Aucun appel direct du LLM à un outil arbitraire : plan validé → broker → tool pack autorisé.
- Processus et accès aux fichiers isolés autant que possible; liste d’hôtes réseau permise; délais, quota, taille et annulation.
- Le contenu d’un fichier, d’une page Web ou d’un serveur d’assets est une donnée non fiable, jamais une instruction système.
- Pas d’exécution shell/code libre par défaut. Les exceptions (ex. scripts Blender/Aseprite) sont désactivées ou explicitement autorisées et isolées, avec aperçu du script, journal et annulation quand possible.
- Vérifier la maintenance, les dépendances, la licence, les comportements réseau/télémétrie et les vulnérabilités avant recommandation/intégration. Un dépôt populaire n’est pas une preuve de sûreté.
- Panne d’un outil : montrer la capacité manquante et continuer sans celle-ci quand la tâche reste possible.

### 7.3 Packs envisagés

Blender/glTF, édition de sprites/pixel art, conversion/optimisation d’images, génération d’audio, banques/catalogues d’assets, Tiled, Git et diagnostics de build. Pour les fonctions avancées 3D/animation, intégrer des logiciels et projets libres déjà disponibles plutôt que réécrire leurs capacités. Aucun outil n’est obligatoire sur toutes les installations; les workflows proposés s’adaptent au matériel et aux capacités installées.

## 8. Catalogue d’assets, formats et provenance

### 8.1 Contrat de catalogue

Chaque adaptateur fournit recherche, pagination, fiche d’item, miniature, licence exacte et URI de licence, auteur, identifiant stable, URL de la page, formats, taille prévue, conditions d’attribution, accès/téléchargement et statut réseau. Un item incomplet peut être affiché comme « informations insuffisantes », mais ne peut être téléchargé automatiquement ou présenté comme compatible commercialement.

- Aucun scrape contraire aux conditions du service; utiliser une API documentée ou des exports/dépôts explicitement ouverts.
- Demander confirmation avant téléchargement volumineux, achat, authentification ou service aux conditions non standard.
- Mettre en cache la licence et la provenance au moment de l’import; faire apparaître attribution et obligations dans le projet et le paquet exporté.
- L’utilisateur choisit si les assets sont copiés/embarqués. Un catalogue indisponible ne bloque pas l’utilisation des assets locaux.
- Prévoir recherche locale de la bibliothèque déjà installée, sans Internet.

### 8.2 Sources candidates, pas des engagements contractuels

| Source/projet | Usage candidat | Condition à intégrer |
|---|---|---|
| Poly Haven API | modèles 3D, textures, HDRI | leurs assets sont annoncés CC0; l’usage de l’API live demande de créditer Poly Haven et de fournir un User-Agent propre. Toujours vérifier les conditions courantes. |
| Sketchfab Download API | modèles 3D téléchargeables | nécessite compte/authentification; API et licences par modèle; afficher licence CC, créateur et lien d’origine. Pas un catalogue universellement libre. |
| Kenney | packs de jeux 2D/3D | candidat utile; l’accès aux conditions et interfaces doit être vérifié avant intégration. |
| Tiled + `rs-tiled` | édition/import de cartes | vérifier couverture TMX, dépendances, licence et état du crate au moment de l’intégration. Le chantier fourni rapporte que l’import Tiled a déjà avancé dans ce dépôt. |
| Blender | préparation, rigging, animation, conversion glTF | intégration d’abord par import/export et commandes pilotées; MCP communautaire soumis à audit de sécurité. |
| Aseprite / alternatives libres | sprites, frames et animation 2D | formats/API/licences propres à l’outil; ne pas supposer Aseprite libre. |
| Khronos glTF | échange d’assets 3D, matériaux, skins et animations | format d’interopérabilité recommandé; valider versions/extensions réellement supportées. |
| Dépôts GitHub d’assets | code, outils, packs publiés | licence du code ≠ licence des assets du dépôt; vérifier chaque ressource et fichiers tiers. |

### 8.3 Registre initial des formats

**Priorité proposée :** PNG, SVG, WAV, atlas/spritesheets, Tiled TMX/TSX, glTF/GLB.  
**À évaluer :** Aseprite `.ase/.aseprite`, GIF/WebP (selon usages), OGG/MP3/FLAC, FBX (souvent par conversion via Blender), USD/USDZ, formats d’animation 2D/rig 2D, formats de polices. Les exports doivent publier les limites connues (compression, flips/rotations, extensions, contraintes de texture, pistes d’animation).

Ne jamais promettre « support complet » sans matrice de fonctionnalités, fichiers d’essai redistribuables et cas d’erreur couverts.

### 8.4 Validation d’assets

Contrôler taille compressée et décompressée, dimensions/résolution, nombre et format de textures, complexité du mesh, animation/skins, chemins, métadonnées, intégrité, licence et budget cible. Prévoir profils low-end et qualité. Toute réduction ou conversion est non destructive par défaut : conserver l’original ou permettre de le restaurer.

## 9. Capacités du moteur et éditeurs

### 9.1 2D

Rendu de sprites et formes, transformations, couches/ordre, tilemaps, collisions et physique selon le moteur, entrée clavier/souris/manette selon priorité, audio, scènes, UI, variables, états, transitions, sauvegarde et export.

### 9.2 2.5D/3D

Scène 3D adressable, transform, mesh glTF, matériaux simples, éclairage, caméra, culling et budget mesuré. Le 2.5D peut conserver gameplay/collisions 2D avec rendu 3D. La physique 3D, les shaders sur mesure et les fonctions avancées ne sont ajoutés qu’après un cas d’usage démontré.

### 9.3 Caméra

Caméra éditable dans la scène; paramètres typés; aperçu; caméra active déterministe; commandes de cadrage/orbite pour l’édition séparées de la caméra du jeu. Les transitions/cinématiques font l’objet d’une capacité explicite.

### 9.4 Audio

Importer sons et musique, écouter dans le Studio, déclencher par événement ou logique, régler volume/loop/bus, conserver les fichiers sources et inclure seulement les ressources utilisées à l’export. Les codecs retenus doivent tenir compte de licence, taille, plateforme, latence et prise en charge navigateur.

## 10. Performance, fiabilité, accessibilité et portabilité

- Profils cibles documentés : **Modeste** (PC sans GPU dédié, base de conception), **Standard** et **Élevé**; les fonctions et niveaux de qualité s’adaptent au matériel détecté et restent configurables.
- Ne pas imposer un GPU dédié ou un LLM local doté d’une VRAM élevée pour ouvrir/éditer un projet standard.
- Limiter taille des builds, textures, géométrie, mémoire, latence de démarrage et fréquence des appels modèle par des budgets configurables et des diagnostics.
- Simulation déterministe là où elle est promise (pas fixe, hash/replay documenté); ne pas qualifier le FPS rendu de déterministe.
- Erreurs localisées, journaux sans secret, récupération après arrêt/crash, autosave et vérification d’intégrité.
- Navigation clavier, focus cohérent, textes lisibles, contraste, libellés accessibles et interface localisable.
- Sauvegarde durable du projet; migration de format annoncée et sauvegarde préalable.

Les seuils d’acceptation de performance devront être complétés par l’utilisateur ou choisis dans un profil matériel publié. Ne pas extrapoler une mesure Mac vers Windows ou Web.

## 11. Tests et critères d’acceptation

Chaque livrable doit avoir des critères automatiques et, lorsque l’expérience utilisateur compte, une recette manuelle courte.

### 11.1 Critères minimums par modification

- compilation/build du périmètre touché;
- tests unitaires du schéma, logique ou import modifié;
- test de sérialisation aller-retour lorsque le format projet/export change;
- absence d’asset ou de chemin sortant du projet dans l’export;
- pré-vol refusant les problèmes critiques et présentant des corrections;
- projet de démonstration ouvert, modifié, sauvegardé, rouvert puis exporté;
- jeu exporté lancé hors du Studio avec les interactions et assets réellement utilisés;
- undo/rollback restaure exactement l’état avant transaction IA;
- aucun appel réseau non annoncé dans un scénario hors ligne.

Ne pas annoncer qu’un critère a été testé s’il n’a pas été exécuté. Le propriétaire peut demander de ne pas exécuter certains tests; dans ce cas, inscrire la validation comme non faite.

### 11.2 Scénarios de bout en bout de référence

1. Créer un mini-jeu 2D depuis un modèle; modifier règle et apparence manuellement puis par IA.
2. Importer une carte Tiled, un sprite, un son; inspecter provenance/licences; jouer puis exporter.
3. Importer un GLB animé, créer une scène/caméra, l’apercevoir et la publier dans une cible supportée.
4. Exécuter le même brief avec un template local, un modèle local configuré, puis un fournisseur distant fictif/test; les fournisseurs ne changent pas le contrat d’édition.
5. Simuler indisponibilité du réseau, du modèle et d’un MCP : interface explicite, projet toujours ouvrable.
6. Fournir un fichier malformé, archive trop grosse, chemin traversal, référence absente ou licence manquante : refus explicite, aucune modification partielle.
7. Annuler un agent en plein travail; le projet revient à son dernier commit cohérent.

## 12. Licence, dépendances et responsabilité des contenus

- Le propriétaire demande une licence logicielle permissive de type MIT ou Apache-2.0; le choix entre les deux et la stratégie de contributions restent à inscrire avant publication.
- Maintenir un inventaire des licences des bibliothèques, plugins, outils embarqués, polices, modèles, sons, images et exemples.
- Distinguer licence du logiciel, licence des données/assets et conditions d’accès API.
- Ne pas intégrer une dépendance copyleft ou un asset sans droits compatibles avec la redistribution visée avant d’établir sa compatibilité.
- Les projets démo doivent être redistribuables et inclure notices/attributions nécessaires.
- Fournir une fiche d’attribution exportable et ne pas laisser entendre qu’un asset est libre au seul motif que son accès ou API est gratuit.

## 13. Feuille de route avec portes de sortie

L’ordre réel est établi après audit du dépôt et inventaire des fonctions déjà livrées. Les étapes sont des capacités et des critères, pas des estimations de durée.

| Étape | Résultat exigé avant d’ouvrir l’étape suivante |
|---|---|
| A. Contrat et audit | décisions propriétaire prises ou valeurs provisoires acceptées; état courant du dépôt, dette et architecture vérifiés |
| B. Boucle 2D complète | créer → éditer → logique → audio/UI → sauvegarder → exporter un jeu complet et jouable |
| C. Manifeste et import | provenance, licences, budget, validation, formats prioritaires et erreur récupérable |
| D. IA interopérable | mode sans IA, backend local, adaptation LLM distante, transactions et benchmark reproductible |
| E. Catalogues et packs | recherche/API facultative, licences affichées, outils confinés, attribution, flux hors ligne |
| F. Animation/3D | scènes 3D et caméras éditables; catalogue de modèles licenciés; retouches mineures; import GLB animé et intégration d’un outil libre avancé démontrée |
| G. Agents de bout en bout | brief complet réalisé sans relance sur un jeu témoin, rapport/proofs, reprise/annulation et sécurité validés |
| H. Distribution | packages signés/identifiés selon les plateformes, install/upgrade documentés, pré-vol et licence complète |

Chaque étape peut réordonner les sous-lots si un résultat livré rend une capacité déjà disponible. Ne pas rouvrir une brique stable sans défaut ou besoin justifié.

## 14. Format de rapport d’un agent

Tout agent qui modifie le produit clôt sa tâche par :

```text
Objectif :
Résultat visible :
Fichiers touchés :
Hypothèses/décisions :
Commandes réellement exécutées et résultat :
Critères validés / non validés :
Limites et risques connus :
Comment reproduire/ouvrir :
Rollback ou procédure de retour arrière :
```

Un agent doit laisser un état ouvrable et expliquer les capacités externes manquantes. Il ne peut remplacer les valeurs de ce contrat par des conventions personnelles non documentées.

## 15. Registre des décisions

Les décisions principales ci-dessous ont été confirmées par le propriétaire le 17 novembre 2026. Les questions résiduelles sont marquées « à préciser »; elles ne bloquent pas les travaux réversibles et l’agent peut proposer des valeurs avec justification.

| ID | Décision | Valeur retenue | État |
|---|---|---|---|
| D1 | Séquençage | jeu solo 2D complet d’abord, puis 3D | Confirmé |
| D2 | Portée 3D | 2.5D/3D déclarative, retouches mineures des modèles importés; outils libres externes pour animation/rigging avancés | Confirmé |
| D3 | Plateformes | v1 desktop + HTML5; v2 mobile et certaines consoles | Confirmé; plateformes desktop précises et consoles à étudier |
| D4 | Matériel | PC modestes sans GPU dédié comme base; adapter capacités et qualité au matériel | Confirmé |
| D5 | Type de projet | jeux solo 2D puis 3D; aucun besoin multijoueur déclaré pour la v1 | Confirmé |
| D6 | Public/langue | débutants et créateurs sans code; français d’abord | Confirmé |
| D7 | Formats prioritaires | PNG/SVG/WAV, Tiled, glTF/GLB | Confirmé comme base |
| D8 | Fournisseurs IA | API compatible OpenAI + adaptateurs | Confirmé; ordre exact des fournisseurs à établir selon compatibilité |
| D9 | Vie privée/réseau | mode hors ligne possible; envoi réseau explicite et contrôlé | Confirmé |
| D10 | Génération | générer facilement SVG/vectoriels et sons chiptune 8/16-bit; importer images/sons; plateformes externes facultatives pour image, son et modèle 3D | Confirmé; génération 3D à traiter comme option |
| D11 | Catalogues/licences | sources libres et licence vérifiée | Confirmé; catalogues exacts à qualifier |
| D12 | MCP | outils d’authoring optionnels et isolés | Confirmé |
| D13 | Autonomie | garde-fous configurables par l’utilisateur final; afficher risques et conséquences des permissions | Confirmé |
| D14 | Données | projets locaux versionnables; secrets dans le trousseau du système | Confirmé |
| D15 | Modèle économique/licence | gratuit et open source; services payants facultatifs; licence permissive MIT/Apache | Confirmé; choisir MIT ou Apache et vérifier les dépendances |
| D16 | Preuve de v1 | jeu 2D complet puis capacité 3D; seuils chiffrés à établir | Confirmé |

## 16. Références vérifiées pour les candidats externes

État relevé le 17 novembre 2026. La disponibilité et les conditions des services tiers peuvent évoluer; les intégrations doivent vérifier ces références de nouveau avant chaque livraison.

- [Poly Haven — API](https://polyhaven.com/our-api) et [licence des assets](https://polyhaven.com/license) : l’API publique est documentée; les assets sont annoncés CC0, avec condition de crédit d’usage de l’API et User-Agent.
- [Sketchfab — Download API](https://sketchfab.com/developers/download-api), [authentification/téléchargement](https://sketchfab.com/developers/download-api/downloading-models) et [conditions API](https://sketchfab.com/developers/terms) : accès téléchargements authentifié et obligations d’afficher licence et attribution.
- [Khronos glTF](https://github.com/KhronosGroup/glTF) : format ouvert d’échange et de livraison d’assets 3D; extensions et versions sont à valider.
- [mapeditor/rs-tiled](https://github.com/mapeditor/rs-tiled) : bibliothèque Rust de lecture TMX/TSX. Le dépôt du projet indique déjà une intégration Tiled; vérifier code et limites plutôt que recommencer à zéro.
- [Aseprite MCP](https://github.com/diivi/aseprite-mcp) : outil communautaire d’édition/animation pixel-art. Son README décrit une voie Lua capable d’exécuter du code hôte; considérer cette capacité à haut risque et désactivée sans permission explicite.
- [MCP for Blender](https://github.com/ahujasid/mcp-for-blender) : candidat communautaire; vérifier confidentialité, exécution de code, permissions, maintenance et configuration. Ne pas présumer d’une approbation officielle Blender ou d’une sûreté par popularité.
- Kenney et autres bibliothèques libres : intégrer uniquement après vérification des conditions de redistribution, API, licences de chaque pack et éventuelles exigences d’attribution.

---

**Fin du contrat de cadrage.** Les choix stratégiques sont confirmés; détails de plateforme, catalogue et implémentation peuvent être précisés dans des décisions techniques sans rouvrir la cible produit.
