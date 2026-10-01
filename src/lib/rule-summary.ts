import type { Extractor, Project } from './types';
import { translate, type Language, type MessageKey } from './i18n';

function extractionSummary(extractor: Extractor, subject: 'filename' | 'folder', language: Language): string {
  const prefix = subject === 'filename' ? 'summaryFilename' : 'summaryFolder';
  if (extractor.kind === 'whole') return translate(language, `${prefix}Whole` as MessageKey);
  if (extractor.kind === 'firstChars') return translate(language, `${prefix}First` as MessageKey, { count: extractor.count });
  return translate(language, `${prefix}Before` as MessageKey, { delimiter: extractor.delimiter });
}

export function targetSummaryLines(project: Project, language: Language = 'ko'): string[] {
  const key = translate(language, 'summaryKey', { value: extractionSummary(project.key.extractor, 'filename', language) });
  const destination = project.target.destination;
  const conflict = translate(language, project.conflict === 'skip' ? 'summaryConflictSkip' : 'summaryConflictRename');

  if (destination.mode === 'root') return [key, translate(language, 'summaryRoot'), conflict];
  if (destination.mode === 'fixedSubfolder') {
    return [
      key,
      translate(language, 'summaryFixed', { path: destination.relativePath || translate(language, 'summaryMissing') }),
      translate(language, 'summaryFolderMissing', { action: translate(language, destination.createIfMissing ? 'summaryCreate' : 'summarySkip'), conflict })
    ];
  }
  if (destination.mode === 'keySubfolder') {
    return [
      key,
      translate(language, 'summaryKeyFolder', { path: destination.parentRelativePath || translate(language, 'summaryTargetRoot') }),
      translate(language, 'summaryFolderMissing', { action: translate(language, destination.createIfMissing ? 'summaryCreate' : 'summarySkip'), conflict })
    ];
  }

  const base = destination.searchBase || translate(language, 'summaryTargetRoot');
  const folderKey = extractionSummary(destination.folderExtractor, 'folder', language);
  const compare = destination.comparison.kind === 'equals'
    ? translate(language, 'summaryCompareEquals')
    : destination.comparison.kind === 'startsWith'
      ? translate(language, 'summaryCompareStarts')
      : translate(language, 'summaryComparePrefix', { count: destination.comparison.count });
  return [
    key,
    translate(language, 'summaryExisting', { path: base }),
    translate(language, 'summaryFolderCompare', { folderKey, comparison: compare }),
    translate(language, 'summaryMultipleMatches', {
      policy: translate(language, destination.multipleMatches === 'skip'
        ? 'multipleMatchesSkip'
        : destination.multipleMatches === 'roundRobin' ? 'multipleMatchesRoundRobin' : 'multipleMatchesFirst')
    }),
    translate(language, 'summaryNoMatch', { action: translate(language, destination.noMatch === 'skip' ? 'summarySkip' : 'createKeyFolder'), conflict })
  ];
}
