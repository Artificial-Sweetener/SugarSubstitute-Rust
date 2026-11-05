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
/** Prove Fluent row navigation, selected-row alignment, literal labels and popup retirement. */

import { jest } from '@jest/globals';
import { createApp, h, nextTick } from 'vue';
import { CanvasAnchoredPicker } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasAnchoredPicker.js';
import { CanvasBatchPicker } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasBatchPicker.js';
import { ensureCanvasOverlayStyles } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById('sugarcubes-output-overlay-styles')?.remove();
  jest.restoreAllMocks();
});

test('aligns the active source row over its control and commits keyboard navigation once', async () => {
  const host = document.createElement('section');
  document.body.append(host);
  const choose = jest.fn<(value: string) => void>();
  const app = createApp({
    setup: () => () =>
      h(CanvasAnchoredPicker, {
        label: 'Stage',
        value: 'b',
        items: [
          { value: 'a', label: 'A' },
          { value: 'b', label: '<b>B</b>' },
          { value: 'empty', label: 'Empty', disabled: true },
          { value: 'c', label: 'C' },
        ],
        choose,
      }),
  });
  app.mount(host);
  const anchor = host.querySelector<HTMLButtonElement>('button')!;
  jest.spyOn(anchor, 'getBoundingClientRect').mockReturnValue({
    x: 200,
    y: 100,
    left: 200,
    top: 100,
    right: 320,
    bottom: 128,
    width: 120,
    height: 28,
    toJSON: () => ({}),
  });
  const forwarded = jest.fn();
  document.body.addEventListener('keydown', forwarded);
  try {
    anchor.click();
    await nextTick();
    const popup = document.querySelector<HTMLElement>('[role="listbox"]')!;
    expect(popup.style.left).toBe('192px');
    expect(popup.style.top).toBe('62px');
    expect(popup.style.minWidth).toBe('136px');
    // Rows are as tall as the anchor so the active row covers it exactly.
    expect(popup.style.getPropertyValue('--sc-picker-row')).toBe('28px');
    expect(popup.style.getPropertyValue('--sc-picker-anchor-width')).toBe('120px');
    expect(popup.querySelector<HTMLButtonElement>('[aria-disabled="true"]')?.disabled).toBe(true);
    expect(popup.querySelector('[aria-selected="true"]')?.textContent).toBe('<b>B</b>');
    expect(popup.querySelector('b')).toBeNull();
    expect(document.activeElement).toBe(popup.querySelector('[aria-selected="true"]'));
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
    );
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
    );
    await nextTick();
    expect(choose).toHaveBeenCalledTimes(1);
    expect(choose).toHaveBeenCalledWith('c');
    expect(forwarded).not.toHaveBeenCalled();
    expect(document.querySelector('[role="listbox"]')).toBeNull();
    expect(document.activeElement).toBe(anchor);
  } finally {
    document.body.removeEventListener('keydown', forwarded);
    app.unmount();
  }
});

test('bounds long picker overflow and retires outside-input listeners with its overlay', async () => {
  const host = document.createElement('section');
  document.body.append(host);
  const added = jest.spyOn(document, 'addEventListener');
  const removed = jest.spyOn(document, 'removeEventListener');
  const choose = jest.fn();
  const app = createApp({
    setup: () => () =>
      h(CanvasAnchoredPicker, {
        label: 'Batch',
        value: '49',
        centered: true,
        items: Array.from({ length: 50 }, (_, index) => ({
          value: String(index),
          label: String(index + 1),
        })),
        choose,
      }),
  });
  app.mount(host);
  const anchor = host.querySelector<HTMLButtonElement>('button')!;
  jest.spyOn(anchor, 'getBoundingClientRect').mockReturnValue({
    x: 100,
    y: 700,
    left: 100,
    top: 700,
    right: 134,
    bottom: 728,
    width: 34,
    height: 28,
    toJSON: () => ({}),
  });
  anchor.click();
  await nextTick();
  const popup = document.querySelector<HTMLElement>('[role="listbox"]')!;
  expect(parseFloat(popup.style.top)).toBeGreaterThanOrEqual(0);
  expect(parseFloat(popup.style.top) + parseFloat(popup.style.maxHeight)).toBeLessThanOrEqual(
    window.innerHeight,
  );
  expect(popup.scrollTop).toBeGreaterThan(0);
  document.body.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
  await nextTick();
  expect(document.querySelector('[role="listbox"]')).toBeNull();
  anchor.click();
  await nextTick();
  document.activeElement?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
  );
  await nextTick();
  expect(choose).not.toHaveBeenCalled();
  anchor.click();
  await nextTick();
  app.unmount();
  expect(document.querySelector('[role="listbox"]')).toBeNull();
  const registered = added.mock.calls.filter(([type]) => type === 'pointerdown');
  expect(registered).toHaveLength(3);
  for (const registration of registered) expect(removed.mock.calls).toContainEqual(registration);
});

/** Open a three-row picker anchored in the last row slot above the viewport's bottom edge. */
async function openNearBottom(value: string): Promise<{ popup: HTMLElement; top: number }> {
  const host = document.createElement('section');
  document.body.append(host);
  const app = createApp({
    setup: () => () =>
      h(CanvasAnchoredPicker, {
        label: 'Scene',
        value,
        items: [
          { value: 'all', label: 'All' },
          { value: 'morning', label: 'Morning' },
          { value: 'night', label: 'Night' },
        ],
        choose: () => undefined,
      }),
  });
  app.mount(host);
  const anchor = host.querySelector<HTMLButtonElement>('button')!;
  const top = window.innerHeight - 40;
  jest.spyOn(anchor, 'getBoundingClientRect').mockReturnValue({
    x: 100,
    y: top,
    left: 100,
    top,
    right: 180,
    bottom: top + 30,
    width: 80,
    height: 30,
    toJSON: () => ({}),
  });
  anchor.click();
  await nextTick();
  return { popup: document.querySelector<HTMLElement>('[role="listbox"]')!, top };
}

test('the active row lies over the anchor, and the bottom row does when rows cannot fit below', async () => {
  const stride = 32;
  const night = await openNearBottom('night');
  // Night is the last row, so the rows above it fit and Night covers the anchor exactly.
  expect(parseFloat(night.popup.style.top) + 8 + 2 * stride).toBe(night.top);
  document.body.replaceChildren();
  const all = await openNearBottom('all');
  // All is the first row; with no room below, the flyout falls back to its bottom row's slot.
  expect(parseFloat(all.popup.style.top) + 8 + 2 * stride).toBe(all.top);
  expect(all.popup.querySelector('[aria-selected="true"]')?.textContent).toBe('All');
});

test('batch rows descend from N to 1 after the optional zero grid row', async () => {
  ensureCanvasOverlayStyles(document);
  const host = document.createElement('section');
  document.body.append(host);
  const app = createApp({
    setup: () => () =>
      h(CanvasBatchPicker, {
        count: 3,
        includeGrid: true,
        selection: { sourceId: 'source:1', batchIndex: 1 },
        label: 'Output image',
        send: () => undefined,
      }),
  });
  app.mount(host);
  host.querySelector('button')!.click();
  await nextTick();
  expect([...document.querySelectorAll('[role="option"]')].map((row) => row.textContent)).toEqual([
    '0',
    '3',
    '2',
    '1',
  ]);
  const activeText = document.querySelector<HTMLElement>(
    '[role="option"][aria-selected="true"] .sc-output-picker-text',
  )!;
  expect(getComputedStyle(activeText).width).toBe('auto');
  expect(getComputedStyle(activeText.parentElement!).textAlign).toBe('center');
  app.unmount();
});
