import { describe, expect, it } from 'vitest';
import { nextPlanSort, sortedPlanItems } from './plan-sort';
import type { PlannedItem } from './types';

function item(id: string, projectName: string, key: string | null): PlannedItem {
  return {
    itemId: id, projectId: id, projectName, sourcePath: `${id}.png`,
    extractedKey: key, proposedTargetPath: null, decision: 'move',
    reasonCode: null, reasonText: null, sizeBytes: '1'
  };
}

describe('preview sorting', () => {
  it('toggles each column from ascending to descending and resets for another column', () => {
    const initial = { key: null, direction: 'asc' } as const;
    const ascending = nextPlanSort(initial, 'project');
    expect(ascending).toEqual({ key: 'project', direction: 'asc' });
    expect(nextPlanSort(ascending, 'project')).toEqual({ key: 'project', direction: 'desc' });
    expect(nextPlanSort(nextPlanSort(ascending, 'project'), 'source')).toEqual({ key: 'source', direction: 'asc' });
  });

  it('sorts Korean names naturally without changing the execution plan order', () => {
    const items = [item('a', '프로젝트 10', null), item('b', '프로젝트 2', '가'), item('c', '프로젝트 2', '나')];
    expect(sortedPlanItems(items, { key: 'project', direction: 'asc' }).map((entry) => entry.itemId)).toEqual(['b', 'c', 'a']);
    expect(sortedPlanItems(items, { key: 'key', direction: 'desc' }).map((entry) => entry.itemId)).toEqual(['c', 'b', 'a']);
    expect(items.map((entry) => entry.itemId)).toEqual(['a', 'b', 'c']);
  });
});
