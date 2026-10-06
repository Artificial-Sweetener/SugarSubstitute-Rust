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
/** Prove masonry graph filtering, native completion and the explicit unfiltered escape hatch. */
import { nextTick } from 'vue';
import { jest } from '@jest/globals';
import { compatibleModel } from '../support/model_compatibility.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import type { ModelPickerBinding } from '../../../frontend/comfyui/ui/model_library/ModelPickerBinding.js';
import { comfyModelCompatibilityContext } from '../../../frontend/comfyui/ui/model_library/ComfyModelCompatibilityContext.js';
import type { ModelCatalogSnapshot } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import {
  assignCubeModelRequirement,
  cubeModelRequirement,
} from '../../../frontend/comfyui/ui/model_library/CubeModelRequirements.js';
import { resolveCubeInstalledModel } from '../../../frontend/comfyui/ui/model_library/CubeInstalledModelResolution.js';
import { modelScopeMenu } from '../../../frontend/comfyui/ui/model_library/ModelScopeMenus.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

beforeAll(() =>
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  }),
);
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('assigns portable Cube policy, preserves compatible selections and resolves mismatches without touching loose nodes', () => {
  const identity: UnknownRecord = { instance_id: 'cube-1', default_alias: 'Example' };
  const definition = {
    id: 'subgraph',
    name: 'Example',
    _nodes: [{ id: 'checkpoint', type: 'CheckpointLoaderSimple' }],
    inputs: [],
    outputs: [],
    extra: {},
  };
  const graph = {
    id: 'root',
    _nodes: [] as unknown[],
    beforeChange: jest.fn(),
    afterChange: jest.fn(),
    setDirtyCanvas: jest.fn(),
  };
  const cube = {
    id: 'cube-1',
    subgraph: definition,
    graph,
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: identity },
    pos: [0, 0],
    size: [400, 300],
    inputs: [],
    outputs: [],
    isSubgraphNode: () => true,
    connect: () => undefined,
    serialize: () => ({}),
  };
  graph._nodes.push(cube);
  const anchor = document.createElement('div');
  anchor.className = 'sugarcubes-cube-face';
  anchor.dataset.cubeNodeLocator = 'subgraph:checkpoint';
  const correct = compatibleModel('correct.safetensors', 'checkpoints', 'SDXL', 'Illustrious');
  const wrong = compatibleModel('wrong.safetensors', 'checkpoints', 'FLUX', 'Flux1');
  let value = correct.value;
  const writeValue = jest.fn((next: string) => {
    value = next;
  });
  const binding: ModelPickerBinding = {
    anchor,
    kind: 'checkpoints',
    label: 'ckpt_name',
    options: [correct.value, wrong.value],
    readValue: () => value,
    writeValue,
    disabled: () => false,
    subscribe: () => () => undefined,
  };
  const snapshot: ModelCatalogSnapshot = {
    revision: 1,
    scanning: false,
    items: [correct, wrong],
    fieldContracts: [],
  };
  const app = { graph };
  assignCubeModelRequirement(app, binding, { architecture: 'SDXL', family: 'Illustrious' });
  expect(cubeModelRequirement(app, binding)).toEqual({
    architecture: 'SDXL',
    family: 'Illustrious',
  });
  expect(definition.extra).toMatchObject({
    sugarcubes_cube: {
      model_requirements: { schemaVersion: 1, architecture: 'SDXL', family: 'Illustrious' },
    },
  });
  expect(graph.beforeChange).toHaveBeenCalledTimes(1);
  expect(graph.afterChange).toHaveBeenCalledTimes(1);
  for (const kind of ['checkpoints', 'diffusion_models', 'loras']) {
    expect(comfyModelCompatibilityContext(app, { ...binding, kind }, snapshot)).toMatchObject({
      architecture: 'SDXL',
      family: 'Illustrious',
      addon: kind === 'loras',
    });
  }
  for (const kind of [
    'upscale_models',
    'vae',
    'embeddings',
    'text_encoders',
    'clip',
    'unet',
    'ultralytics',
    'misc',
  ]) {
    const unrelated = { ...binding, kind };
    expect(comfyModelCompatibilityContext(app, unrelated, snapshot)).toBeNull();
    expect(modelScopeMenu(app, unrelated, snapshot, () => undefined)).toEqual([]);
    resolveCubeInstalledModel(app, unrelated, snapshot);
    expect(writeValue).not.toHaveBeenCalled();
  }
  resolveCubeInstalledModel(app, binding, snapshot);
  expect(writeValue).not.toHaveBeenCalled();
  value = wrong.value;
  resolveCubeInstalledModel(app, binding, snapshot);
  expect(writeValue).toHaveBeenCalledWith(correct.value);
  const unfiltered = modelScopeMenu(app, binding, snapshot, () => undefined)[0]?.children?.find(
    (entry) => entry.label === 'Unfiltered',
  );
  unfiltered?.select?.();
  value = wrong.value;
  resolveCubeInstalledModel(app, binding, snapshot);
  expect(comfyModelCompatibilityContext(app, binding, snapshot)).toBeNull();
  expect(writeValue).toHaveBeenCalledTimes(1);
  anchor.className = '';
  assignCubeModelRequirement(app, binding, { architecture: 'SDXL', family: 'Illustrious' });
  resolveCubeInstalledModel(app, binding, snapshot);
  expect(value).toBe(wrong.value);
  expect(modelScopeMenu(app, binding, snapshot, () => undefined)).toEqual([]);
});

