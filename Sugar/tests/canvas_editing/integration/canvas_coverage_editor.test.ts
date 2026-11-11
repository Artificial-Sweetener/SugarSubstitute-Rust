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
/** Prove the canvas menu opens the exclusive coverage editor, which Esc closes. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import type { CanvasEditCommand } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { editState } from '../support/CanvasEditHostFixture.js';
import { disposeEditing, flush, mountEditing, press } from '../support/CanvasEditOverlayFixture.js';

afterEach(() => {
  disposeEditing();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Open the canvas menu on the region the native canvas leaves to the page. */
function openCanvasMenu(region: HTMLElement): HTMLElement | undefined {
  region.dispatchEvent(
    new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 400, clientY: 300 }),
  );
  return [
    ...document.querySelectorAll<HTMLElement>('[data-canvas-action="edit-layer-coverage"]'),
  ].at(-1);
}

describe('canvas coverage editor', () => {
  test('opens from the canvas menu as a modal bottom panel and closes with Esc', async () => {
    const f = await mountEditing(
      editState({ selection: { bounds: { left: 0, top: 0, width: 9, height: 9 } } }),
    );
    f.host.reply = (command: CanvasEditCommand) => {
      if (command.type === 'mode')
        f.host.state = { ...f.host.state, mode: command.mode, revision: f.host.state.revision + 1 };
      return { kind: 'state', state: f.host.state };
    };
    const outputMenu = jest.fn();
    f.scope.addEventListener('contextmenu', outputMenu);
    const entry = openCanvasMenu(f.viewport)!;
    expect(outputMenu).not.toHaveBeenCalled();
    expect(entry.textContent).toContain('Edit layer coverage');
    expect(entry.getAttribute('aria-disabled')).not.toBe('true');
    entry.click();
    await flush();
    const panel = f.query('[role="dialog"]')!;
    expect(panel.getAttribute('aria-modal')).toBe('true');
    expect(panel.getAttribute('aria-label')).toBe('Edit layer coverage');
    expect(f.query('.sc-canvas-edit-rail')).toBeNull();
    expect(f.query('.sc-canvas-edit-toolbar')).toBeNull();
    expect(document.activeElement).toBe(panel.querySelector('select'));
    // The menu is withheld while the coverage editor owns the canvas.
    expect(openCanvasMenu(f.viewport)).toBeUndefined();
    expect(press(panel.querySelector('select')!, 'Escape')).toBe(true);
    await flush();
    expect(f.host.sent()).toEqual([
      { type: 'mode', mode: 'coverage' },
      { type: 'modify', target: 'mask', operation: 'expand', pixels: 4, phase: 'preview' },
      { type: 'modify', target: 'mask', operation: 'expand', pixels: 4, phase: 'cancel' },
      { type: 'mode', mode: 'none' },
    ]);
    expect(f.query('[role="dialog"]')).toBeNull();
    expect(f.query('.sc-canvas-edit-rail')).not.toBeNull();
  });

  test('applies the whole-mask modification and disables controls meanwhile', async () => {
    const f = await mountEditing(editState({ mode: 'coverage' }));
    f.host.hold = true;
    await f.click('[role="dialog"] [data-canvas-action="apply"]');
    expect(
      f.query<HTMLButtonElement>('[role="dialog"] [data-canvas-action="apply"]')?.disabled,
    ).toBe(true);
    f.host.release();
    await flush();
    f.host.release();
    await flush();
    expect(f.host.sent()).toEqual([
      { type: 'modify', target: 'mask', operation: 'expand', pixels: 4, phase: 'apply' },
      { type: 'mode', mode: 'none' },
    ]);
  });

  test('disables Edit layer coverage without an active mask', async () => {
    const f = await mountEditing(editState({ activeMask: null }));
    expect(openCanvasMenu(f.viewport)?.getAttribute('aria-disabled')).toBe('true');
  });
});
