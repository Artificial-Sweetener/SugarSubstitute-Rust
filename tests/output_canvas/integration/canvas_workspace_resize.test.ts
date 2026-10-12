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
/** Prove local divider capture, bounded geometry writes, keyboard clamps, and interrupted resize cleanup. */

import { jest } from '@jest/globals';
import { CanvasWorkspaceResizeInput } from '../../../frontend/comfyui/ui/output_canvas/presentation/browser/CanvasWorkspaceResizeInput.js';
import { CanvasWorkspaceLayout } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasWorkspaceLayout.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

/** Compose the real allocation owner while supplying only missing platform pointer-capture methods. */
function fixture() {
  const model = new CanvasWorkspaceLayout();
  model.bounds(1400, 800);
  const frames = new CanvasFrameFixture();
  const divider = document.createElement('div');
  const captures = new Set<number>();
  Object.assign(divider, {
    setPointerCapture: (id: number) => captures.add(id),
    hasPointerCapture: (id: number) => captures.has(id),
    releasePointerCapture: (id: number) => captures.delete(id),
  });
  const commit = jest.fn<(width: number) => void>();
  const preview = jest.fn((width: number) => {
    model.resize(width);
  });
  const cancel = jest.fn(() => model.cancelResize());
  const input = new CanvasWorkspaceResizeInput(divider, frames, {
    width: () => model.read().canvas.width,
    preview,
    commit,
    cancel,
  });
  return { model, frames, divider, captures, input, preview, commit, cancel };
}

/** Send browser-shaped pointer coordinates without replacing application or gesture policy. */
function pointer(element: HTMLElement, type: string, x: number, id = 1): void {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x });
  Object.defineProperty(event, 'pointerId', { value: id });
  element.dispatchEvent(event);
}

test('coalesces a captured resize and commits its final release coordinate exactly once', () => {
  const f = fixture();
  pointer(f.divider, 'pointerdown', 840);
  for (let step = 0; step < 100; step++) pointer(f.divider, 'pointermove', 840 - step);
  expect(f.frames.pending).toBe(1);
  expect(f.preview).not.toHaveBeenCalled();
  f.frames.advance(16);
  expect(f.preview).toHaveBeenCalledTimes(1);
  expect(f.model.read().canvas.width).toBe(659);
  pointer(f.divider, 'pointerup', 680);
  expect(f.model.read().canvas.width).toBe(720);
  expect(f.commit.mock.calls).toEqual([[720]]);
  expect(f.captures.size).toBe(0);
  expect(f.frames.pending).toBe(0);
  f.input.dispose();
});

test('cancels capture and restores accepted width without allowing queued work after retirement', () => {
  const f = fixture();
  pointer(f.divider, 'pointerdown', 840);
  pointer(f.divider, 'pointermove', 600);
  f.frames.advance(16);
  expect(f.model.read().canvas.width).toBe(800);
  pointer(f.divider, 'pointercancel', 600);
  expect(f.model.read().canvas.width).toBe(560);
  expect(f.commit).not.toHaveBeenCalled();
  pointer(f.divider, 'pointerdown', 840);
  pointer(f.divider, 'pointermove', 500);
  f.input.dispose();
  f.frames.advance(16);
  expect(f.captures.size).toBe(0);
  expect(f.frames.pending).toBe(0);
  expect(f.model.read().canvas.width).toBe(560);
  expect(f.commit).not.toHaveBeenCalled();
});

test('clamps keyboard resizing through the model and preserves modified host shortcuts', () => {
  const f = fixture();
  const key = (key: string, options: KeyboardEventInit = {}) => {
    const event = new KeyboardEvent('keydown', {
      key,
      bubbles: true,
      cancelable: true,
      ...options,
    });
    f.divider.dispatchEvent(event);
    return event;
  };
  expect(key('ArrowRight', { ctrlKey: true }).defaultPrevented).toBe(false);
  expect(f.commit).not.toHaveBeenCalled();
  key('Home');
  expect(f.commit).toHaveBeenLastCalledWith(320);
  key('End');
  expect(f.commit).toHaveBeenLastCalledWith(1040);
  key('ArrowRight');
  expect(f.commit).toHaveBeenLastCalledWith(1020);
  pointer(f.divider, 'pointerdown', 380);
  pointer(f.divider, 'pointermove', 100);
  key('Escape');
  expect(f.captures.size).toBe(0);
  expect(f.model.read().canvas.width).toBe(560);
  f.input.dispose();
});
