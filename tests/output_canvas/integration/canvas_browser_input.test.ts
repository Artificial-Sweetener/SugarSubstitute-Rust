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
/** Prove local image gestures preserve host shortcuts and release capture on interruption. */

import { jest } from '@jest/globals';
import { BrowserCanvasInput } from '../../../frontend/comfyui/ui/output_canvas/presentation/browser/BrowserCanvasInput.js';
import type { CanvasAnchor } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCamera.js';

/** Model browser pointer capture, the sole unavailable jsdom input boundary. */
function fixture(outbound?: (event: MouseEvent) => boolean) {
  const element = document.createElement('div');
  document.body.append(element);
  const captures = new Set<number>();
  Object.assign(element, {
    setPointerCapture: (id: number) => captures.add(id),
    hasPointerCapture: (id: number) => captures.has(id),
    releasePointerCapture: (id: number) => captures.delete(id),
  });
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue({
    left: 10,
    top: 20,
    width: 500,
    height: 400,
    right: 510,
    bottom: 420,
    x: 10,
    y: 20,
    toJSON: () => ({}),
  });
  const gestures = {
    contains: (point: CanvasAnchor) =>
      point.x >= 50 && point.x <= 450 && point.y >= 50 && point.y <= 350,
    interrupt: jest.fn<() => void>(),
    pan: jest.fn<(delta: CanvasAnchor) => void>(),
    zoom: jest.fn<(factor: number) => void>(),
    wheel: jest.fn<(delta: number, units: 0 | 1 | 2, anchor: CanvasAnchor) => void>(),
    pinch: jest.fn<(factor: number, anchor: CanvasAnchor, delta: CanvasAnchor) => void>(),
    doubleClick: jest.fn<(anchor: CanvasAnchor) => void>(),
    fit: jest.fn<() => void>(),
    ...(outbound ? { outbound } : {}),
  };
  const input = new BrowserCanvasInput(element, gestures);
  return { element, captures, gestures, input };
}

/** Send browser-shaped pointers without relying on an absent jsdom PointerEvent constructor. */
function pointer(
  element: HTMLElement,
  type: string,
  id: number,
  x: number,
  y: number,
  touch = false,
): Event {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    clientX: x + 10,
    clientY: y + 20,
  });
  Object.defineProperties(event, {
    pointerId: { value: id },
    pointerType: { value: touch ? 'touch' : 'mouse' },
  });
  element.dispatchEvent(event);
  return event;
}

/** Exercise normal DOM keyboard bubbling and browser cancellation. */
function key(
  element: HTMLElement,
  value: string,
  modifiers: KeyboardEventInit = {},
): KeyboardEvent {
  const event = new KeyboardEvent('keydown', {
    bubbles: true,
    cancelable: true,
    key: value,
    ...modifiers,
  });
  element.dispatchEvent(event);
  return event;
}

