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
/** Prove view choices survive workflow activation, closure, rename, and session restoration. */
import { jest } from '@jest/globals';
import { WorkflowViewPreferences } from '../../../frontend/comfyui/ui/workflow/application/WorkflowViewPreferences.js';

test('retains independent choices without persistence or reconstruction on tab switches', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  const a = { id: 'a', path: 'workflows/a.json' };
  const b = { id: 'b', path: 'workflows/b.json' };
  views.activate(a, () => true);
  views.change({ masonry: true });
  const first = views.snapshot;
  views.activate(b, () => true);
  views.change({ canvas: false });
  write.mockClear();
  for (let index = 0; index < 100; index++) {
    views.activate(a, () => true);
    expect(views.snapshot).toBe(first);
    views.activate(b, () => true);
    expect(views.snapshot).toEqual({ canvas: false, masonry: false, editorWidth: 0 });
  }
  expect(write).not.toHaveBeenCalled();
  views.retain(['b']);
  views.activate({ ...a, id: 'reopened' }, () => true);
  expect(views.snapshot).toBe(first);
});

test('restores persisted false choices and carries accepted state through Save As', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences(
    [{ path: 'workflows/a.json', canvas: false, masonry: true }],
    write,
  );
  views.activate({ id: 'a', path: 'workflows/a.json' }, () => true);
  expect(views.snapshot).toEqual({ canvas: false, masonry: true, editorWidth: 0 });
  views.activate({ id: 'a', path: 'workflows/renamed.json' }, () => true);
  const restored = new WorkflowViewPreferences(write.mock.calls.at(-1)![0], write);
  restored.activate({ id: 'new-session', path: 'workflows/renamed.json' }, () => true);
  expect(restored.snapshot).toEqual({ canvas: false, masonry: true, editorWidth: 0 });
});

test('rejects malformed persistence and bounds stored choices without evicting live workflows', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([{ path: 'x', canvas: 'false', masonry: true }], write);
  views.activate({ id: 'x', path: 'x' }, () => true);
  expect(views.snapshot).toEqual({ canvas: true, masonry: false, editorWidth: 0 });
  views.change({ masonry: true });
  const first = views.snapshot;
  for (let index = 0; index < 140; index++) {
    views.activate({ id: String(index), path: `workflows/${index}.json` }, () => true);
    views.change({ canvas: false });
  }
  expect(write.mock.calls.at(-1)![0]).toHaveLength(128);
  views.activate({ id: 'x', path: 'x' }, () => true);
  expect(views.snapshot).toBe(first);
});

test('remembers each workflow editor width and restores it after switching and reopening', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  const a = { id: 'a', path: 'workflows/a.json' };
  const b = { id: 'b', path: 'workflows/b.json' };
  views.activate(a, () => true);
  expect(views.snapshot.editorWidth).toBe(0);
  expect(views.change({ editorWidth: 900 })).toBe(true);
  expect(views.change({ editorWidth: 900 })).toBe(false);
  views.activate(b, () => true);
  expect(views.snapshot.editorWidth).toBe(0);
  views.change({ editorWidth: 1200 });
  views.activate(a, () => true);
  expect(views.snapshot.editorWidth).toBe(900);
  views.activate(b, () => true);
  expect(views.snapshot.editorWidth).toBe(1200);
  expect(() => views.change({ editorWidth: -1 })).toThrow(RangeError);
  expect(() => views.change({ editorWidth: Infinity })).toThrow(RangeError);
  expect(views.snapshot.editorWidth).toBe(1200);
  const restored = new WorkflowViewPreferences(write.mock.calls.at(-1)![0], write);
  restored.activate({ id: 'reopened-a', path: 'workflows/a.json' }, () => true);
  expect(restored.snapshot).toEqual({ canvas: true, masonry: false, editorWidth: 900 });
  restored.activate({ id: 'reopened-b', path: 'workflows/b.json' }, () => true);
  expect(restored.snapshot.editorWidth).toBe(1200);
});

test('restores entries stored before editor widths with the default width and keeps their choices', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences(
    [
      { path: 'workflows/legacy.json', canvas: false, masonry: true },
      { path: 'workflows/corrupt.json', canvas: true, masonry: true, editorWidth: 'wide' },
      { path: 'workflows/huge.json', canvas: true, masonry: false, editorWidth: 1e9 },
    ],
    write,
  );
  views.activate({ id: 'legacy', path: 'workflows/legacy.json' }, () => true);
  expect(views.snapshot).toEqual({ canvas: false, masonry: true, editorWidth: 0 });
  views.activate({ id: 'corrupt', path: 'workflows/corrupt.json' }, () => true);
  expect(views.snapshot).toEqual({ canvas: true, masonry: true, editorWidth: 0 });
  views.activate({ id: 'huge', path: 'workflows/huge.json' }, () => true);
  expect(views.snapshot).toEqual({ canvas: true, masonry: false, editorWidth: 0 });
  expect(write).not.toHaveBeenCalled();
  views.change({ editorWidth: 1000 });
  expect(write.mock.calls.at(-1)![0]).toContainEqual({
    path: 'workflows/huge.json',
    canvas: true,
    masonry: false,
    editorWidth: 1000,
  });
});
