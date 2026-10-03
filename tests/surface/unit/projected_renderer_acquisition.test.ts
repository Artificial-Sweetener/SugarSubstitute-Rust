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
/** Prove projected native mounts always acquire their own lifecycle-safe component boundary. */
import { afterEach, expect, jest, test } from '@jest/globals';
import type { ComfyVueRuntime } from '../../../frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.js';

const projectedComponent = { __name: 'LGraphNode' };
const loadComponent = jest.fn(async () => projectedComponent);
const runtime: ComfyVueRuntime = {
  h: () => ({}),
  render: () => undefined,
  extractVueNodeData: () => ({}),
  requestSlotLayoutSync: () => undefined,
};
jest.unstable_mockModule(
  '../../../frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.js',
  () => ({
    loadComfyNativeNodeComponent: loadComponent,
    loadComfyVueRuntime: async () => runtime,
  }),
);
const { CubeSurfaceRendererLifecycle } = await import(
  '../../../frontend/comfyui/ui/surface/CubeSurfaceRendererLifecycle.js'
);

afterEach(() => {
  document.body.replaceChildren();
  loadComponent.mockClear();
});

test('acquires the projection component even when a live graph node is already mounted', async () => {
  const root = document.createElement('div');
  root.id = 'vue-app';
  Reflect.set(root, '__vue_app__', { _context: {} });
  Reflect.set(root, '_vnode', { type: { __name: 'LGraphNode' } });
  document.body.append(root);
  const lifecycle = new CubeSurfaceRendererLifecycle({ document, logger: console });
  const renderer = await lifecycle.getRenderer();
  expect(loadComponent).toHaveBeenCalledTimes(1);
  expect(loadComponent).toHaveBeenCalledWith(document);
  expect(await lifecycle.getRenderer()).toBe(renderer);
  lifecycle.release();
  expect(await lifecycle.getRenderer()).not.toBe(renderer);
  expect(loadComponent).toHaveBeenCalledTimes(2);
  lifecycle.dispose();
});
