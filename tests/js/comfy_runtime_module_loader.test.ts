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
  buildPrivateNodeDataModule,
  NativeRendererCompatibilityError,
  discoverComfyNodes2RuntimeAssets,
  discoverComfySettingsRuntimeAssets,
  loadComfySettingsAutoCompleteComponent,
  loadComfySettingsSelectComponent,
  loadComfyVueRenderRuntime,
  loadComfyVueRuntime,
} from '../../frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.js';

describe('Comfy runtime module discovery', () => {
  test('resolves the legacy direct-entry Nodes 2 runtime layout', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-current.js';
    document.head.append(entry);
    const sources = new Map([
      [
        entry.src,
        [
          'import "./vendor-vue-core-current.js";',
          'import { x } from "./dialogService-current.js";',
          'const view = () => import("./GraphView-current.js");',
        ].join('\n'),
      ],
      ['http://127.0.0.1:8188/assets/vendor-vue-core-current.js', 'export const vue = true;'],
      [
        'http://127.0.0.1:8188/assets/dialogService-current.js',
        'export{extractVueNodeData,requestSlotLayoutSyncForAllNodes};',
      ],
      [
        'http://127.0.0.1:8188/assets/GraphView-current.js',
        'var Native=defineComponent({__name:`LGraphNode`,setup(){}});',
      ],
    ]);
    const fetchText = moduleFetcher(sources);

    await expect(discoverComfyNodes2RuntimeAssets(document, fetchText)).resolves.toEqual({
      entry: 'http://127.0.0.1:8188/assets/index-current.js',
      vueRuntime: 'http://127.0.0.1:8188/assets/vendor-vue-core-current.js',
      nodeDataRuntime: 'http://127.0.0.1:8188/assets/dialogService-current.js',
      slotLayoutRuntime: 'http://127.0.0.1:8188/assets/dialogService-current.js',
      graphView: 'http://127.0.0.1:8188/assets/GraphView-current.js',
    });
  });

  test('resolves the current entry-to-main Nodes 2 runtime layout by capability', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-next.js';
    document.head.append(entry);
    const sources = new Map([
      [
        entry.src,
        [
          'const deps=["./main-next.js","./settingStore-next.js","./vendor-vue-core-next.js"];',
          'await import("./main-next.js");',
        ].join('\n'),
      ],
      [
        'http://127.0.0.1:8188/assets/main-next.js',
        'const graph = () => import("./GraphView-next.js");',
      ],
      [
        'http://127.0.0.1:8188/assets/settingStore-next.js',
        'export{internalA as extractVueNodeData,internalB as requestSlotLayoutSyncForAllNodes};',
      ],
      ['http://127.0.0.1:8188/assets/vendor-vue-core-next.js', 'export const vue = true;'],
      [
        'http://127.0.0.1:8188/assets/GraphView-next.js',
        'const Native=defineComponent({__name:"LGraphNode",setup(){}});',
      ],
    ]);

    await expect(
      discoverComfyNodes2RuntimeAssets(document, moduleFetcher(sources)),
    ).resolves.toEqual({
      entry: 'http://127.0.0.1:8188/assets/index-next.js',
      vueRuntime: 'http://127.0.0.1:8188/assets/vendor-vue-core-next.js',
      nodeDataRuntime: 'http://127.0.0.1:8188/assets/settingStore-next.js',
      slotLayoutRuntime: 'http://127.0.0.1:8188/assets/settingStore-next.js',
      graphView: 'http://127.0.0.1:8188/assets/GraphView-next.js',
    });
  });

  test('prioritizes semantic modules when the entry advertises more chunks than the fetch bound', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-wide.js';
    document.head.append(entry);
    const decoyReferences = Array.from(
      { length: 160 },
      (_value, index) => `"./feature-${index}.js"`,
    );
    const sources = new Map([
      [
        entry.src,
        `const deps=[${[
          ...decoyReferences,
          '"./main-wide.js"',
          '"./settingStore-wide.js"',
          '"./vendor-vue-core-wide.js"',
        ].join(',')}];`,
      ],
      ['http://127.0.0.1:8188/assets/main-wide.js', 'import("./GraphView-wide.js");'],
      [
        'http://127.0.0.1:8188/assets/settingStore-wide.js',
        'export{extractVueNodeData,requestSlotLayoutSyncForAllNodes};',
      ],
      ['http://127.0.0.1:8188/assets/vendor-vue-core-wide.js', 'export const vue = true;'],
      [
        'http://127.0.0.1:8188/assets/GraphView-wide.js',
        'const Native=defineComponent({__name:"LGraphNode",setup(){}});',
      ],
    ]);

    await expect(
      discoverComfyNodes2RuntimeAssets(document, moduleFetcher(sources)),
    ).resolves.toEqual({
      entry: entry.src,
      vueRuntime: 'http://127.0.0.1:8188/assets/vendor-vue-core-wide.js',
      nodeDataRuntime: 'http://127.0.0.1:8188/assets/settingStore-wide.js',
      slotLayoutRuntime: 'http://127.0.0.1:8188/assets/settingStore-wide.js',
      graphView: 'http://127.0.0.1:8188/assets/GraphView-wide.js',
    });
  });

  test('discovers Settings assets without requiring Nodes 2 renderer modules', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-settings.js';
    document.head.append(entry);
    const sources = new Map([
      [entry.src, 'const deps=["./vendor-vue-core-settings.js","./vendor-primevue-settings.js"];'],
      ['http://127.0.0.1:8188/assets/vendor-vue-core-settings.js', 'export const vue = true;'],
      ['http://127.0.0.1:8188/assets/vendor-primevue-settings.js', 'export const prime = true;'],
    ]);

    await expect(
      discoverComfySettingsRuntimeAssets(document, moduleFetcher(sources)),
    ).resolves.toEqual({
      entry: 'http://127.0.0.1:8188/assets/index-settings.js',
      vueRuntime: 'http://127.0.0.1:8188/assets/vendor-vue-core-settings.js',
      primeVueRuntime: 'http://127.0.0.1:8188/assets/vendor-primevue-settings.js',
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

  test('exports retained private node data from the installed floor GraphView chunk', () => {
    const transformed = buildPrivateNodeDataModule(
      'import{h as a}from"./vendor-vue-core-floor.js";function extractVueNodeData(e){return{id:e.id}}',
      'http://127.0.0.1:8188/assets/GraphView-floor.js',
    );

    expect(transformed).toContain('from"http://127.0.0.1:8188/assets/vendor-vue-core-floor.js"');
    expect(transformed).toContain('export{extractVueNodeData as sugarcubesExtractVueNodeData};');
  });

  test('discovers floor renderer capabilities without a global slot-layout coordinator', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-floor.js';
    document.head.append(entry);
    const sources = new Map([
      [entry.src, 'const deps=["./vendor-vue-core-floor.js","./GraphView-floor.js"];'],
      ['http://127.0.0.1:8188/assets/vendor-vue-core-floor.js', 'export const vue = true;'],
      [
        'http://127.0.0.1:8188/assets/GraphView-floor.js',
        'function extractVueNodeData(e){return e}const Native=defineComponent({__name:`LGraphNode`,setup(){}});',
      ],
    ]);

    await expect(
      discoverComfyNodes2RuntimeAssets(document, moduleFetcher(sources)),
    ).resolves.toEqual({
      entry: entry.src,
      vueRuntime: 'http://127.0.0.1:8188/assets/vendor-vue-core-floor.js',
      nodeDataRuntime: 'http://127.0.0.1:8188/assets/GraphView-floor.js',
      slotLayoutRuntime: null,
      graphView: 'http://127.0.0.1:8188/assets/GraphView-floor.js',
    });
  });

  test('fails closed instead of substituting a copied node renderer', () => {
    expect(() =>
      buildNativeNodeComponentModule(
        'export const unrelated = true;',
        'http://127.0.0.1:8188/assets/GraphView-current.js',
      ),
    ).toThrow(NativeRendererCompatibilityError);
  });

  test('loads the same PrimeVue Select and AutoComplete component family used by Settings', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-current.js';
    document.head.append(entry);
    const fetchText = moduleFetcher(
      new Map([
        [entry.src, 'const deps=["./vendor-vue-core-current.js","./vendor-primevue-current.js"];'],
        ['http://127.0.0.1:8188/assets/vendor-vue-core-current.js', 'export const vue = true;'],
        ['http://127.0.0.1:8188/assets/vendor-primevue-current.js', 'export const prime = true;'],
      ]),
    );
    const select = { name: 'Select' };
    const autoComplete = { name: 'AutoComplete' };
    const importModule = jest.fn(async () => ({
      unrelated: { name: 'Button' },
      select,
      autoComplete,
    }));

    await expect(loadComfySettingsSelectComponent(document, fetchText, importModule)).resolves.toBe(
      select,
    );
    await expect(
      loadComfySettingsAutoCompleteComponent(document, fetchText, importModule),
    ).resolves.toBe(autoComplete);
    expect(importModule).toHaveBeenCalledWith(
      'http://127.0.0.1:8188/assets/vendor-primevue-current.js',
    );
  });

  test('fails closed instead of substituting a hand-built Settings control', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-current.js';
    document.head.append(entry);
    const fetchText = moduleFetcher(
      new Map([
        [entry.src, 'const deps=["./vendor-vue-core-current.js","./vendor-primevue-current.js"];'],
        ['http://127.0.0.1:8188/assets/vendor-vue-core-current.js', 'export const vue = true;'],
        ['http://127.0.0.1:8188/assets/vendor-primevue-current.js', 'export const prime = true;'],
      ]),
    );

    await expect(
      loadComfySettingsSelectComponent(document, fetchText, async () => ({})),
    ).rejects.toThrow(NativeRendererCompatibilityError);
  });

  test('fails closed when the installed runtime no longer exposes the contract', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-next.js';
    document.head.append(entry);

    await expect(
      discoverComfyNodes2RuntimeAssets(document, async () => 'export {};'),
    ).rejects.toThrow(NativeRendererCompatibilityError);
  });

  test('exposes Comfy native slot remeasurement with the other Vue runtime capabilities', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-current.js';
    document.head.append(entry);
    const requestSlotLayoutSync = jest.fn();
    const sources = new Map([
      [
        entry.src,
        'const deps=["./main-current.js","./settingStore-current.js","./vendor-vue-core-current.js"];',
      ],
      [
        'http://127.0.0.1:8188/assets/main-current.js',
        'const graph = () => import("./GraphView-current.js");',
      ],
      [
        'http://127.0.0.1:8188/assets/settingStore-current.js',
        'export{extractVueNodeData,requestSlotLayoutSyncForAllNodes};',
      ],
      ['http://127.0.0.1:8188/assets/vendor-vue-core-current.js', 'export const vue = true;'],
      [
        'http://127.0.0.1:8188/assets/GraphView-current.js',
        'const Native=defineComponent({__name:`LGraphNode`,setup(){}});',
      ],
    ]);
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

    const runtime = await loadComfyVueRuntime(document, moduleFetcher(sources), importModule);
    runtime.requestSlotLayoutSync();

    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(1);
  });

  test('keeps floor Nodes 2 usable when global slot remeasurement is unavailable', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-floor.js';
    document.head.append(entry);
    const sources = new Map([
      [entry.src, 'const deps=["./vendor-vue-core-floor.js","./GraphView-floor.js"];'],
      ['http://127.0.0.1:8188/assets/vendor-vue-core-floor.js', 'export const vue = true;'],
      [
        'http://127.0.0.1:8188/assets/GraphView-floor.js',
        'function extractVueNodeData(e){return e}const Native=defineComponent({__name:`LGraphNode`,setup(){}});',
      ],
    ]);
    const importModule = jest.fn(async (url: string) =>
      url.endsWith('vendor-vue-core-floor.js')
        ? {
            render: function render(): void {},
            h: function h(): object {
              return {};
            },
          }
        : {
            extractVueNodeData: function extractVueNodeData(): object {
              return {};
            },
          },
    );

    const runtime = await loadComfyVueRuntime(document, moduleFetcher(sources), importModule);
    expect(() => runtime.requestSlotLayoutSync()).not.toThrow();
  });

  test('loads the Vue render runtime without requiring node or Settings capabilities', async () => {
    document.head.replaceChildren();
    const entry = document.createElement('script');
    entry.type = 'module';
    entry.src = 'http://127.0.0.1:8188/assets/index-vue.js';
    document.head.append(entry);
    const render = function render(): void {};
    const h = function h(): object {
      return {};
    };
    const runtime = await loadComfyVueRenderRuntime(
      document,
      moduleFetcher(
        new Map([
          [entry.src, 'import "./vendor-vue-core-vue.js";'],
          ['http://127.0.0.1:8188/assets/vendor-vue-core-vue.js', 'export const vue = true;'],
        ]),
      ),
      async () => ({ render, h }),
    );

    expect(runtime.render).toBeDefined();
    expect(runtime.h).toBeDefined();
  });
});

/** Return deterministic module text and fail when discovery escapes the fixture graph. */
function moduleFetcher(
  sources: ReadonlyMap<string, string>,
): jest.MockedFunction<(url: string) => Promise<string>> {
  return jest.fn(async (url: string) => {
    const source = sources.get(url);
    if (source === undefined) throw new Error(`Unexpected module request: ${url}`);
    return source;
  });
}
