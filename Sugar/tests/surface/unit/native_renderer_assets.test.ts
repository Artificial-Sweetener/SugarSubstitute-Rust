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
/** Prove immutable Comfy component assets are shared while failed acquisitions remain retryable. */
import { jest } from '@jest/globals';
import { ComfyNativeNodeAssets } from '../../../frontend/comfyui/ui/surface/ComfyNativeNodeAssets.js';
import type { ComfyVueRuntime } from '../../../frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.js';

test('coalesces loads and retains only immutable component/runtime capabilities across renderer lifetimes', async () => {
  const component = { name: 'native' };
  const runtime: ComfyVueRuntime = {
    render() {},
    h() {
      return {};
    },
    extractVueNodeData() {
      return {};
    },
    requestSlotLayoutSync() {},
  };
  const loadComponent = jest.fn(async () => component);
  const loadRuntime = jest.fn(async () => runtime);
  const assets = new ComfyNativeNodeAssets(loadComponent, loadRuntime);
  for (let index = 0; index < 100; index++) {
    expect(await assets.component()).toBe(component);
    expect(await assets.runtime()).toBe(runtime);
  }
  expect(loadComponent).toHaveBeenCalledTimes(1);
  expect(loadRuntime).toHaveBeenCalledTimes(1);
});

test('shares concurrent acquisitions and drops failures independently of successful assets', async () => {
  const loadComponent = jest.fn(async () => ({ name: 'native' }));
  const loadRuntime = jest
    .fn<() => Promise<ComfyVueRuntime>>()
    .mockRejectedValueOnce(new Error('Transport failure'))
    .mockResolvedValue({
      render() {},
      h() {
        return {};
      },
      extractVueNodeData() {
        return {};
      },
      requestSlotLayoutSync() {},
    });
  const assets = new ComfyNativeNodeAssets(loadComponent, loadRuntime);
  const first = assets.runtime();
  expect(assets.runtime()).toBe(first);
  await expect(first).rejects.toThrow('Transport failure');
  await assets.component();
  await assets.runtime();
  expect(loadComponent).toHaveBeenCalledTimes(1);
  expect(loadRuntime).toHaveBeenCalledTimes(2);
});
