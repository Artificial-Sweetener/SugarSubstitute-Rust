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
/** Prove the rail, its flyouts, tool options and history render host state and emit commands. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { editState } from '../support/CanvasEditHostFixture.js';
import { disposeEditing, flush, mountEditing } from '../support/CanvasEditOverlayFixture.js';

afterEach(() => {
  disposeEditing();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

describe('canvas tool rail', () => {
  test('renders nothing until a document is shown and nothing after the host is retired', async () => {
    const f = await mountEditing(editState({ document: null }));
    expect(f.root.children).toHaveLength(0);
    await f.push({ document: 'document-1' });
    expect(f.query('.sc-canvas-edit-rail')).not.toBeNull();
    f.mounted.attach(null);
    await flush();
    expect(f.root.children).toHaveLength(0);
    expect(f.host.observers).toBe(0);
    f.mounted.dispose();
  });

  test('shows slots in order with vendored icons, the active tool pressed and unavailable disabled', async () => {
    const tools = editState().tools.map((tool) =>
      tool.id.startsWith('mask-') ? { ...tool, available: false } : tool,
    );
    const f = await mountEditing(editState({ tool: 'brush', tools }));
    const buttons = [...f.root.querySelectorAll<HTMLButtonElement>('[data-canvas-slot]')];
    expect(buttons.map((button) => button.dataset.canvasTool)).toEqual([
      'pan-zoom',
      'move',
      'brush',
      'eraser',
      'select-rectangle',
      'mask-rectangle',
      'transform',
    ]);
    expect(
      buttons.map((button) => button.querySelector('[data-icon]')?.getAttribute('data-icon')),
    ).toEqual([
      'pan-zoom',
      'move',
      'brush',
      'eraser',
      'select-rectangle',
      'mask-rectangle',
      'transform',
    ]);
    const icon = buttons[0]!.querySelector<HTMLElement>('.sc-canvas-edit-icon')!;
    expect(icon.style.getPropertyValue('--sc-canvas-edit-icon')).toMatch(
      /icons\/canvas-editing\/pan-zoom\.svg/,
    );
    expect(buttons.map((button) => button.getAttribute('aria-pressed'))).toEqual([
      'false',
      'false',
      'true',
      'false',
      'false',
      'false',
      'false',
    ]);
    expect(buttons[5]!.disabled).toBe(true);
    expect(buttons[0]!.getAttribute('aria-label')).toBe('Pan & Zoom');
    expect(buttons.filter((b) => b.getAttribute('aria-haspopup') === 'menu')).toHaveLength(3);
    await f.click('[data-canvas-slot="move"]');
    expect(f.host.sent()).toEqual([{ type: 'tool', tool: 'move' }]);
    await f.push({ tool: 'move' });
    expect(f.query('[data-canvas-slot="move"]')?.getAttribute('aria-pressed')).toBe('true');
  });

  test('opens a group flyout on the secondary button and activates the chosen member', async () => {
    const f = await mountEditing();
    const outputMenu = jest.fn();
    f.scope.addEventListener('contextmenu', outputMenu);
    const group = f.query('[data-canvas-slot="selection"]')!;
    group.dispatchEvent(
      new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 40, clientY: 200 }),
    );
    expect(outputMenu).not.toHaveBeenCalled();
    const rows = [...document.querySelectorAll<HTMLElement>('[data-canvas-tool]')].filter(
      (row) => !f.root.contains(row),
    );
    expect(rows.map((row) => [row.dataset.canvasTool, row.getAttribute('aria-checked')])).toEqual([
      ['select-rectangle', 'true'],
      ['select-ellipse', 'false'],
      ['select-lasso', 'false'],
    ]);
    rows[2]!.click();
    await flush();
    expect(f.host.sent()).toEqual([{ type: 'tool', tool: 'select-lasso' }]);
    expect(f.query('[data-canvas-slot="selection"]')?.dataset.canvasTool).toBe('select-lasso');
    await f.click('[data-canvas-slot="selection"]');
    expect(f.host.sent().at(-1)).toEqual({ type: 'tool', tool: 'select-lasso' });
  });

  test('shows brush options with the tip in the mask colour and sends slider values', async () => {
    const f = await mountEditing(editState({ tool: 'brush' }));
    const tip = f.query('.sc-canvas-edit-tip')!;
    expect(tip.getAttribute('data-color')).toBe('#33cc99');
    expect(tip.getAttribute('data-hardness')).toBe('80');
    expect(f.query('input[type="range"]')).toBeNull();
    await f.click('[data-canvas-options] button[aria-expanded]');
    expect(f.query('[data-canvas-option-value="size"]')?.textContent).toBe('40 px');
    expect(f.query('[data-canvas-option-value="opacity"]')?.textContent).toBe('100%');
    const size = f.query<HTMLInputElement>('input[data-canvas-option="size"]')!;
    expect([size.min, size.max]).toEqual(['1', '1000']);
    size.value = '250';
    size.dispatchEvent(new Event('input', { bubbles: true }));
    await flush();
    expect(f.host.sent()).toEqual([
      { type: 'options', tool: 'brush', options: { size: 250, hardness: 80, opacity: 100 } },
    ]);
    expect(f.query('[data-canvas-option-value="size"]')?.textContent).toBe('250 px');
    await f.push({ tool: 'eraser' });
    expect(f.query('[data-canvas-options]')?.getAttribute('data-canvas-options')).toBe('eraser');
    expect(f.query('.sc-canvas-edit-tip')?.getAttribute('data-color')).toBe('#ffffff');
    await f.push({ tool: 'pan-zoom' });
    expect(f.query('[data-canvas-options]')).toBeNull();
  });

  test('reflects document history in Undo and Redo', async () => {
    const f = await mountEditing();
    const undo = () => f.query<HTMLButtonElement>('[data-canvas-action="undo"]')!;
    expect(undo().disabled).toBe(true);
    await f.push({ history: { undo: true, redo: false } });
    expect(undo().disabled).toBe(false);
    expect(f.query<HTMLButtonElement>('[data-canvas-action="redo"]')!.disabled).toBe(true);
    await f.click('[data-canvas-action="undo"]');
    expect(f.host.sent()).toEqual([{ type: 'history', action: 'undo' }]);
  });
});
