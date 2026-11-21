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

/** A workflow with no recorded view and no Cubes, opening with the canvas shown or hidden. */
const graph = () => ({ canvas: true, masonry: false });
const graphNoCanvas = () => ({ canvas: false, masonry: false });

test('retains independent choices without persistence or reconstruction on tab switches', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  const a = { id: 'a', path: 'workflows/a.json' };
  const b = { id: 'b', path: 'workflows/b.json' };
  views.activate(a, graph);
  views.change({ masonry: true });
  const first = views.snapshot;
  views.activate(b, graph);
  views.change({ canvas: false });
  write.mockClear();
  for (let index = 0; index < 100; index++) {
    views.activate(a, graph);
    expect(views.snapshot).toBe(first);
    views.activate(b, graph);
    expect(views.snapshot).toEqual({ canvas: false, masonry: false, editorWidth: 0 });
  }
  expect(write).not.toHaveBeenCalled();
  views.retain(['b']);
  // The reopened workflow recorded masonry in itself; the rest comes from storage.
  views.activate({ ...a, id: 'reopened' }, () => ({ canvas: true, masonry: true }));
  expect(views.snapshot).toEqual(first);
});

test('restores persisted false choices and carries accepted state through Save As', () => {
  const write = jest.fn<(value: unknown) => void>();
  // An entry stored before workflows recorded their own view still restores its canvas choice;
  // its masonry flag is ignored, since the workflow says which view it opens in.
  const views = new WorkflowViewPreferences(
    [{ path: 'workflows/a.json', canvas: false, masonry: true }],
    write,
  );
  views.activate({ id: 'a', path: 'workflows/a.json' }, graph);
  expect(views.snapshot).toEqual({ canvas: false, masonry: false, editorWidth: 0 });
  views.activate({ id: 'a', path: 'workflows/renamed.json' }, graph);
  expect(write.mock.calls.at(-1)![0]).toContainEqual({
    path: 'workflows/renamed.json',
    canvas: false,
    editorWidth: 0,
    scroll: null,
  });
  const restored = new WorkflowViewPreferences(write.mock.calls.at(-1)![0], write);
  restored.activate({ id: 'new-session', path: 'workflows/renamed.json' }, () => ({
    canvas: true,
    masonry: true,
  }));
  expect(restored.snapshot).toEqual({ canvas: false, masonry: true, editorWidth: 0 });
});

test('a masonry switch is never written to browser storage', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  views.activate({ id: 'a', path: 'workflows/a.json' }, graph);
  write.mockClear();
  expect(views.change({ masonry: true })).toBe(true);
  expect(views.snapshot.masonry).toBe(true);
  expect(write).not.toHaveBeenCalled();
});

test('rejects malformed persistence and bounds stored choices without evicting live workflows', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([{ path: 'x', canvas: 'false', masonry: true }], write);
  views.activate({ id: 'x', path: 'x' }, graph);
  expect(views.snapshot).toEqual({ canvas: true, masonry: false, editorWidth: 0 });
  views.change({ masonry: true });
  const first = views.snapshot;
  for (let index = 0; index < 140; index++) {
    views.activate({ id: String(index), path: `workflows/${index}.json` }, graph);
    views.change({ canvas: false });
  }
  expect(write.mock.calls.at(-1)![0]).toHaveLength(128);
  views.activate({ id: 'x', path: 'x' }, graph);
  expect(views.snapshot).toBe(first);
});

test('remembers each workflow editor width and restores it after switching and reopening', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  const a = { id: 'a', path: 'workflows/a.json' };
  const b = { id: 'b', path: 'workflows/b.json' };
  views.activate(a, graph);
  expect(views.snapshot.editorWidth).toBe(0);
  expect(views.change({ editorWidth: 900 })).toBe(true);
  expect(views.change({ editorWidth: 900 })).toBe(false);
  views.activate(b, graph);
  expect(views.snapshot.editorWidth).toBe(0);
  views.change({ editorWidth: 1200 });
  views.activate(a, graph);
  expect(views.snapshot.editorWidth).toBe(900);
  views.activate(b, graph);
  expect(views.snapshot.editorWidth).toBe(1200);
  expect(() => views.change({ editorWidth: -1 })).toThrow(RangeError);
  expect(() => views.change({ editorWidth: Infinity })).toThrow(RangeError);
  expect(views.snapshot.editorWidth).toBe(1200);
  const restored = new WorkflowViewPreferences(write.mock.calls.at(-1)![0], write);
  restored.activate({ id: 'reopened-a', path: 'workflows/a.json' }, graph);
  expect(restored.snapshot).toEqual({ canvas: true, masonry: false, editorWidth: 900 });
  restored.activate({ id: 'reopened-b', path: 'workflows/b.json' }, graph);
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
  views.activate({ id: 'legacy', path: 'workflows/legacy.json' }, graph);
  expect(views.snapshot).toEqual({ canvas: false, masonry: false, editorWidth: 0 });
  views.activate({ id: 'corrupt', path: 'workflows/corrupt.json' }, graph);
  expect(views.snapshot).toEqual({ canvas: true, masonry: false, editorWidth: 0 });
  views.activate({ id: 'huge', path: 'workflows/huge.json' }, graph);
  expect(views.snapshot).toEqual({ canvas: true, masonry: false, editorWidth: 0 });
  expect(write).not.toHaveBeenCalled();
  views.change({ editorWidth: 1000 });
  expect(write.mock.calls.at(-1)![0]).toContainEqual({
    path: 'workflows/huge.json',
    canvas: true,
    editorWidth: 1000,
    scroll: null,
  });
});

