import type { LocalState } from './types';

export function migrateSourceFolders(state: LocalState): LocalState {
  const next = structuredClone(state);
  for (const project of next.projects) {
    const source = project.source as typeof project.source & { root?: string };
    source.roots ??= source.root?.trim() ? [source.root] : [];
    delete source.root;
  }
  return next;
}

export function mergeSourceFolders(current: string[], selected: string[], index?: number): string[] {
  const folders = [...current];
  if (index === undefined) folders.push(...selected);
  else folders.splice(index, 1, ...selected);
  const seen = new Set<string>();
  return folders.filter((folder) => {
    if (!folder.trim()) return false;
    const normalized = folder.replace(/\\/g, '/').replace(/\/+$/, '') || '/';
    const key = /^(?:[a-z]:(?:\/|$)|\/\/)/i.test(normalized) ? normalized.toLowerCase() : normalized;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
