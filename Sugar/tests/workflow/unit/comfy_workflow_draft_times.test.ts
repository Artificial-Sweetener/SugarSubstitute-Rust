//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prove Comfy's draft index is read strictly: a known index gives times, anything else is unreadable. */
import { expect, test } from '@jest/globals';
import { readComfyDraftTimes } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyWorkflowDraftTimes.js';

/** Model page storage over fixed items. */
function storage(
  items: Readonly<Record<string, string>>,
): Pick<Storage, 'length' | 'key' | 'getItem'> {
  const keys = Object.keys(items);
  return {
    length: keys.length,
    key: (index) => keys[index] ?? null,
    getItem: (key) => items[key] ?? null,
  };
}

/** Serialize an index in the shape Comfy stores. */
function index(entries: Readonly<Record<string, { path: string; updatedAt: unknown }>>): string {
  return JSON.stringify({ v: 2, updatedAt: 1, order: Object.keys(entries), entries });
}

test('each workflow answers with the latest time any Comfy workspace kept it', () => {
  const times = readComfyDraftTimes(
    storage({
      unrelated: 'value',
      'Comfy.Workflow.DraftIndex.v2:personal': index({
        a: { path: 'workflows/a.json', updatedAt: 100 },
        b: { path: 'workflows/b.json', updatedAt: 300 },
      }),
      'Comfy.Workflow.DraftIndex.v2:team': index({
        a: { path: 'workflows/a.json', updatedAt: 200 },
      }),
    }),
  );
  expect(times('workflows/a.json')).toBe(200);
  expect(times('workflows/b.json')).toBe(300);
  expect(times('workflows/none.json')).toBeNull();
});

test('storage with no draft index holds no copy of any workflow', () => {
  expect(readComfyDraftTimes(storage({ unrelated: 'value' }))('workflows/a.json')).toBeNull();
});

test.each([
  ['an index of another version', { 'Comfy.Workflow.DraftIndex.v3:personal': index({}) }],
  ['an index that is not JSON', { 'Comfy.Workflow.DraftIndex.v2:personal': '{' }],
  ['an index of another shape', { 'Comfy.Workflow.DraftIndex.v2:personal': '{"v":2}' }],
  [
    'an entry without a time',
    {
      'Comfy.Workflow.DraftIndex.v2:personal': index({
        a: { path: 'workflows/a.json', updatedAt: 'soon' },
      }),
    },
  ],
])('%s makes every answer unreadable', (_name, items) => {
  expect(readComfyDraftTimes(storage(items))('workflows/a.json')).toBe('unreadable');
});

test('storage that refuses to be read makes every answer unreadable', () => {
  const refusing: Pick<Storage, 'length' | 'key' | 'getItem'> = {
    length: 1,
    key: () => {
      throw new Error('denied');
    },
    getItem: () => null,
  };
  expect(readComfyDraftTimes(refusing)('workflows/a.json')).toBe('unreadable');
});
