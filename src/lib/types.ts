export type Extractor =
  | { kind: 'whole' }
  | { kind: 'firstChars'; count: number }
  | { kind: 'beforeDelimiter'; delimiter: string; missing: 'skip' | 'useWhole' };

export type ExtensionFilter =
  | { mode: 'all' }
  | { mode: 'only'; values: string[]; includeExtensionless: boolean };

export type NameFilter = {
  op: 'startsWith' | 'contains' | 'endsWith' | 'equals';
  value: string;
};

export type MultipleMatchPolicy = 'first' | 'roundRobin' | 'skip';

export type MoveUnit = 'file' | 'sameNameGroup';

export type Destination =
  | { mode: 'root' }
  | { mode: 'fixedSubfolder'; relativePath: string; createIfMissing: boolean }
  | {
      mode: 'matchSubfolder';
      searchBase: string;
      folderExtractor: Extractor;
      comparison:
        | { kind: 'equals' }
        | { kind: 'startsWith' }
        | { kind: 'prefixEqual'; count: number };
      noMatch: 'skip' | 'createKeyFolder';
      multipleMatches?: MultipleMatchPolicy;
    }
  | { mode: 'keySubfolder'; parentRelativePath: string; createIfMissing: boolean };

export type Project = {
  id: string;
  name: string;
  checked: boolean;
  source: {
    root: string;
    recursive: boolean;
    includeHidden: boolean;
    moveUnit?: MoveUnit;
    extensions: ExtensionFilter;
    nameFilters: NameFilter[];
  };
  key: { extractor: Extractor; trim: boolean };
  comparisonOptions: { ignoreCase: boolean; normalization: 'NFC' };
  target: { root: string; destination: Destination };
  conflict: 'skip' | 'renameWithNumber';
};

export type SourceRuleSnapshot = Pick<Project['source'], 'recursive' | 'includeHidden' | 'moveUnit' | 'extensions' | 'nameFilters'>;
export type TargetRuleSnapshot = Pick<Project, 'key' | 'comparisonOptions' | 'conflict'> & { destination: Destination };
export type RuleTag =
  | { id: string; name: string; kind: 'source'; rules: SourceRuleSnapshot }
  | { id: string; name: string; kind: 'target'; rules: TargetRuleSnapshot };

export type Preferences = {
  theme: 'system' | 'light' | 'dark';
  language: 'ko' | 'en' | 'ja' | 'zh';
  previewBeforeRun: boolean;
};

export type LocalState = {
  schemaVersion: 1;
  revision: number;
  preferences: Preferences;
  projects: Project[];
  ruleTags: RuleTag[];
};

export type PlannedItem = {
  itemId: string;
  projectId: string;
  projectName: string;
  sourcePath: string;
  extractedKey: string | null;
  proposedTargetPath: string | null;
  decision: 'move' | 'skip' | 'blocked';
  reasonCode: string | null;
  reasonText: string | null;
  sizeBytes: string;
};

export type Plan = {
  planId: string;
  createdAt: string;
  revision: number;
  items: PlannedItem[];
  movable: number;
  skipped: number;
  blocked: number;
};

export type RunResult = {
  runId: string;
  startedAt: string;
  finishedAt: string;
  moved: number;
  skipped: number;
  failed: number;
  sourceRetained: number;
  items: Array<PlannedItem & { state: string; finalTargetPath: string | null; message: string | null }>;
};

export type AppInfo = { version: string; platform: string; configDir: string };
