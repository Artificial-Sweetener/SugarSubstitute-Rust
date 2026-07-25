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
/** Verify native-card measurements trigger finite coalesced masonry reflows. */

import { jest } from '@jest/globals';

import { NativeCardGeometryObserver } from '../../frontend/comfyui/ui/surface/NativeCardGeometryObserver.js';

describe('NativeCardGeometryObserver', () => {
  test('observes exact cells and coalesces native resize bursts', () => {
    const observe = jest.fn();
    const disconnect = jest.fn();
    let notify: ResizeObserverCallback = () => undefined;
    class TestResizeObserver {
      constructor(callback: ResizeObserverCallback) {
        notify = callback;
      }

      observe = observe;
      unobserve = jest.fn();
      disconnect = disconnect;
    }
    const callbacks = new Map<number, FrameRequestCallback>();
    let nextFrame = 1;
    const requestAnimationFrame = jest
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((callback) => {
        const id = nextFrame++;
        callbacks.set(id, callback);
        return id;
      });
    const cancelAnimationFrame = jest
      .spyOn(window, 'cancelAnimationFrame')
      .mockImplementation((id) => {
        callbacks.delete(id);
      });
    const originalResizeObserver = window.ResizeObserver;
    window.ResizeObserver = TestResizeObserver as unknown as typeof ResizeObserver;
    const onGeometryChange = jest.fn();
    const observer = new NativeCardGeometryObserver(document, onGeometryChange);
    const first = document.createElement('div');
    const second = document.createElement('div');

    observer.observe([first, second]);
    notify([], observer as unknown as ResizeObserver);
    notify([], observer as unknown as ResizeObserver);
    expect(observe.mock.calls.map(([cell]) => cell)).toEqual([first, second]);
    expect(callbacks).toHaveProperty('size', 1);

    const pending = [...callbacks.values()][0];
    pending?.(0);
    expect(onGeometryChange).toHaveBeenCalledTimes(1);

    observer.dispose();
    expect(disconnect).toHaveBeenCalled();
    window.ResizeObserver = originalResizeObserver;
    requestAnimationFrame.mockRestore();
    cancelAnimationFrame.mockRestore();
  });
});