describe('browser image input ownership', () => {
  afterEach(() => {
    document.body.replaceChildren();
    jest.restoreAllMocks();
  });

  test('leaves fitted mouse drags to the transfer owner while touch still pans', () => {
    const f = fixture(() => true);
    expect(pointer(f.element, 'pointerdown', 1, 100, 100).defaultPrevented).toBe(false);
    pointer(f.element, 'pointermove', 1, 120, 110);
    expect(f.captures.size).toBe(0);
    expect(f.gestures.pan).not.toHaveBeenCalled();
    expect(pointer(f.element, 'dragstart', 1, 100, 100).defaultPrevented).toBe(false);
    expect(pointer(f.element, 'pointerdown', 2, 100, 100, true).defaultPrevented).toBe(true);
    pointer(f.element, 'pointermove', 2, 120, 110, true);
    expect(f.gestures.pan).toHaveBeenCalledWith({ x: 20, y: 10 });
    f.input.dispose();
  });

  test('requires image engagement for wheel zoom and preserves modified browser gestures', () => {
    const f = fixture();
    const wheel = (options: WheelEventInit = {}) => {
      const event = new WheelEvent('wheel', {
        bubbles: true,
        cancelable: true,
        clientX: 210,
        clientY: 220,
        deltaY: 0.5,
        ...options,
      });
      f.element.dispatchEvent(event);
      return event;
    };
    expect(wheel().defaultPrevented).toBe(false);
    f.element.focus();
    expect(wheel().defaultPrevented).toBe(true);
    expect(f.gestures.wheel).toHaveBeenLastCalledWith(0.5, 0, { x: 200, y: 200 });
    expect(wheel({ deltaMode: 1 }).defaultPrevented).toBe(true);
    expect(wheel({ ctrlKey: true }).defaultPrevented).toBe(false);
    expect(wheel({ clientX: 15 }).defaultPrevented).toBe(false);
    key(f.element, 'Escape');
    expect(wheel().defaultPrevented).toBe(false);
    f.input.dispose();
    f.element.focus();
    expect(wheel().defaultPrevented).toBe(false);
  });

  test('captures only image presses and pans continuously without work after cancellation', () => {
    const f = fixture();
    expect(pointer(f.element, 'pointerdown', 1, 5, 5).defaultPrevented).toBe(false);
    expect(f.captures.size).toBe(0);
    expect(pointer(f.element, 'pointerdown', 1, 100, 100).defaultPrevented).toBe(true);
    expect(f.captures.has(1)).toBe(true);
    pointer(f.element, 'pointermove', 1, 120, 110);
    expect(f.gestures.pan).toHaveBeenLastCalledWith({ x: 20, y: 10 });
    pointer(f.element, 'pointercancel', 1, 120, 110);
    pointer(f.element, 'pointermove', 1, 130, 120);
    expect(f.gestures.pan).toHaveBeenCalledTimes(1);
    expect(f.captures.size).toBe(0);
    pointer(f.element, 'pointerdown', 2, 100, 100);
    f.input.dispose();
    expect(f.captures.size).toBe(0);
    expect(f.element.classList.contains('sc-output-dragging')).toBe(false);
  });

  test('rebases two-pointer pinch and the surviving finger without a transition jump', () => {
    const f = fixture();
    pointer(f.element, 'pointerdown', 1, 100, 100, true);
    pointer(f.element, 'pointerdown', 2, 200, 100, true);
    expect(f.gestures.pan).not.toHaveBeenCalled();
    expect(f.gestures.pinch).not.toHaveBeenCalled();
    pointer(f.element, 'pointermove', 2, 220, 100, true);
    expect(f.gestures.pinch).toHaveBeenLastCalledWith(1.2, { x: 150, y: 100 }, { x: 10, y: 0 });
    pointer(f.element, 'pointerup', 2, 220, 100, true);
    pointer(f.element, 'pointermove', 1, 105, 110, true);
    expect(f.gestures.pan).toHaveBeenLastCalledWith({ x: 5, y: 10 });
    f.input.dispose();
  });

  test('scopes keyboard controls to viewport focus and preserves forms and host shortcuts', () => {
    const f = fixture();
    const field = document.createElement('input');
    f.element.append(field);
    expect(key(field, 'ArrowLeft').defaultPrevented).toBe(false);
    expect(key(f.element, 'ArrowLeft', { ctrlKey: true }).defaultPrevented).toBe(false);
    expect(key(f.element, 'Enter').defaultPrevented).toBe(false);
    expect(key(f.element, 'ArrowLeft').defaultPrevented).toBe(true);
    expect(f.gestures.pan).toHaveBeenLastCalledWith({ x: 40, y: 0 });
    key(f.element, '+', { shiftKey: true });
    expect(f.gestures.zoom).toHaveBeenLastCalledWith(1.25);
    key(f.element, '0');
    expect(f.gestures.fit).toHaveBeenCalledTimes(1);
    f.input.dispose();
    expect(key(f.element, 'ArrowLeft').defaultPrevented).toBe(false);
  });

  test('routes double click/tap only on image pixels and suppresses browser dragging', () => {
    const f = fixture();
    f.element.dispatchEvent(
      new MouseEvent('dblclick', { bubbles: true, clientX: 210, clientY: 220 }),
    );
    expect(f.gestures.doubleClick).toHaveBeenLastCalledWith({ x: 200, y: 200 });
    for (let tap = 0; tap < 2; tap++) {
      pointer(f.element, 'pointerdown', 1, 100, 100, true);
      pointer(f.element, 'pointerup', 1, 100, 100, true);
    }
    expect(f.gestures.doubleClick).toHaveBeenCalledTimes(2);
    const drag = new Event('dragstart', { bubbles: true, cancelable: true });
    f.element.dispatchEvent(drag);
    expect(drag.defaultPrevented).toBe(true);
    f.input.dispose();
  });
});
