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
/** Prove keyboard shortcuts reach the canvas only while it owns focus and no text field does. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { editState } from '../support/CanvasEditHostFixture.js';
import { disposeEditing, flush, mountEditing, press } from '../support/CanvasEditOverlayFixture.js';

afterEach(() => {
  disposeEditing();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

describe('canvas editing keyboard', () => {
  test('routes the reference shortcuts while focus rests on the page or the canvas', async () => {
    const f = await mountEditing(
      editState({ tool: 'move', selection: { bounds: { left: 1, top: 1, width: 2, height: 2 } } }),
    );
    expect(press(document.body, ' ')).toBe(true);
    expect(press(document.body, ' ', { repeat: true })).toBe(true);
    expect(press(document.body, ' ', {}, 'keyup')).toBe(true);
    expect(press(document.body, 'z', { ctrlKey: true })).toBe(true);
    expect(press(document.body, 'Z', { ctrlKey: true, shiftKey: true })).toBe(true);
    expect(press(document.body, 'y', { ctrlKey: true })).toBe(true);
    f.viewport.tabIndex = 0;
    f.viewport.focus();
    expect(press(f.viewport, 'Enter')).toBe(true);
    expect(press(f.viewport, 'Escape')).toBe(true);
    expect(press(f.viewport, 'Delete')).toBe(true);
    expect(press(f.viewport, 'ArrowRight', { shiftKey: true })).toBe(true);
    expect(press(f.viewport, 'b')).toBe(false);
    expect(press(f.viewport, '[')).toBe(false);
    await flush();
    expect(f.host.sent()).toEqual([
      { type: 'navigation', held: true },
      { type: 'navigation', held: false },
      { type: 'history', action: 'undo' },
      { type: 'history', action: 'redo' },
      { type: 'history', action: 'redo' },
      { type: 'commit' },
      { type: 'cancel' },
      { type: 'selection', action: 'clear' },
      { type: 'nudge', dx: 10, dy: 0 },
    ]);
  });

  test('leaves keys to text fields and to focus owners outside the canvas', async () => {
    const f = await mountEditing();
    const search = document.createElement('input');
    document.body.append(search);
    search.focus();
    expect(press(search, 'z', { ctrlKey: true })).toBe(false);
    expect(press(search, ' ')).toBe(false);
    const field = document.createElement('input');
    f.scope.append(field);
    field.focus();
    expect(press(field, 'Delete')).toBe(false);
    const graph = document.createElement('button');
    document.body.append(graph);
    graph.focus();
    expect(press(graph, 'z', { ctrlKey: true })).toBe(false);
    await flush();
    expect(f.host.sent()).toEqual([]);
  });

  test('lets the chrome keep Enter and Space for its own buttons', async () => {
    const f = await mountEditing();
    const button = f.query<HTMLButtonElement>('[data-canvas-slot="move"]')!;
    button.focus();
    expect(press(button, ' ')).toBe(false);
    expect(press(button, ' ', {}, 'keyup')).toBe(false);
    expect(press(button, 'Enter')).toBe(false);
    expect(press(button, 'z', { ctrlKey: true })).toBe(false);
    await flush();
    expect(f.host.sent()).toEqual([]);
  });

  test('releases a held Space when the window loses focus and stops after disposal', async () => {
    const f = await mountEditing();
    press(document.body, ' ');
    window.dispatchEvent(new Event('blur'));
    await flush();
    expect(f.host.sent()).toEqual([
      { type: 'navigation', held: true },
      { type: 'navigation', held: false },
    ]);
    f.mounted.dispose();
    expect(press(document.body, 'z', { ctrlKey: true })).toBe(false);
  });

  test('takes no keys before the host presents an editable document', async () => {
    const f = await mountEditing(editState({ document: null }));
    expect(press(document.body, 'z', { ctrlKey: true })).toBe(false);
    await flush();
    expect(f.host.sent()).toEqual([]);
  });
});
