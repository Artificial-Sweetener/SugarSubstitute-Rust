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
/** Prove reusable wipe controls coalesce local capture and publish only qualified normalized intents. */

import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

/** Mount an independent comparison overlay without allocating images or execution listeners. */
function fixture(key = 'wipe') {
  const initial = canvasSnapshot(key);
  const port = new CanvasPortFixture({
    ...initial,
    view: { ...initial.view, comparison: { secondary: null, split: 0.5, orientation: 'vertical' } },
  });
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(root, port, jest.fn());
  const separator = root.querySelector<HTMLElement>('[role="separator"]')!;
  const captures = new Set<number>();
  Object.assign(separator, {
    setPointerCapture: (id: number) => captures.add(id),
    hasPointerCapture: (id: number) => captures.has(id),
    releasePointerCapture: (id: number) => captures.delete(id),
  });
  jest.spyOn(separator.parentElement!, 'getBoundingClientRect').mockReturnValue({
    left: 100,
    top: 50,
    width: 800,
    height: 600,
    x: 100,
    y: 50,
    right: 900,
    bottom: 650,
    toJSON: () => ({}),
  });
  return { port, root, mounted, separator, captures };
}

/** Supply local browser-shaped pointer events at the platform boundary. */
function pointer(element: HTMLElement, type: string, x: number, y: number): void {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y });
  Object.defineProperty(event, 'pointerId', { value: 1 });
  element.dispatchEvent(event);
}

describe('reusable wipe separator', () => {
  afterEach(() => {
    document.body.replaceChildren();
    jest.restoreAllMocks();
  });

  test('provides oriented keyboard movement, end stops, and host shortcut isolation', async () => {
    const f = fixture();
    const key = (value: string, options: KeyboardEventInit = {}) => {
      const event = new KeyboardEvent('keydown', {
        bubbles: true,
        cancelable: true,
        key: value,
        ...options,
      });
      f.separator.dispatchEvent(event);
      return event;
    };
    expect(key('ArrowLeft', { ctrlKey: true }).defaultPrevented).toBe(false);
    expect(key('ArrowDown').defaultPrevented).toBe(false);
    expect(key('ArrowLeft').defaultPrevented).toBe(true);
    expect(f.port.requests.at(-1)?.command).toEqual({
      type: 'wipe',
      split: 0.45,
      orientation: 'vertical',
    });
    key('Home');
    key('End');
    expect(f.port.requests.at(-1)?.command).toEqual({
      type: 'wipe',
      split: 1,
      orientation: 'vertical',
    });
    f.port.publish({
      ...f.port.read(),
      revision: 2,
      view: {
        ...f.port.read().view,
        comparison: { split: 0.5, orientation: 'horizontal', secondary: null },
      },
    });
    await nextTick();
    key('ArrowUp');
    expect(f.port.requests.at(-1)?.command).toEqual({
      type: 'wipe',
      split: 0.45,
      orientation: 'horizontal',
    });
    expect(f.separator.getAttribute('aria-orientation')).toBe('horizontal');
    expect(document.querySelectorAll('img,canvas')).toHaveLength(0);
    f.mounted.dispose();
  });

  test('coalesces a drag burst and flushes the latest clamped position on release', async () => {
    const frames: FrameRequestCallback[] = [];
    const canceled = new Set<number>();
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    jest.spyOn(window, 'cancelAnimationFrame').mockImplementation((id) => {
      canceled.add(id);
    });
    const f = fixture();
    const other = fixture('other');
    pointer(f.separator, 'pointerdown', 500, 300);
    for (let update = 0; update < 100; update++)
      pointer(f.separator, 'pointermove', 500 + update, 300);
    expect(frames).toHaveLength(1);
    expect(f.port.requests).toHaveLength(0);
    frames[0]!(16);
    expect(f.port.requests.at(-1)?.command).toEqual({
      type: 'wipe',
      orientation: 'vertical',
      split: 499 / 800,
    });
    pointer(f.separator, 'pointermove', 1000, 300);
    pointer(f.separator, 'pointerup', 1000, 300);
    await nextTick();
    expect(f.port.requests.at(-1)?.command).toEqual({
      type: 'wipe',
      orientation: 'vertical',
      split: 1,
    });
    expect(canceled.has(2)).toBe(true);
    expect(f.captures.size).toBe(0);
    expect(other.port.requests).toHaveLength(0);
    f.mounted.dispose();
    other.mounted.dispose();
  });

  test('cancels pending drag intent on orientation change or root disposal', async () => {
    const frames: FrameRequestCallback[] = [];
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    const cancel = jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {});
    const f = fixture();
    pointer(f.separator, 'pointerdown', 500, 300);
    pointer(f.separator, 'pointermove', 600, 300);
    f.port.publish({
      ...f.port.read(),
      revision: 2,
      view: {
        ...f.port.read().view,
        comparison: { split: 0.5, orientation: 'horizontal', secondary: null },
      },
    });
    await nextTick();
    expect(f.captures.size).toBe(0);
    expect(cancel).toHaveBeenCalledWith(1);
    frames[0]!(16);
    expect(f.port.requests).toHaveLength(0);
    pointer(f.separator, 'pointerdown', 500, 300);
    pointer(f.separator, 'pointermove', 500, 400);
    f.mounted.dispose();
    expect(f.captures.size).toBe(0);
    frames[1]!(32);
    expect(f.port.requests).toHaveLength(0);
  });
});