test('a closed workflow reopens with the choices a session restore gave it', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  const restored = { id: 'restored', path: 'workflows/restored.json' };
  // A session restore binds choices without publishing them to browser persistence.
  views.restore(restored, { canvas: true, masonry: true, editorWidth: 640 });
  expect(write).not.toHaveBeenCalled();
  views.retain([]);
  expect(write).toHaveBeenCalledTimes(1);
  // Releasing again, or a workflow whose stored choices are current, writes nothing more.
  views.retain([]);
  expect(write).toHaveBeenCalledTimes(1);
  // The restored workflow recorded masonry in itself when it was switched.
  views.activate({ id: 'reopened', path: restored.path }, () => ({ canvas: true, masonry: true }));
  expect(views.snapshot).toEqual({ canvas: true, masonry: true, editorWidth: 640 });
  views.retain([]);
  expect(write).toHaveBeenCalledTimes(1);
});

test('each workflow keeps where its masonry was scrolled, across a restart and a session restore', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  const a = { id: 'a', path: 'workflows/a.json' };
  const b = { id: 'b', path: 'workflows/b.json' };
  views.activate(a, graph);
  const scrollA = views.scrollFor(a);
  views.activate(b, graph);
  const scrollB = views.scrollFor(b);
  // A memory taken for a workflow keeps answering for it after another one is shown.
  scrollA.record({ cube: '12', offset: 40 });
  scrollB.record({ cube: null, offset: 8 });
  expect(scrollA.read()).toEqual({ cube: '12', offset: 40 });
  expect(scrollB.read()).toEqual({ cube: null, offset: 8 });
  const stored = write.mock.calls.at(-1)![0];
  // The same scroll again writes nothing.
  write.mockClear();
  scrollA.record({ cube: '12', offset: 40 });
  expect(write).not.toHaveBeenCalled();

  // A restart reads it back by the workflow's path.
  const restarted = new WorkflowViewPreferences(stored, write);
  restarted.activate({ id: 'fresh', path: a.path }, graph);
  expect(restarted.scrollFor({ id: 'fresh', path: a.path }).read()).toEqual({
    cube: '12',
    offset: 40,
  });
  // A session restore names no scroll; the workflow keeps its own.
  restarted.restore({ id: 'fresh', path: a.path }, { canvas: true, masonry: true, editorWidth: 0 });
  expect(restarted.scrollFor({ id: 'fresh', path: a.path }).read()).toEqual({
    cube: '12',
    offset: 40,
  });
});

test('a scroll taken as a closed workflow masonry is torn down is still kept', () => {
  const write = jest.fn<(value: unknown) => void>();
  const views = new WorkflowViewPreferences([], write);
  const a = { id: 'a', path: 'workflows/a.json' };
  views.activate(a, graph);
  const memory = views.scrollFor(a);
  memory.record({ cube: '3', offset: 5 });
  views.retain([]);
  memory.record({ cube: '3', offset: 90 });
  const restarted = new WorkflowViewPreferences(write.mock.calls.at(-1)![0], write);
  expect(restarted.scrollFor({ id: 'again', path: a.path }).read()).toEqual({
    cube: '3',
    offset: 90,
  });
  // A stored scroll that is not a valid one is dropped, never trusted.
  const damaged = new WorkflowViewPreferences(
    [{ path: a.path, canvas: true, editorWidth: 0, scroll: { cube: 3, offset: -1 } }],
    write,
  );
  expect(damaged.scrollFor({ id: 'x', path: a.path }).read()).toBeNull();
});
