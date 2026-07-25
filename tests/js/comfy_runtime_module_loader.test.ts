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
/** Characterize runtime asset discovery for native Nodes 2.0 embedding. */

import { describe, expect, jest, test } from '@jest/globals';

import {
  buildNativeNodeComponentModule,
  NativeRendererCompatibilityError,
  discoverComfyRuntimeAssets,
  loadComfyVueRuntime,
} from '../../frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.js';

describe('discoverComfyRuntimeAssets', () => {
  test('resolves the installed Vue and host-data chunks from Comfy entry source', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-current.js';
    document.head.append(entry);
    const fetchText = jest.fn(async () =>
      [
        'import "./vendor-vue-core-current.js";',
        'import { x } from "./dialogService-current.js";',
        'const view = () => import("./GraphView-current.js");',
      ].join('\n'),
    );

    await expect(discoverComfyRuntimeAssets(document, fetchText)).resolves.toEqual({
      entry: 'http://127.0.0.1:8188/assets/index-current.js',
      vueRuntime: 'http://127.0.0.1:8188/assets/vendor-vue-core-current.js',
      hostRuntime: 'http://127.0.0.1:8188/assets/dialogService-current.js',
      graphView: 'http://127.0.0.1:8188/assets/GraphView-current.js',
    });
  });

  test('exports the exact native component from its installed GraphView chunk', () => {
    const source = [
      'import{h as a}from"./vendor-vue-core-current.js";',
      'const load=()=>import("./lazy-current.js");',
      'var Native=a({__name:`LGraphNode`,setup(){}}),Graph=a({__name:`GraphCanvas`});',
      'export{Graph as default};',
    ].join('');

    const transformed = buildNativeNodeComponentModule(
      source,
      'http://127.0.0.1:8188/assets/GraphView-current.js',
    );

    expect(transformed).toContain('from"http://127.0.0.1:8188/assets/vendor-vue-core-current.js"');
    expect(transformed).toContain('import("http://127.0.0.1:8188/assets/lazy-current.js")');
    expect(transformed).toContain('export{Native as sugarcubesNativeNodeComponent};');
  });

  test('fails closed instead of substituting a copied node renderer', () => {
    expect(() =>
      buildNativeNodeComponentModule(
        'export const unrelated = true;',
        'http://127.0.0.1:8188/assets/GraphView-current.js',
      ),
    ).toThrow(NativeRendererCompatibilityError);
  });

  test('fails closed when the installed runtime no longer exposes the contract', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-next.js';
    document.head.append(entry);

    await expect(discoverComfyRuntimeAssets(document, async () => 'export {};')).rejects.toThrow(
      NativeRendererCompatibilityError,
    );
  });

  test('exposes Comfy native slot remeasurement with the other Vue runtime capabilities', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-current.js';
    document.head.append(entry);
    const requestSlotLayoutSync = jest.fn();
    const importModule = jest.fn(async (url: string) => {
      if (url.endsWith('vendor-vue-core-current.js')) {
        return {
          render: function render(): void {},
          h: function h(): object {
            return {};
          },
        };
      }
      return {
        extractVueNodeData: function extractVueNodeData(): object {
          return {};
        },
        requestSlotLayoutSyncForAllNodes: function requestSlotLayoutSyncForAllNodes(): void {
          requestSlotLayoutSync();
        },
      };
    });

    const runtime = await loadComfyVueRuntime(
      document,
      async () =>
        [
          'import "./vendor-vue-core-current.js";',
          'import { x } from "./dialogService-current.js";',
          'const view = () => import("./GraphView-current.js");',
        ].join('\n'),
      importModule,
    );
    runtime.requestSlotLayoutSync();

    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(1);
  });
});
