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
/** Prove bounded native mount settlement and release on cancellation or replacement. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { SurfacePresentationBoundary } from '../../../frontend/comfyui/ui/surface/motion/SurfacePresentationBoundary.js';

afterEach(() => {
  jest.restoreAllMocks();
});

/** Expose frame scheduling without advancing unrelated browser or animation timers. */
function fixture() {
  const callbacks: FrameRequestCallback[] = [];
  const request = jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    callbacks.push(callback);
    return callbacks.length;
  });
  const cancel = jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  return { boundary: new SurfacePresentationBoundary(document), callbacks, request, cancel };
}

test('yields exactly the styling and resize frames before endpoint measurement', async () => {
  const setup = fixture();
  const settled = jest.fn();
  const pending = setup.boundary.settled().then(settled);
  setup.callbacks[0]?.(0);
  await Promise.resolve();
  expect(settled).not.toHaveBeenCalled();
  setup.callbacks[1]?.(16);
  await pending;
  expect(settled).toHaveBeenCalledTimes(1);
  expect(setup.request).toHaveBeenCalledTimes(2);
  setup.boundary.cancel();
  expect(setup.cancel).not.toHaveBeenCalled();
});

test('resolves retired callers and prevents stale callbacks from taking over the new request', async () => {
  const setup = fixture();
  const obsolete = setup.boundary.settled();
  const current = setup.boundary.settled();
  await obsolete;
  expect(setup.cancel).toHaveBeenCalledWith(1);
  setup.callbacks[0]?.(0);
  expect(setup.request).toHaveBeenCalledTimes(2);
  setup.callbacks[1]?.(16);
  expect(setup.request).toHaveBeenCalledTimes(3);
  setup.boundary.cancel();
  await current;
  expect(setup.cancel).toHaveBeenCalledWith(3);
  setup.callbacks[2]?.(32);
  expect(setup.request).toHaveBeenCalledTimes(3);
});

test('settles explicitly prepared styling at the next frame without a second frame delay', async () => {
  const setup = fixture();
  const pending = setup.boundary.settled(1);
  setup.callbacks[0]?.(0);
  await pending;
  expect(setup.request).toHaveBeenCalledTimes(1);
  setup.boundary.cancel();
  expect(setup.cancel).not.toHaveBeenCalled();
});
