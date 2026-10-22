// Archétypes de jeu « en 1 clic » — données partagées entre le Hub et la galerie dédiée.

export interface Archetype {
  id: string;
  name: string;
  genre: string;
  color: string;
  borderColor: string;
  emoji: string;
  description: string;
  defaultScene: string;
  defaultGame: string;
  features: string[];
  controls: string[];
}

export const ARCHETYPES: Archetype[] = [
  {
    id: 'rpg',
    name: 'RPG DONJON',
    genre: 'Top-Down 2D',
    color: '#10b981',
    borderColor: '#059669',
    emoji: '🗡️',
    description: 'Explorez un donjon top-down : portes, leviers et coffres, un inventaire, et des sentinelles à esquiver.',
    defaultScene: 'level_01.scene.json',
    defaultGame: 'Niveau 1 : Salle des Émeraudes',
    features: ['20 murs de donjon', 'Objectif : 50 pièces d’or', '2 sentinelles A*', 'Dalles + porte KVL'],
    controls: ['Flèches / ZQSD : se déplacer (8 directions)', 'E : interagir (portes, coffres)', 'Dash : esquive', 'Combat au corps à corps'],
  },
  {
    id: 'platformer',
    name: 'PLATFORMER',
    genre: 'Vue Latérale 2D',
    color: '#8b5cf6',
    borderColor: '#7c3aed',
    emoji: '🥷',
    description: 'Gravité, double saut et dash : enchaînez les plateformes flottantes jusqu’au portail d’arrivée.',
    defaultScene: 'platformer_01.scene.json',
    defaultGame: 'Niveau Plateforme : L’Ascension des Nuages',
    features: ['Gravité & sauts', '3 pièces aériennes', 'Plateformes flottantes', 'Portail d’arrivée'],
    controls: ['← → : se déplacer', 'Espace : saut / double saut', 'E : interagir', 'Dash en plein saut'],
  },
  {
    id: 'shmup',
    name: 'SHMUP SPATIAL',
    genre: 'Action Espace',
    color: '#ec4899',
    borderColor: '#db2777',
    emoji: '🚀',
    description: 'Pilotez un vaisseau à inertie, tirez en continu et survivez aux vagues puis au boss final.',
    defaultScene: 'space_01.scene.json',
    defaultGame: 'Niveau Spatial : Ceinture d’Astéroïdes',
    features: ['Inertie newtonienne', '2 astéroïdes', 'Drone patrouilleur', 'Capsule stellaire'],
    controls: ['Flèches / ZQSD : piloter (inertie)', 'Tir automatique', 'Dash : esquive', 'Vagues + boss final'],
  },
  {
    id: 'sokoban',
    name: 'PUZZLE SOKOBAN',
    genre: 'Grille Tactique',
    color: '#f59e0b',
    borderColor: '#d97706',
    emoji: '🧩',
    description: 'Déplacez des caisses sur des dalles de pression pour activer le portail logique, case par case.',
    defaultScene: 'puzzle_01.scene.json',
    defaultGame: 'Sanctuaire des Mécanismes',
    features: ['2 caisses runiques', '2 dalles de pression', 'Portail logique', '2 orbes ancestrales'],
    controls: ['Flèches / ZQSD : se déplacer (grille)', 'E : pousser / activer', 'Résolvez l’enchaînement', 'Aucun saut'],
  },
  {
    id: 'boss',
    name: 'TITAN BOSS',
    genre: 'Combat Arène',
    color: '#ef4444',
    borderColor: '#dc2626',
    emoji: '⚔️',
    description: 'Affrontez le Colosse dans une arène fermée : esquivez la télégraphie au sol et ripostez.',
    defaultScene: 'boss_01.scene.json',
    defaultGame: 'Arène du Titan : Sanctuaire de la Sentinelle',
    features: ['Titan Proto-Sentinelle', 'Télégraphie au sol', '2 cœurs de vie', 'Arène 8 murs + piliers'],
    controls: ['Flèches / ZQSD : se déplacer (8 directions)', 'Espace : saut', 'Dash : esquive', 'Combat hybride'],
  },
];
