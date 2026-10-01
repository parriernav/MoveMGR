import { describe, expect, it } from 'vitest';
import { newProject } from './defaults';
import { targetSummaryLines } from './rule-summary';

describe('target rule summary', () => {
  it('shows both delimiter extractors, search scope, comparison and outcomes', () => {
    const project = newProject();
    project.key.extractor = { kind: 'beforeDelimiter', delimiter: '-', missing: 'skip' };
    project.target.destination = {
      mode: 'matchSubfolder',
      searchBase: '',
      folderExtractor: { kind: 'beforeDelimiter', delimiter: '-', missing: 'skip' },
      comparison: { kind: 'equals' },
      noMatch: 'skip'
    };
    project.conflict = 'renameWithNumber';
    expect(targetSummaryLines(project)).toEqual([
      '분류값: 파일명 ‘-’ 앞',
      '목적지: 타겟 루트의 기존 폴더',
      '폴더명 ‘-’ 앞 · 정확 일치',
      '복수 일치: 첫 번째 폴더에 모으기',
      '미일치 건너뜀 · 동명 파일 번호 붙임'
    ]);
  });

  it.each([
    ['first', '첫 번째 폴더에 모으기'],
    ['roundRobin', '순서대로 균등 배분'],
    ['skip', '건너뛰기']
  ] as const)('shows the %s policy in the project summary', (multipleMatches, label) => {
    const project = newProject();
    project.target.destination = {
      mode: 'matchSubfolder', searchBase: '', folderExtractor: { kind: 'whole' },
      comparison: { kind: 'equals' }, noMatch: 'skip', multipleMatches
    };
    expect(targetSummaryLines(project)).toContain(`복수 일치: ${label}`);
  });

  it('shows key-named folder placement without suggesting an existing-folder match', () => {
    const project = newProject();
    project.target.destination = { mode: 'keySubfolder', parentRelativePath: '완료', createIfMissing: true };
    expect(targetSummaryLines(project)).toContain('목적지: 완료 아래 분류값 폴더');
  });
});
