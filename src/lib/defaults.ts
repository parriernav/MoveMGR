import type { LocalState, Project } from './types';
import { translate, type Language } from './i18n';

export const newProject = (language: Language = 'ko'): Project => ({
  id: crypto.randomUUID(),
  name: translate(language, 'newProject'),
  checked: false,
  source: {
    roots: [],
    recursive: false,
    includeHidden: false,
    moveUnit: 'file',
    extensions: { mode: 'all' },
    nameFilters: []
  },
  key: { extractor: { kind: 'beforeDelimiter', delimiter: '_', missing: 'skip' }, trim: true },
  comparisonOptions: { ignoreCase: true, normalization: 'NFC' },
  target: { root: '', destination: { mode: 'root' } },
  conflict: 'skip'
});

export const emptyState: LocalState = {
  schemaVersion: 1,
  revision: 0,
  preferences: { theme: 'system', language: 'en', previewBeforeRun: true },
  projects: [],
  ruleTags: []
};

export const cloneProject = (project: Project, language: Language = 'ko'): Project => ({
  ...structuredClone(project),
  id: crypto.randomUUID(),
  name: `${project.name}${translate(language, 'copySuffix')}`,
  checked: false
});
