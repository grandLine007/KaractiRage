# Design tokens — KaractiRage Studio

> WS1 (UX-102 + UX-103) · Référence unique des jetons d'interface Dark Atelier.
> Source de vérité : `apps/studio/src/index.css` (`:root` + `@theme`).
> Règle générale : **aucun composant nouveau n'utilise de couleur/valeur codée en dur** — il référence un token.

## 1. Couleurs

### Surfaces & bordures

| Token | Valeur | Usage |
|---|---|---|
| `--bg-deep` | `#07040d` | fond de scène / viewport |
| `--bg-panel` | `#0d0819` | panneaux, asides |
| `--bg-card` | `#150d27` | cartes, champs |
| `--bg-card-hover` | `#1e1338` | survol de carte |
| `--border-subtle` | `#2d1b4e` | séparation 1 px |
| `--border-focus` | `#a855f7` | focus / sélection |

### Texte

| Token | Valeur | Contraste (panneau/carte) | Usage |
|---|---|---|---|
| `--text-main` | `#f8fafc` | 15,6:1 | texte principal |
| `--text-muted` | `#94a3b8` | 7,68:1 / 7,32:1 | texte secondaire |
| `--text-faint` | `#64748b` | 4,14:1 / 3,94:1 ❌ | **décoratif uniquement** (icônes), jamais du texte informatif |

### Accents (1 accent = 1 rôle)

| Token | Valeur | Rôle |
|---|---|---|
| `--accent-primary` | `#a855f7` | action principale, focus, sélection |
| `--success` | `#34d399` | validation, succès, FPS OK |
| `--warning` | `#f59e0b` | avertissement, score, pause |
| `--danger` | `#f43f5e` | erreur, suppression |
| `--info` | `#38bdf8` | information, liens, cyan |

> Pas de violet « d'ambiance » sur un état de succès/erreur : chaque état a **un** accent.

## 2. Mise à l'échelle (UX-103)

### Espacements

| Tokens | Valeurs |
|---|---|
| `--space-2/4/6/8/12/16/24/32` | 2 / 4 / 6 / 8 / 12 / 16 / 24 / 32 px |

Échelle unique : `2 4 6 8 12 16 24 32`. Pas de valeurs intermédiaires arbitraires
(`p-5`, `gap-7`, `m-3.5`) dans les nouveaux composants.

### Rayons

| Tokens | Valeurs |
|---|---|
| `--radius-4/6/8/12` | 4 / 6 / 8 / 12 px |

### Élévation (3 niveaux max)

| Token | Valeur | Usage |
|---|---|---|
| `--shadow-e1` | `0 1px 2px rgba(0,0,0,.4)` | cartes, champs |
| `--shadow-e2` | `0 2px 6px rgba(0,0,0,.45)` | menus, panneaux flottants |
| `--shadow-e3` | `0 6px 18px rgba(0,0,0,.5)` | modales, overlays |

Les glows colorés (halos violets) sont réservés à **l'identité** (logo, badge actif) ;
le reste utilise l'élévation neutre ci-dessus.

## 3. Typographie (UX-101 — à trancher §13.3)

Échelle cible : **12 / 13 / 14 / 16 / 20 / 24 / 32** · line-height 1,4 (corps), 1,2 (titres).
Interdiction de texte porteur de sens **< 12 px**.

| Rôle | Police (décision §13.3 à confirmer) |
|---|---|
| Corps | Outfit (ou système) |
| Titres | Space Grotesk (ou système) |
| IDs / code | JetBrains Mono (ou système) |

## 4. Règles d'usage

1. **Capitales** réservées aux badges ≤ 3 mots ; les labels sont en casse de phrase.
2. **Letter-spacing** uniquement sur les capitales.
3. **Bordures 1 px** seulement pour séparer, jamais pour décorer chaque carte.
4. **Contraste** : texte normal ≥ 4,5:1 ; grand texte ≥ 3:1 (K3).
5. **Cibles** : boutons ≥ 24×24 px (K5, WCAG 2.5.8).

## 5. Vérification

```bash
cd apps/studio
npm run audit:ui            # mesures header/canvas/contraste (branche WS0)
```
