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
import { CanvasViewportKeyboard } from '../../../frontend/comfyui/ui/output_canvas/presentation/CanvasViewportKeyboard.js';
import { CanvasSurfacePort } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfacePort.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';

/** Model browser pointer capture, the sole unavailable jsdom input boundary. */
function fixture(outbound?: (event: MouseEvent) => boolean) {
  let navigable = true;
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
    navigable: () => navigable,
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
  const port = new CanvasSurfacePort(canvasSnapshot('keyboard'), {
    origin: document.location.origin,
    capabilities: { schema: 1, commands: ['fit', 'native', 'zoom', 'pan'] },
    accepts: () => true,
    apply: (command) => {
      if (command.type === 'fit') gestures.fit();
      if (command.type === 'zoom') gestures.zoom(command.factor);
      if (command.type === 'pan') gestures.pan({ x: command.x * 500, y: command.y * 400 });
      return true;
    },
    geometry: () => {},
    release: () => {},
    report: () => {},
  });
  const keyboard = new CanvasViewportKeyboard(
    element,
    port,
    () => {},
    () => input.cancel(),
  );
  return {
    element,
    captures,
    gestures,
    navigable: (enabled: boolean) => {
      navigable = enabled;
    },
    input: {
      dispose: () => {
        keyboard.dispose();
        input.dispose();
        port.dispose();
      },
    },
  };
}

/** Send browser-shaped pointers without relying on an absent jsdom PointerEvent constructor. */
function pointer(
  element: HTMLElement,
  type: string,
  id: number,
  x: number,
  y: number,
  touch = false,
  modifiers: MouseEventInit = {},
): Event {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    clientX: x + 10,
    clientY: y + 20,
    ...modifiers,
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

  test('refuses preview wheel, drag, pinch and double activation and releases an interrupted final drag', () => {
    const outbound = jest.fn<() => boolean>().mockReturnValue(false);
    const f = fixture(outbound);
    pointer(f.element, 'pointerdown', 1, 100, 100);
    expect(f.captures.size).toBe(1);
    outbound.mockClear();
    f.navigable(false);
    const escapedMove = jest.fn();
    document.body.addEventListener('pointermove', escapedMove);
    expect(pointer(f.element, 'pointermove', 1, 120, 110).defaultPrevented).toBe(true);
    expect(escapedMove).not.toHaveBeenCalled();
    document.body.removeEventListener('pointermove', escapedMove);
    expect(f.captures.size).toBe(0);
    const escaped = jest.fn();
    for (const event of ['pointerdown', 'wheel', 'dblclick'])
      document.body.addEventListener(event, escaped);
    pointer(f.element, 'pointerdown', 4, 100, 100);
    const wheel = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: 210,
      clientY: 220,
      deltaY: -10,
    });
    f.element.dispatchEvent(wheel);
    expect(wheel.defaultPrevented).toBe(true);
    key(f.element, 'Escape');
    const stillFocused = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: 210,
      clientY: 220,
      deltaY: -10,
    });
    f.element.dispatchEvent(stillFocused);
    expect(stillFocused.defaultPrevented).toBe(true);
    for (let tap = 0; tap < 2; tap++) {
      pointer(f.element, 'pointerdown', 2, 100, 100, true);
      pointer(f.element, 'pointerdown', 3, 200, 100, true);
      pointer(f.element, 'pointermove', 3, 240, 110, true);
      pointer(f.element, 'pointerup', 3, 240, 110, true);
      pointer(f.element, 'pointerup', 2, 100, 100, true);
    }
    f.element.dispatchEvent(
      new MouseEvent('dblclick', { bubbles: true, cancelable: true, clientX: 210, clientY: 220 }),
    );
    expect(pointer(f.element, 'dragstart', 4, 100, 100).defaultPrevented).toBe(true);
    expect(f.captures.size).toBe(0);
    expect(document.activeElement).toBe(f.element);
    expect(escaped).not.toHaveBeenCalled();
    for (const event of ['pointerdown', 'wheel', 'dblclick'])
      document.body.removeEventListener(event, escaped);
    expect(f.gestures.pan).not.toHaveBeenCalled();
    expect(f.gestures.pinch).not.toHaveBeenCalled();
    expect(f.gestures.wheel).not.toHaveBeenCalled();
    expect(f.gestures.doubleClick).not.toHaveBeenCalled();
    expect(outbound).not.toHaveBeenCalled();
    f.navigable(true);
    pointer(f.element, 'pointerdown', 5, 100, 100);
    pointer(f.element, 'pointermove', 5, 120, 110);
    expect(f.gestures.pan).toHaveBeenCalledWith({ x: 20, y: 10 });
    f.input.dispose();
  });

  test('consumes modified primary preview presses without changing modified final presses', () => {
    const f = fixture();
    const escaped = jest.fn();
    document.body.addEventListener('pointerdown', escaped);
    const modifiers = [{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { shiftKey: true }];
    for (const modified of modifiers)
      expect(pointer(f.element, 'pointerdown', 1, 100, 100, false, modified).defaultPrevented).toBe(
        false,
      );
    expect(escaped).toHaveBeenCalledTimes(4);
    escaped.mockClear();
    f.navigable(false);
    for (const modified of modifiers)
      expect(pointer(f.element, 'pointerdown', 1, 100, 100, false, modified).defaultPrevented).toBe(
        true,
      );
    expect(escaped).not.toHaveBeenCalled();
    expect(f.captures.size).toBe(0);
    expect(f.gestures.pan).not.toHaveBeenCalled();
    document.body.removeEventListener('pointerdown', escaped);
    f.input.dispose();
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

  test('preserves all modified navigation keys and the existing discrete pan and zoom steps', () => {
    const f = fixture();
    for (const modifiers of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }]) {
      for (const value of ['ArrowLeft', '+', '-', '0'])
        expect(key(f.element, value, modifiers).defaultPrevented).toBe(false);
    }
    expect(key(f.element, 'ArrowLeft', { shiftKey: true }).defaultPrevented).toBe(false);
    expect(key(f.element, '0', { shiftKey: true }).defaultPrevented).toBe(false);
    for (const [value, delta] of [
      ['ArrowRight', { x: -40, y: 0 }],
      ['ArrowUp', { x: 0, y: 40 }],
      ['ArrowDown', { x: 0, y: -40 }],
    ] as const) {
      expect(key(f.element, value).defaultPrevented).toBe(true);
      expect(f.gestures.pan).toHaveBeenLastCalledWith(delta);
    }
    key(f.element, '-');
    expect(f.gestures.zoom).toHaveBeenLastCalledWith(0.8);
    key(f.element, '=');
    expect(f.gestures.zoom).toHaveBeenLastCalledWith(1.25);
    f.input.dispose();
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
