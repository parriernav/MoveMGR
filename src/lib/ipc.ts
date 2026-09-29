import { invoke } from '@tauri-apps/api/core';
import type { AppInfo, LocalState, Plan, Project, RunResult } from './types';

export const isTauri = () => '__TAURI_INTERNALS__' in window;

export async function getAppInfo(): Promise<AppInfo> {
  return invoke('get_app_info');
}

export async function loadState(): Promise<LocalState> {
  return invoke('load_state');
}

export async function saveState(state: LocalState, expectedRevision: number): Promise<LocalState> {
  return invoke('save_state', { nextState: state, expectedRevision });
}

export async function evaluateExample(project: Project, sampleName: string, sampleFolders: string[]) {
  return invoke<{ key: string | null; targetFolder: string | null; reason: string | null }>('evaluate_example', {
    project,
    sampleName,
    sampleFolders
  });
}

export async function createPlan(projectIds: string[]): Promise<Plan> {
  return invoke('create_plan', { projectIds });
}

export async function startRun(planId: string): Promise<RunResult> {
  return invoke('start_run', { planId });
}

export async function cancelRun(): Promise<void> {
  return invoke('cancel_run');
}

export async function listHistory(): Promise<RunResult[]> {
  return invoke('list_history');
}

export async function importSettings(path: string, mode: 'replace' | 'append'): Promise<LocalState> {
  return invoke('import_settings', { path, mode });
}

export async function exportSettings(path: string): Promise<void> {
  return invoke('export_settings', { path });
}
