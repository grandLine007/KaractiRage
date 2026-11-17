# Audit du dépôt contre le contrat global — 17 novembre 2026

| Champ | Valeur |
|---|---|
| Référence | [`CONTRAT_GLOBAL_KARACTIRAGE_2D_3D_IA.md`](CONTRAT_GLOBAL_KARACTIRAGE_2D_3D_IA.md) v0.2 |
| Étape de la feuille de route | **A — contrat et audit** |
| Commit audité | `9d8f7e0` (+ documents non commités) |
| Méthode | lecture ciblée du code, exécution des gates, vérification des rapports de version |

> But : remplacer les suppositions par un état factuel. Chaque ligne cite ce qui existe
> réellement, pas ce qui est prévu.

---

## 1. Ce qui est déjà en place (vérifié)

| Contrat | État | Preuve |
|---|---|---|
| §2.1 jeu 2D solo complet (scènes, collisions, logique, UI, audio, sauvegarde, menus) | ✅ | 598 tests, export jouable 3 niveaux, boss, menu, HUD |
| §3.1 « scène jouable sans lire de documentation » | ⚠️ partiel | parcours automatisé mesuré à 3,3 s ; **le propriétaire n'a pas trouvé le parcours** — le problème est l'interface, pas le moteur |
| §3.2 édition scène/hiérarchie/inspecteur/KVL/UI | ✅ | Studio + audit 39 captures, 0 violation |
| §3.2 undo/redo, sauvegarde | ✅ | transaction IA annulable, `SaveState`, 3 slots |
| §3.2 import Tiled | ✅ v1 | objets + tilesets ; un seul tileset, pas de rotations |
| §3.3 animation 2D | ⚠️ partiel | spritesheets/clips ; éditeur de timeline absent |
| §3.4 export + pré-vol bloquant | ✅ | pré-vol script/asset/manifeste, 0 réseau, déterministe |
| §5.2 cycle transactionnel | ⚠️ partiel | proposition typée, application sur clone, undo, historique ; **pas de boucle de reprise sur erreur** |
| §5.2 catalogue d'opérations fermé | ✅ | 31 opérations typées, schéma publié (`dist/ai`) |
| §6.1 fournisseur IA | ⚠️ partiel | compatible OpenAI + Ollama/LM Studio configurés ; pas de détection de capacités |
| §8 manifeste d'assets | ✅ | provenance, licence, hash, budget, pré-vol |
| §9.1 moteur 2D | ✅ | rendu logiciel, tilemaps, collisions, audio |
| §10 profils de performance | ⚠️ partiel | adaptation existante, mesures FPS headless ; pas de profils publiés Modeste/Standard/Élevé |
| §11 critères par modification | ⚠️ partiel | gates solides ; pas de recette manuelle systématique |
| §14 format de rapport d'agent | ✅ | 17 rapports de version au format |

## 2. Ce qui manque, par ordre d'impact

| # | Manque | Contrat | Pourquoi c'est bloquant |
|---|---|---|---|
| 1 | **Boucle de reprise IA sur erreur** | §5.2 « limiter les tentatives, détecter les répétitions et no-op, diagnostic borné » | c'est la colonne vertébrale de « facile avec l'IA » ; aujourd'hui une proposition invalide est simplement rejetée |
| 2 | **Import GLB/GLTF et `.blend` optionnel** | §2.1, §3.4, §8.3 | la 3D déclarative n'a aucune porte d'entrée |
| 3 | **Adaptateurs de catalogues** | §8.1 | aucune recherche ni licence affichée ; les fournisseurs candidats sont documentés mais non branchés |
| 4 | **Modes N0–N3** | §5.1 | templates déterministes et agent outillé absents ; l'IA actuelle est N1/N2 seulement |
| 5 | **Génération SVG/chiptune** | §6.4, D10 | rien n'est généré ; seuls fichiers décodés/importés |
| 6 | **3D déclarative** (scène, caméra, matériaux, clips) | §9.2 | le rendu moteur est 2D ; la prévisualisation three.js du Studio ne s'exporte pas |
| 7 | **Profils de performance publiés** | §10 | cible « Modeste » affirmée, jamais mesurée sur matériel modeste |
| 8 | **Inventaire des licences** | §12 | dispersé dans les rapports, non consolidé |
| 9 | **MCP / packs d'outils** | §7 | spécifiés, absents |
| 10 | **Recette manuelle par livrable** | §11 | automatisation forte, mais l'expérience réelle a déjà révélé 3 défauts non couverts |

## 3. Contradictions ou points à trancher

| Point | Contrat | Réalité | Proposition |
|---|---|---|---|
| « jouable en < 2 min » | §1.2, §3.1 | le moteur y arrive en 3,3 s ; **l'utilisateur s'y perd** | transformer le Hub en parcours à 3 boutons, mesurer le temps **dans l'app**, pas par harnais |
| 3D en v1 | §2.1 | le runtime exporté est 2D | soit 2.5D déclarative en v1 (rendu 3D, gameplay 2D), soit décaler la 3D en v2 — **décision D2 à préciser** |
| Blender | §2.2, §8.2 | installé chez le propriétaire, absent chez beaucoup | capacité **détectée**, jamais requise ; `.blend` refusé avec marche à suivre |
| Catalogues | §8.1 | réseau bloqué (`crates.io`, `api.github.com` indisponibles) | écrire le contrat + un fournisseur factice maintenant ; brancher à la restauration réseau |
| « Aucune IA requise » | §6.1 | éditeur manuel complet | à tester explicitement : projet sans IA, sans réseau, exportable |

## 4. Ordre proposé

Le contrat dit de ne pas rouvrir une brique stable. B (boucle 2D) et C (manifeste/import)
sont largement faits ; l'étape suivante du contrat est **D — IA interopérable**.

| Lot | Contenu | Critère de sortie |
|---|---|---|
| **D1** | Boucle IA : proposition → bac à sable → validation graduée → `ok` ou diagnostics → reprise bornée | un fournisseur factice échoue puis réussit ; l'état n'est modifié qu'une fois ; une proposition toujours invalide s'arrête proprement |
| **D2** | Modes N0/N1 : templates déterministes avant tout LLM | « rends plus rapide » sans IA, sans réseau, même résultat |
| **D3** | Diagnostic borné + non-progrès | même proposition deux fois = arrêt ; ≤ 8 erreurs, ≤ 400 caractères |
| **C2** | Import GLB/GLTF + `.blend` si Blender détecté | déposer un GLB → entité 3D dans la scène → export |

## 5. Ce que je propose de faire immédiatement

1. **C2 puis D1** : l'import 3D donne un résultat *visible* (le propriétaire voit un
   modèle dans sa scène), et la boucle IA est le contrat qui rend tout le reste éditable.
2. **Repenser le Hub** en parallèle, parce que le retour réel (« je ne sais pas comment
   créer un jeu ») vaut plus que n'importe quel lot technique.
3. Écrire le contrat de catalogue **sans** l'implémenter contre un vrai service tant que
   le réseau est bloqué.

## 6. Limite de cet audit

Je n'ai pas exécuté de scénario manuel sur la fenêtre Tauri (glisser-déposer réel,
première expérience) : l'environnement d'agent n'a pas de session interactive fiable. Les
lignes §3.1 et §5.2 de cet audit s'appuient sur le code et les captures, pas sur une
recette humaine — et c'est précisément là que le retour du propriétaire a contredit
l'automatisation.