test('follows MODEL connections through reroutes and chained LoRAs only in masonry', () => {
  const anchor = document.createElement('div');
  anchor.dataset.nodeId = '4';
  const binding: ModelPickerBinding = {
    anchor,
    kind: 'loras',
    label: 'lora_name',
    options: [],
    readValue: () => '',
    writeValue: () => undefined,
    disabled: () => false,
    subscribe: () => () => undefined,
  };
  const checkpoint = compatibleModel('base.safetensors', 'checkpoints', 'SDXL', 'Illustrious');
  const graph = {
    id: 'root',
    _nodes: [
      {
        id: 1,
        type: 'CheckpointLoaderSimple',
        widgets: [{ name: 'ckpt_name', value: checkpoint.value }],
      },
      { id: 2, type: 'Reroute', inputs: [{ type: '*', link: 1 }] },
      { id: 3, type: 'LoraLoader', inputs: [{ type: 'MODEL', link: 2 }] },
      { id: 4, type: 'LoraLoader', inputs: [{ type: 'MODEL', link: 3 }] },
    ],
    links: new Map([
      [1, { origin_id: 1 }],
      [2, { origin_id: 2 }],
      [3, { origin_id: 3 }],
    ]),
  };
  const snapshot: ModelCatalogSnapshot = {
    revision: 1,
    scanning: false,
    items: [checkpoint],
    fieldContracts: [
      { classType: 'CheckpointLoaderSimple', inputName: 'ckpt_name', kind: 'checkpoints' },
    ],
  };
  expect(comfyModelCompatibilityContext({ graph }, binding, snapshot)).toBeNull();
  anchor.className = 'sugarcubes-workflow-masonry';
  expect(comfyModelCompatibilityContext({ graph }, binding, snapshot)).toEqual({
    architecture: 'SDXL',
    family: 'Illustrious',
    addon: true,
    label: 'Illustrious',
  });
  graph.links.set(1, { origin_id: 4 });
  expect(comfyModelCompatibilityContext({ graph }, binding, snapshot)).toBeNull();
});

test('filters compatible models, navigates arrows and Tab, then exposes all choices without writes', async () => {
  const root = document.createElement('div');
  document.body.append(root);
  const first = compatibleModel('Portrait/first.safetensors', 'loras', 'SDXL', 'Illustrious', [
    { family: 'Illustrious', support: 'full' },
  ]);
  const second = compatibleModel('Styles/second.safetensors', 'loras', 'SDXL', 'SDXL', [
    { family: 'Illustrious', support: 'partial' },
  ]);
  const wrong = compatibleModel('Flux/wrong.safetensors', 'loras', 'FLUX', 'Flux1');
  let value = first.value;
  const writes = jest.fn((next: string) => {
    value = next;
  });
  const binding: ModelPickerBinding = {
    anchor: root,
    kind: 'loras',
    label: 'lora_name',
    options: [first.value, second.value, wrong.value],
    readValue: () => value,
    writeValue: writes,
    disabled: () => false,
    subscribe: () => () => undefined,
  };
  const picker = mountModelPicker(
    root,
    binding,
    [first, second, wrong],
    null,
    null,
    null,
    null,
    null,
    () => ({ architecture: 'SDXL', family: 'Illustrious', addon: true, label: 'Illustrious' }),
  );
  try {
    const input = root.querySelector('input');
    if (!input) throw new Error('Missing input.');
    input.click();
    await nextTick();
    expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(2);
    expect(document.querySelector('.sc-model-compatibility')).toBeNull();
    expect(
      [...document.querySelectorAll('.sc-model-child-folder')].map((folder) => folder.textContent),
    ).toEqual(['Portrait', 'Styles']);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await nextTick();
    expect(document.querySelector('.sc-model-completion')?.textContent).toContain('second');
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
    );
    await nextTick();
    expect(input.value).toBe('second');
    expect(
      [...document.querySelectorAll('.sc-model-child-folder')].map((folder) => folder.textContent),
    ).toEqual(['Styles']);
    expect(writes).not.toHaveBeenCalled();
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
    );
    await nextTick();
    expect(writes).toHaveBeenCalledWith(second.value);
    input.click();
    await nextTick();
    const showAll = document.querySelector<HTMLButtonElement>(
      '.sc-model-routes button[aria-label="Show all"]',
    );
    expect(showAll?.querySelector('.sc-model-filter-icon[aria-hidden="true"]')).not.toBeNull();
    expect(showAll?.getAttribute('aria-pressed')).toBe('false');
    showAll?.click();
    await nextTick();
    expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(3);
    expect(
      [...document.querySelectorAll('.sc-model-child-folder')].map((folder) => folder.textContent),
    ).toEqual(['Flux', 'Portrait', 'Styles']);
    const showCompatible = document.querySelector<HTMLButtonElement>(
      '.sc-model-routes button[aria-label="Show only SDXL"]',
    );
    expect(showCompatible?.getAttribute('aria-pressed')).toBe('true');
    expect(showCompatible?.textContent).toBe('Show only SDXL');
    showCompatible?.click();
    await nextTick();
    expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(2);
    expect(
      [...document.querySelectorAll('.sc-model-child-folder')].map((folder) => folder.textContent),
    ).toEqual(['Portrait', 'Styles']);
    expect(writes).toHaveBeenCalledTimes(1);
  } finally {
    picker.dispose();
  }
});
