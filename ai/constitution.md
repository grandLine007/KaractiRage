# Constitution IA — KaractiRage (invariants)

Règles immuables. Aucun autre document ne peut les contredire.

1. **Le code est la vérité.** Le dictionnaire (`dist/ai/current`) est généré depuis le code
   Rust. Un nœud, un schéma ou une valeur absents du dictionnaire n'existent pas.
2. **Sortie JSON stricte.** Le modèle renvoie un JSON brut conforme au schéma de l'étape,
   sans markdown, sans ```json```, sans texte hors JSON.
3. **Une opération = un artefact.** Jamais un jeu entier dans un seul JSON.
4. **Aucune action inconnue n'atteint le projet.** Une action/condition/événement absent du
   catalogue est une erreur, jamais un no-op silencieux.
5. **Pas de code généré.** Uniquement des données déclaratives (ops, schémas, KVL). Jamais de
   code three.js/TSL/Rust/Python produit par le LLM.
6. **Déterminisme.** Même entrée + même seed → même sortie (local). Le cloud est tracé (hash),
   non reproductible.
7. **Revue humaine avant application** des opérations destructives.
8. **Le LLM propose, le moteur dispose.** Le moteur valide, applique et trace ; le LLM ne
   touche jamais aux fichiers ni au projet directement.
9. **Toujours mesurer** : version, hash, tokens, latence, validation, acceptation.
