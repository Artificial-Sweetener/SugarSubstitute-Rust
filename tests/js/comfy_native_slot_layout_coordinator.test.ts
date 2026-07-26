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
/** Verify native slot synchronization is bounded to one call per presentation frame. */

import { jest } from '@jest/globals';
import type { ComfyVueRuntime } from '../../frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.js';
import { ComfyNativeSlotLayoutCoordinator } from '../../frontend/comfyui/ui/surface/ComfyNativeSlotLayoutCoordinator.js';

describe('ComfyNativeSlotLayoutCoordinator', () => {
  test('coalesces every boundary request in one frame', async () => {
    const callbacks: FrameRequestCallback[] = [];
    const requestSlotLayoutSync = jest.fn();
    const coordinator = new ComfyNativeSlotLayoutCoordinator({
      getRuntime: async () => ({ requestSlotLayoutSync }) as unknown as ComfyVueRuntime,
      requestFrame: (callback) => {
        callbacks.push(callback);
        return callbacks.length;
      },
      cancelFrame: jest.fn(),
      logger: console,
    });

    coordinator.request();
    coordinator.request();
    coordinator.request();

    expect(callbacks).toHaveLength(1);
    callbacks[0]?.(0);
    await Promise.resolve();
    await Promise.resolve();

    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(1);
    coordinator.dispose();
  });

  test('cancels a pending frame when presentation is disposed', () => {
    const cancelFrame = jest.fn();
    const coordinator = new ComfyNativeSlotLayoutCoordinator({
      getRuntime: async () => ({}) as ComfyVueRuntime,
      requestFrame: () => 42,
      cancelFrame,
      logger: console,
    });

    coordinator.request();
    coordinator.dispose();

    expect(cancelFrame).toHaveBeenCalledWith(42);
  });
});
