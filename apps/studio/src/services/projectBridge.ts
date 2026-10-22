/**
 * Service Pont Projets KaractiRage (Project Bridge)
 *
 * Assure la communication entre le Hub de Projets React et le registre de
 * projets Rust (via les commandes Tauri `engine_*_project`). En mode navigateur
 * pur (hors Tauri), bascule sur une liste de démonstration en lecture seule.
 */

import { invoke } from '@tauri-apps/api/core';

export type EntryKind = 'scene' | 'project';
export type EntryStatus = 'available' | 'missing';

export interface ProjectEntry {
  path: string;
  title: string;
  description: string;
  objective: string | null;
  genre: string;
  dimension: string;
  kind: EntryKind;
  size_bytes: number;
  modified_at: number;
  entity_count: number;
  is_recent: boolean;
  status: EntryStatus;
}

export interface NewProjectConfig {
  title: string;
  author?: string;
  genre: string;
  dimension: string;
  archetype?: string | null;
  destination_dir: string;
  default_mode: string;
}

export interface SystemDiagnostics {
  os: string;
  arch: string;
  cpu_cores: number;
  fps: number;
  entities: number;
  projects_home: string;
  engine: string;
}

/** Vrai si le frontend tourne dans le conteneur Tauri (IPC disponible). */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * Récupère la liste réelle des projets/scènes depuis le registre Rust.
 * En mode navigateur, retourne une liste de démonstration statique.
 */
export async function listProjects(extraDirs: string[] = []): Promise<ProjectEntry[]> {
  if (!isTauri()) {
    return DEMO_PROJECTS;
  }
  const raw: string = await invoke('engine_list_projects', { extraDirs });
  try {
    return JSON.parse(raw) as ProjectEntry[];
  } catch {
    return [];
  }
}

/** Crée un projet sur disque et le charge immédiatement dans le moteur. */
export async function createProject(config: NewProjectConfig): Promise<ProjectEntry> {
  if (!isTauri()) {
    return DEMO_PROJECTS[0];
  }
  const raw: string = await invoke('engine_create_project', { configJson: JSON.stringify(config) });
  return JSON.parse(raw) as ProjectEntry;
}

/** Ouvre un projet/scène existant depuis son chemin disque. */
export async function openProject(path: string): Promise<number> {
  if (!isTauri()) return 0;
  return await invoke('engine_open_project', { path });
}

export async function recentProjects(): Promise<string[]> {
  if (!isTauri()) return [];
  const raw: string = await invoke('engine_recent_projects');
  try {
    return JSON.parse(raw) as string[];
  } catch {
    return [];
  }
}

export async function removeRecent(path: string): Promise<boolean> {
  if (!isTauri()) return false;
  return await invoke('engine_remove_recent', { path });
}

export async function duplicateProject(path: string, newTitle: string): Promise<ProjectEntry> {
  if (!isTauri()) return DEMO_PROJECTS[0];
  const raw: string = await invoke('engine_duplicate_project', { path, newTitle });
  return JSON.parse(raw) as ProjectEntry;
}

export async function renameProject(path: string, newTitle: string): Promise<ProjectEntry> {
  if (!isTauri()) return DEMO_PROJECTS[0];
  const raw: string = await invoke('engine_rename_project', { path, newTitle });
  return JSON.parse(raw) as ProjectEntry;
}

export async function archiveProject(path: string): Promise<string> {
  if (!isTauri()) return '';
  return await invoke('engine_archive_project', { path });
}

export async function revealProject(path: string): Promise<void> {
  if (!isTauri()) return;
  await invoke('engine_reveal_project', { path });
}

export async function systemDiagnostics(): Promise<SystemDiagnostics> {
  if (!isTauri()) {
    return {
      os: 'web',
      arch: 'browser',
      cpu_cores: navigator.hardwareConcurrency ?? 1,
      fps: 60,
      entities: 0,
      projects_home: '— (mode web)',
      engine: 'KaractiRage Engine Core 0.1.0',
    };
  }
  const raw: string = await invoke('engine_system_diagnostics');
  return JSON.parse(raw) as SystemDiagnostics;
}

/** Liste de démonstration utilisée uniquement hors Tauri (vite preview). */
const DEMO_PROJECTS: ProjectEntry[] = [
  {
    path: 'assets/scenes/level_01.scene.json',
    title: 'Donjon Niveau 1 — Aventure RPG Top-Down',
    description: 'Quêtes, clés, portes & sentinelles.',
    objective: 'Atteignez la sortie du donjon.',
    genre: 'Aventure',
    dimension: '2D',
    kind: 'scene',
    size_bytes: 36_864,
    modified_at: 1_752_000_000,
    entity_count: 39,
    is_recent: false,
    status: 'available',
  },
  {
    path: 'assets/scenes/platformer_01.scene.json',
    title: 'Cyber Platformer 2D — Gravité & Sauts',
    description: 'Moteurs de saut, plateformes mobiles.',
    objective: 'Survivez aux plateformes.',
    genre: 'Plateforme',
    dimension: '2D',
    kind: 'scene',
    size_bytes: 9_011,
    modified_at: 1_751_900_000,
    entity_count: 24,
    is_recent: false,
    status: 'available',
  },
  {
    path: 'assets/scenes/boss_01.scene.json',
    title: 'Arène du Titan — Combat de Boss & Télégraphie',
    description: 'Boss réactif KVL, télégraphie & phase 2.',
    objective: 'Terrassez le Titan.',
    genre: 'Combat',
    dimension: '2D',
    kind: 'scene',
    size_bytes: 11_878,
    modified_at: 1_751_800_000,
    entity_count: 18,
    is_recent: false,
    status: 'available',
  },
];
