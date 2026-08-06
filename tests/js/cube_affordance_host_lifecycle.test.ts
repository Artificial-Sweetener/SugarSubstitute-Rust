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
/** Verify Cube affordances bind after Comfy creates its canvas. */

import { jest } from '@jest/globals';
import { CubeAffordanceHostLifecycle } from '../../frontend/comfyui/ui/affordance/CubeAffordanceHostLifecycle.js';
import type { CubeHostAffordanceController } from '../../frontend/comfyui/ui/affordance/CubeHostAffordanceController.js';
import type { ComfyCubeRuntime } from '../../frontend/comfyui/ui/cube/ComfyCubeRuntime.js';

test('captures the canvas during graph configuration instead of module evaluation', () => {
  let canvas: object | null = null;
  const attach = jest.fn();
  const createIntegration = jest.fn((_document: Document, capturedCanvas: unknown) => ({
    attach,
    getNodeMenuItems: jest.fn(() => [{ content: 'Save Cube', callback: jest.fn() }]),
    adaptMissingNodes: jest.fn(() => 0),
    refresh: jest.fn(),
    dispose: jest.fn(),
    capturedCanvas,
  }));
  const lifecycle = new CubeAffordanceHostLifecycle({
    getDocument: () => document,
    getCanvas: () => canvas,
    controller: {} as CubeHostAffordanceController,
    logger: console,
    createIntegration,
  });
  const runtime = {} as ComfyCubeRuntime;

  expect(lifecycle.getNodeMenuItems({})).toEqual([]);
  expect(createIntegration).not.toHaveBeenCalled();
  canvas = { selectedItems: new Set() };
  lifecycle.attach(runtime);

  expect(createIntegration).toHaveBeenCalledWith(document, canvas);
  expect(attach).toHaveBeenCalledWith(runtime);
  expect(lifecycle.getNodeMenuItems({}).map((item) => item.content)).toEqual(['Save Cube']);
});

test('fails explicitly when graph configuration precedes required host surfaces', () => {
  const lifecycle = new CubeAffordanceHostLifecycle({
    getDocument: () => null,
    getCanvas: () => null,
    controller: {} as CubeHostAffordanceController,
    logger: console,
  });

  expect(() => lifecycle.attach({} as ComfyCubeRuntime)).toThrow(
    'Comfy Cube affordance host surfaces are unavailable.',
  );
});
