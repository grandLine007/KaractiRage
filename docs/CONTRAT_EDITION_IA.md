> **Statut : document de travail — remplacé comme référence par
> [`CONTRAT_GLOBAL_KARACTIRAGE_2D_3D_IA.md`](CONTRAT_GLOBAL_KARACTIRAGE_2D_3D_IA.md) v0.2.**
> Conservé pour deux apports non détaillés dans le contrat global : la mécanique exacte de
> la boucle (§3) et l'échelle de validation en huit marches (§5). En cas de divergence,
> le contrat global prime.

# Contrat d'édition IA — spécification

| Champ | Valeur |
|---|---|
| Statut | **proposition à valider** |
| Date | 17 novembre 2026 |
| Portée | toute modification du projet, par une IA, quel que soit le modèle |
| Remplace | la description éparse de `ai_edit.rs` / `ai_patch.rs` |

> Objectif : rendre la création **facile**. L'utilisateur ne code pas, ne modélise pas de
> A à Z, et n'a pas besoin de Blender. Il décrit, l'IA propose, le moteur valide.

---

## 1. Principes non négociables

1. **Rien n'est appliqué sans validation.** Une proposition IA est exécutée dans un bac
   à sable ; seul un résultat validé modifie le projet.
2. **Tout est adressable.** Entité, écran, asset, niveau, réglage, graphe KVL, matériau,
   caméra : chaque chose a une adresse stable qu'une IA peut cibler.
3. **L'IA ne reçoit jamais un « bravo ».** Succès = un jeton court. Échec = un diagnostic
   exploitable. C'est ce qui fait converger un petit modèle local sans brûler de tokens.
4. **Le moteur décide, pas le modèle.** Les opérations sont un catalogue fermé ; une
   opération inconnue est une erreur, pas une extension du langage.
5. **Hors ligne par défaut.** Aucun appel réseau n'est nécessaire pour éditer. Les
   catalogues en ligne sont une commodité, pas une dépendance.
6. **Réversible.** Toute application validée est annulable. Le bac à sable ne laisse
   aucune trace en cas d'échec.

---

## 2. Les acteurs

| Acteur | Rôle | Ne fait jamais |
|---|---|---|
| **Utilisateur** | décrit l'intention en langage naturel | ne choisit pas d'opération technique |
| **IA** (locale ou distante) | traduit l'intention en opérations | ne modifie pas l'état directement |
| **Moteur** | applique, valide, teste, annule, répond | n'invente pas d'intention |
| **Outils externes** | Blender, éditeurs 3D, catalogues d'assets | ne sont pas requis pour créer |

---

## 3. Le cycle d'édition (la boucle)

```text
         ┌──────────────────────────────────────────────────────────┐
         │                                                          │
intention│  1. l'IA produit une PROPOSITION                          │
   │     └─ { intention, operations[] }                            │
   │                                                               │
   │  2. le moteur l'applique dans un BAC À SABLE                  │
   │     └─ copie d'état + journal d'annulation                    │
   │                                                               │
   │  3. le moteur VALIDE (échelle §5)                             │
   │     ├─ succès → COMMIT + réponse {"ok": true}                 │
   │     └─ échec  → ANNULE + réponse {"ok": false, errors: [...]} │
   │                                                               │
   │  4. l'IA lit les erreurs et recommence ───────────────────────┘
   │     (borné : tentatives_max)
   │
   └─ épuisement → l'utilisateur voit la dernière erreur, l'état est intact
```

### 3.1 Réponse de succès — volontairement pauvre

```json
{ "ok": true }
```

Rien d'autre n'est envoyé au modèle. Le résumé lisible (« 3 entités déplacées ») est
calculé par le moteur **pour l'interface**, jamais pour le modèle.

### 3.2 Réponse d'échec — volontairement précise

```json
{
  "ok": false,
  "errors": [
    {
      "code": "cible-introuvable",
      "target": "entity:42",
      "message": "aucune entité d'identifiant 42 dans la scène courante",
      "hint": "utilise entity:12 ou tag:enemy"
    }
  ]
}
```

Règles du diagnostic :
- **borné** : au plus 8 erreurs, 400 caractères chacune ;
- **stable** : le `code` ne change pas de formulation entre versions ;
- **actionnable** : `hint` dit quoi faire, pas seulement ce qui a échoué ;
- **localisé** : `message` en français pour l'utilisateur, `code` en ASCII stable pour le modèle.

### 3.3 Paramètres de la boucle

