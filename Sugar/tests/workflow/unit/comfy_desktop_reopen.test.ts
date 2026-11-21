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
/** Reopen the tabs a checkpoint holds that Comfy did not, in the checkpoint's order. */
import { jest } from '@jest/globals';
import { ComfyDesktopDocuments } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyDesktopDocuments.js';
import type { SessionEntry } from '../../../integrations/contracts/SessionProtocol.generated.js';

function entry(id: string, name: string, path: string | null): SessionEntry {
  return {
    id,
    document: {
      graphJson: JSON.stringify({ nodes: [], extra: { name } }),
      graphRevision: 1,
      path,
      name,
      temporary: path === null,
      dirty: path === null,
      sourceIdentity: null,
      view: { x: 0, y: 0, scale: 1 },
      baselineGraphJson: null,
    },
  } as unknown as SessionEntry;
}

test('tabs Comfy did not reopen come back in the background, checkpoint tabs first and in order', () => {
  const saved = { path: 'workflows/saved.json' };
  const kept = { path: 'workflows/kept.json' };
  const store = {
    getWorkflowByPath: jest.fn((path: string) =>
      path === saved.path ? saved : path === kept.path ? kept : null,
    ),
    createTemporary: jest.fn((filename: string, _graph: unknown) => ({
      path: `workflows/${filename}`,
    })),
    openWorkflowsInBackground: jest.fn(),
  };
  const documents = new ComfyDesktopDocuments({
    app: { extensionManager: { workflow: store } },
    sessions: {} as never,
    adapter: {} as never,
  });
  const entries = [
    entry('a', 'kept', kept.path),
    entry('b', 'Unsaved', null),
    entry('c', 'saved', saved.path),
    entry('d', 'gone', 'workflows/deleted.json'),
  ];
  expect(documents.reopen(entries, new Set())).toBe(false);
  expect(documents.reopen(entries, new Set(['b', 'c', 'd']))).toBe(true);
  // The unsaved tab, and the saved one whose file is gone, reopen from the checkpoint's document.
  expect(store.createTemporary.mock.calls.map(([name]) => name)).toEqual([
    'Unsaved.json',
    'gone.json',
  ]);
  expect(store.openWorkflowsInBackground).toHaveBeenCalledWith({
    left: [
      'workflows/kept.json',
      'workflows/Unsaved.json',
      'workflows/saved.json',
      'workflows/gone.json',
    ],
  });
});
