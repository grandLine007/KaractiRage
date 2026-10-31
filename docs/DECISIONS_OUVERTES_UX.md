# Décisions ouvertes UX — KaractiRage Studio

> À trancher pour débloquer UX-101 (WS1), UX-501 (WS5) et WS6.
> Source : `docs/PLAN_ACTION_UX_UI_EQUIPE_IA.md` §13.
> Chaque décision a une **recommandation** (option par défaut si tu réponds « la reco »).

---

## D13.1 — Typographie (§13.3)

**Question** : charger les polices d'identité (Outfit / Space Grotesk / JetBrains Mono,
woff2 auto-hébergées) ou assumer la police système ?

| Option | Pour | Contre |
|---|---|---|
| **A. Polices d'identité** (woff2) | identité Dark Atelier, typographie industrielle | +2-3 graisses à charger, FOUT à gérer, bundle +30-60 Ko |
| **B. Police système** | zéro poids, zéro FOUT, perf maximale | identité affaiblie |

> **Recommandation : A** (Outfit corps + Space Grotesk titres + JetBrains Mono IDs), en
> woff2 subset, `font-display: swap`, 2 graisses max. L'identité est un non-négociable (§2.1).

> ✅ **Décision prise : A.** Implémentation UX-101 : woff2 auto-hébergées (Outfit/Space
> Grotesk/JetBrains Mono), `@font-face` + préchargement des 2 graisses utiles, échelle
> 12→32 px. (Nécessite les fichiers woff2 — lot dédié.)

---

## D13.2 — Portée du Hub (§13.2)

**Question** : refonte complète plein écran 3 choix, ou hub minimal d'abord ?

| Option | Pour | Contre |
|---|---|---|
| **A. Refonte complète (UX-501)** | K11 (démo ≤ 60 s), parcours débutant | ~5 j, retarde la v1 |
| **B. Hub minimal** (3 choix d'abord) | rapide, ne bloque pas la v1 | UX-501 repoussé |

> **Recommandation : B** — livrer d'abord les 3 choix visuels (Jouer une démo / Créer /
> Comprendre) sans refondre le reste du hub, puis itérer.

> ✅ **Décision prise : B — implémenté.** `ProjectHubModal` ouvre désormais sur un
> écran d'accueil « 3 choix » (Jouer une démo → `ArchetypesModal`, Créer → formulaire,
> Comprendre → visite guidée `StudioGuideModal`) + accès discret « Ouvrir un projet
> existant » et bouton « Accueil » pour y revenir. K11 : démo lancée en 2 clics.

---

## D13.3 — Portée de l'éditeur KVL (§13.4)

**Question** : éditeur complet (câblage, undo, params) ou générateur de recettes + insertion ?

| Option | Pour | Contre |
|---|---|---|
| **A. Éditeur complet** | K12 (règle connectée ≤ 2 min), contrat « éditeur visuel » | ~8 j (WS6) |
| **B. Générateur + insertion** (statut quo) | déjà en place | ne tient pas le contrat visuel |

> **Recommandation : A** — c'est le cœur de la promesse « éditeur visuel » ; à séquencer
> (câblage d'abord, puis undo, puis params).

> ✅ **Décision prise : A.** WS6 séquencé en 3 vagues, **toutes livrées** :
> ① UX-601+602 (palette filtrable + cartes lisibles), ② UX-604 (câblage des pins,
> suppression arête/nœud, undo/redo Ctrl+Z/Y), ③ UX-603+606 (inspecteur de nœud au clic,
> gestion des graphes nouveau/renommer/dupliquer/supprimer, badge « ✓ Enregistré »,
> état vide « Créer ma première règle »). UX-605 : insertion + câblage manuels couverts
> par ①+② ; l'assistant « insérer & câbler » dédié reste un raffinement optionnel.

---

## Comment répondre
Une ligne suffit, ex. : **« D13.1 A, D13.2 B, D13.3 A »** (ou « la reco » pour tout).

---

## Plan d'intégration (une fois les décisions prises)

Ordre de fusion des branches (chacune partant de `feat/tauri-react-studio`) :

```
1. feat/ux-ws0-audit          → PR vers feat/tauri-react-studio   (harnais, baseline, testids)
2. feat/ux-ws1-design-system  → PR vers feat/tauri-react-studio   (tokens, DESIGN_TOKENS.md)
3. feat/ux-ws2-header         → PR vers feat/ux-ws1-design-system (empilé : header + statut)
4. feat/ux-ws3-viewport       → PR vers feat/tauri-react-studio   (letterbox, zoom)
5. feat/ux-ws4-a11y           → PR vers feat/tauri-react-studio   (Tab, focus, labels, lang)
6. feat/ux-ws5-hub            → PR vers feat/tauri-react-studio   (libellés hub)
```

> Après fusion de WS0 + WS2 : mettre à jour les sélecteurs du harnais
> (`ATELIER STUDIO` → `Créer`, `LOGIQUE KVL` → `Logique`, …) puis relancer
> `npm run audit:ui` pour valider K2/K5/K6 et exécuter UX-206/306/410.

> PR via `gh` : il faut d'abord `gh auth login` (le CLI n'est pas authentifié ici).
