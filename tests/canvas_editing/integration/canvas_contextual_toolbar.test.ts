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
/** Prove the selection, modification, transform and session toolbars render and command. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { editState } from '../support/CanvasEditHostFixture.js';
import { disposeEditing, flush, mountEditing } from '../support/CanvasEditOverlayFixture.js';

afterEach(() => {
  disposeEditing();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

const selection = { bounds: { left: 300, top: 100, width: 200, height: 100 } };

describe('canvas contextual toolbar', () => {
  test('floats twelve pixels below the selection and follows its bounds', async () => {
    const f = await mountEditing();
    expect(f.query('.sc-canvas-edit-toolbar')).toBeNull();
    await f.push({ selection });
    const toolbar = f.query('.sc-canvas-edit-toolbar')!;
    expect(toolbar.dataset.canvasSurface).toBe('selection');
    expect(toolbar.dataset.canvasPlacement).toBe('300,212');
    await f.push({ selection: { bounds: { ...selection.bounds, left: 100 } } });
    expect(f.query('.sc-canvas-edit-toolbar')?.dataset.canvasPlacement).toBe('100,212');
    // Bottom overflow past the hysteresis margin moves it above the selection.
    await f.push({ selection: { bounds: { ...selection.bounds, left: 100, top: 520 } } });
    expect(f.query('.sc-canvas-edit-toolbar')?.dataset.canvasPlacement).toBe('100,472');
  });

  test('offers Modify selection, Transform, Clear and Deselect', async () => {
    const f = await mountEditing(editState({ selection }));
    const actions = [
      ...f.root.querySelectorAll<HTMLElement>('.sc-canvas-edit-toolbar [data-canvas-action]'),
    ];
    expect(actions.map((action) => action.dataset.canvasAction)).toEqual([
      'modify',
      'transform',
      'clear',
      'deselect',
    ]);
    expect(actions[0]?.textContent).toBe('Modify selection');
    await f.click('[data-canvas-action="transform"]');
    await f.click('[data-canvas-action="clear"]');
    await f.click('[data-canvas-action="deselect"]');
    expect(f.host.sent()).toEqual([
      { type: 'tool', tool: 'transform' },
      { type: 'selection', action: 'clear' },
      { type: 'selection', action: 'deselect' },
    ]);
    await f.push({ activeMask: null });
    expect(f.query<HTMLButtonElement>('[data-canvas-action="clear"]')?.disabled).toBe(true);
  });

  test('previews selection modification live with the rail hidden, then applies it', async () => {
    const f = await mountEditing(editState({ selection }));
    await f.click('[data-canvas-action="modify"]');
    expect(f.query('.sc-canvas-edit-rail')).toBeNull();
    expect(f.query('.sc-canvas-edit-toolbar')?.dataset.canvasSurface).toBe('modify');
    const operation = f.query<HTMLSelectElement>('[data-canvas-coverage="operation"]')!;
    expect([...operation.options].map((option) => option.textContent)).toEqual([
      'Expand',
      'Contract',
      'Feather',
    ]);
    operation.value = 'feather';
    operation.dispatchEvent(new Event('change', { bubbles: true }));
    await flush();
    const pixels = f.query<HTMLInputElement>('[data-canvas-coverage="pixels"]')!;
    expect(pixels.value).toBe('4');
    pixels.value = '2000';
    pixels.dispatchEvent(new Event('input', { bubbles: true }));
    await flush();
    await f.click('.sc-canvas-edit-toolbar [data-canvas-action="apply"]');
    expect(f.host.sent()).toEqual([
      { type: 'modify', target: 'selection', operation: 'expand', pixels: 4, phase: 'preview' },
      { type: 'modify', target: 'selection', operation: 'feather', pixels: 4, phase: 'preview' },
      { type: 'modify', target: 'selection', operation: 'feather', pixels: 999, phase: 'preview' },
      { type: 'modify', target: 'selection', operation: 'feather', pixels: 999, phase: 'apply' },
    ]);
    expect(f.query('.sc-canvas-edit-rail')).not.toBeNull();
  });

  test('presents rotate, flip, provisional history and settlement while transforming', async () => {
    const f = await mountEditing(
      editState({ selection, session: { kind: 'transform', undo: true, redo: false } }),
    );
    expect(f.query('.sc-canvas-edit-rail')).toBeNull();
    expect(f.query('.sc-canvas-edit-label')?.textContent).toBe('Transform selection');
    for (const action of ['rotate-left', 'rotate-right', 'flip-horizontal', 'flip-vertical'])
      await f.click(`[data-canvas-action="${action}"]`);
    expect(f.query<HTMLButtonElement>('[data-canvas-action="session-redo"]')?.disabled).toBe(true);
    await f.click('[data-canvas-action="session-undo"]');
    await f.click('.sc-canvas-edit-toolbar [data-canvas-action="apply"]');
    await f.click('.sc-canvas-edit-toolbar [data-canvas-action="cancel"]');
    expect(f.host.sent()).toEqual([
      { type: 'transform', action: 'rotate-left' },
      { type: 'transform', action: 'rotate-right' },
      { type: 'transform', action: 'flip-horizontal' },
      { type: 'transform', action: 'flip-vertical' },
      { type: 'session', action: 'undo' },
      { type: 'commit' },
      { type: 'cancel' },
    ]);
    await f.push({ selection: null });
    expect(f.query('.sc-canvas-edit-label')?.textContent).toBe('Transform');
    // Without selection bounds the toolbar sits centred along the canvas bottom.
    expect(f.query('.sc-canvas-edit-toolbar')?.dataset.canvasPlacement).toBe('300,556');
  });

  test('presents the edit-session toolbar for an armed or active shared-edge session', async () => {
    const f = await mountEditing(editState({ tool: 'shared-edges' }));
    expect(f.query('.sc-canvas-edit-label')?.textContent).toBe('Resize shared edges');
    expect(f.query('.sc-canvas-edit-rail')).not.toBeNull();
    expect(
      f.query<HTMLButtonElement>('.sc-canvas-edit-toolbar [data-canvas-action="apply"]')?.disabled,
    ).toBe(true);
    await f.click('.sc-canvas-edit-toolbar [data-canvas-action="cancel"]');
    await f.push({ session: { kind: 'shared-edges', undo: true, redo: true } });
    await f.click('[data-canvas-action="session-redo"]');
    await f.click('.sc-canvas-edit-toolbar [data-canvas-action="apply"]');
    await f.click('.sc-canvas-edit-toolbar [data-canvas-action="cancel"]');
    expect(f.host.sent()).toEqual([
      { type: 'cancel' },
      { type: 'session', action: 'redo' },
      { type: 'session', action: 'apply' },
      { type: 'session', action: 'cancel' },
    ]);
  });
});