| Paramètre | Proposition par défaut | Réglable |
|---|---|---|
| `tentatives_max` | **3** | oui (réglage projet) |
| Détection de non-progrès | même proposition deux fois → arrêt | — |
| `no-op` (proposition vide ou identique à l'état) | `EditError::NoChange`, ne compte pas comme tentative | — |
| Annulation utilisateur | interrompt la boucle entre deux tentatives | — |
| Coût | une tentative = un appel modèle, pas plus | — |

---

## 4. L'espace adressable

Une adresse est une chaîne stable, lisible par un modèle, sans ambiguïté.

| Forme | Cible |
|---|---|
| `entity:<id>` / `entity:<nom>` | une entité |
| `tag:<tag>` / `kind:<tag>` | toutes les entités d'un tag |
| `entity:<id>/transform` | un champ précis |
| `kvl:<graphe>/node:<id>` | un nœud KVL |
| `screen:<hud\|menu\|pause\|game_over\|victory>` | un écran d'interface |
| `level:<n>` | un niveau du projet |
| `asset:<id>` | un asset du manifeste |
| `project` | réglages du projet (titre, genre, dimension) |
| `project/export` | réglages d'export |
| `camera3d`, `lights` | caméra et lumières |
| `selection` / `hover` / `cursor:<x,y>` | contexte implicite |

**Règle** : toute opération qui cible une adresse inexistante échoue avec
`cible-introuvable` et un `hint` listant les adresses proches. Une IA ne doit jamais
« deviner » silencieusement.

---

## 5. L'échelle de validation

Une proposition est validée en cascade ; la première marche qui échoue arrête tout.

| # | Marche | Ce qui est vérifié | Code d'erreur |
|---|---|---|---|
| 1 | **Syntaxe** | JSON conforme au schéma des opérations | `json-invalide` |
| 2 | **Catalogue** | chaque opération existe, arité correcte | `operation-inconnue` |
| 3 | **Cibles** | chaque adresse se résout | `cible-introuvable` |
| 4 | **Application** | application sur l'état en bac à sable sans erreur | `application-impossible` |
| 5 | **Invariants** | pas de référence orpheline, graphe KVL valide, pas de doublon d'id, budgets d'assets | `invariant-viole` |
| 6 | **Chargement** | la scène se recharge (`from_scene_data`) | `scene-inchargeable` |
| 7 | **Simulation** | N frames (par défaut 60) sans panic, joueur présent si niveau jouable | `simulation-echec` |
| 8 | **Rendu** | un rendu de contrôle ne panique pas | `rendu-echec` |

Les marches 7 et 8 sont **conditionnelles** : simulation seulement si la modification
touche au gameplay (entités, physique, KVL), rendu seulement si elle touche à
l'affichage. Une retouche de titre ne doit pas coûter 60 frames.

---

## 6. Le catalogue d'opérations (fermé)

Familles actuelles (à conserver et compléter) :

| Famille | Exemples |
|---|---|
| Entité | créer, supprimer, dupliquer, transformer, propriété, composant |
| KVL | ajouter/supprimer nœud, connexion, paramètre |
| Scène | réglages, objectif, niveau |
| Interface | écran, élément, texte, position |
| Asset | importer, renommer, recadrer, remplacer, métadonnées, provenance |
| Projet | titre, genre, dimension, export |

**Règle** : une opération non listée est refusée avec `operation-inconnue` et la liste
**des noms autorisés** — pas une explication littéraire. Le modèle apprend le vocabulaire
en une erreur.

---

## 7. Les assets : import et provenance

Tout asset entre par la même porte, avec la même fiche.

```text
source (fichier hôte | catalogue | générateur)
   → hash du contenu
   → provenance { Studio | Pxl | Generated { modèle, prompt, seed } | Imported { outil } | Catalog { plateforme, id, url } }
   → licence (obligatoire pour un catalogue, recommandée sinon)
   → budget du manifeste
   → enregistrement
```

### 7.1 Import depuis la machine

| Format | Traitement |
|---|---|
| PNG / SVG / WAV | décodés, classés, enregistrés |
| GLB / GLTF | chargés comme mesh, enregistrés |
| **.blend** | converti **si Blender est présent** ; sinon refus explicite `blender-absent` avec la marche à suivre (exporter en GLB) |

Aucun import ne dépend de Blender : il est une **capacité détectée**, pas un prérequis.

### 7.2 Catalogues en ligne (contrat, pas encore branché)

```rust
trait Catalogue {
    fn identifiant(&self) -> &'static str;
    fn lister(&self, recherche: &str, limite: usize) -> Result<Vec<ItemCatalogue>, ErreurCatalogue>;
    fn telecharger(&self, item: &ItemCatalogue) -> Result<Vec<u8>, ErreurCatalogue>;
}
struct ItemCatalogue { id, titre, auteur, licence, url_page, url_fichier, format, miniature }
```

- **la licence est obligatoire** dans un item ; un item sans licence n'est pas listé ;
- hors ligne → `reseau-indisponible`, jamais de repli silencieux ;
- le moteur ne télécharge rien sans action explicite de l'utilisateur.

### 7.3 Édition d'un modèle 3D existant

Même contrat que le reste : transformation, matériau, remplacement de mesh, nom, tag.
Les retouches lourdes (sculpt, topology) restent du ressort d'un outil externe ; le
Studio orchestre, il ne remplace pas Blender.

---

## 8. Garanties

| Garantie | Mise en œuvre |
|---|---|
| Échec = état intact | application en bac à sable + annulation |
| Aucune modification silencieuse | réponse `ok` sans effet = `no-op` détecté |
| Coût borné | `tentatives_max` × un appel modèle |
| Déterminisme | mêmes opérations sur même état → même résultat, même empreinte |
| Rejouable | les opérations validées sont enregistrées dans l'historique IA |
| Hors ligne | aucune opération du cœur n'exige le réseau |

---

## 9. Décisions qui t'appartiennent

| # | Question | Proposition par défaut |
|---|---|---|
| D1 | `tentatives_max` : 3, 5, illimité tant que ça progresse ? | **3** |
| D2 | Faut-il simuler à chaque modification d'affichage ? | **non**, seulement gameplay |
| D3 | Une IA peut-elle télécharger un asset dans la boucle ? | **non** : elle propose, l'utilisateur autorise |
| D4 | Le message d'erreur pour le modèle : français ou anglais ? | code ASCII + message français |
| D5 | La boucle vit-elle dans le moteur ou dans l'application ? | **moteur** (testable, réutilisable) |
| D6 | Un catalogue sans licence est-il rejeté ? | **oui** |
| D7 | L'IA peut-elle réécrire l'historique (undo/redo) ? | **non** |

---

## 10. Ce que ce contrat ne promet pas

- Que l'IA produise un jeu **intéressant** : elle produit un jeu **valide**. Le goût reste
  humain.
- L'import de **tous** les formats 3D : GLB/GLTF d'abord, `.blend` si Blender est là.
- Le fonctionnement des catalogues hors ligne.
- La modification lourde de modèles 3D dans le Studio.
