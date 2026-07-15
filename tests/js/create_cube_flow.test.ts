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
import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { CubeCreationService } from '../../frontend/comfyui/ui/create/CubeCreationService.js';
import { ComfyAdapter } from '../../frontend/comfyui/ui/core/ComfyAdapter.js';
import type { HostWindow } from '../../frontend/comfyui/ui/core/ComfyAdapter.js';
import type {
  CreationApi,
  CreateCubeValues,
} from '../../frontend/comfyui/ui/create/CubeCreationService.js';
import { CreateCubeModal } from '../../frontend/comfyui/ui/dialogs/CreateCubeModal.js';
import type { CreateCubeModalCandidate } from '../../frontend/comfyui/ui/dialogs/CreateCubeModal.js';
import type { ModalAdapter } from '../../frontend/comfyui/ui/dialogs/ModalShell.js';
import type {
  ComfyCanvas,
  ComfyGraph,
  ComfyGroup,
  ComfyLink,
  ComfyNode,
  ComfyWidget,
} from '../../frontend/comfyui/ui/types/graph.js';

interface MarkerOptions {
  id: number;
  type: string;
  defaultAlias: string;
  cubeId?: string;
}

interface TestNode extends ComfyNode {
  id: number;
  type: string;
  pos: number[];
  size: number[];
  inputs: Array<{ name: string; link: number | null }>;
  outputs: Array<{ type: string; links: number[] }>;
  widgets: ComfyWidget[];
  graph?: TestGraph;
}

interface TestGraph extends ComfyGraph {
  _nodes: TestNode[];
  _groups: ComfyGroup[];
  links: Record<string, ComfyLink>;
  add: jest.Mock<(entry: ComfyGroup) => void>;
  remove: jest.Mock<(entry: ComfyGroup) => void>;
  setDirtyCanvas: jest.Mock;
}

interface PointerOptions {
  clientX?: number;
  clientY?: number;
  button?: number;
  isPrimary?: boolean;
  pointerId?: number;
}

interface PromptNode {
  class_type: string;
  inputs: Record<string, unknown>;
}

interface SavedRequestBody {
  actor?: unknown;
  cubes: Array<{ cube_id: string; metadata: unknown }>;
  graph: unknown;
  workflow: { groups?: unknown; [key: string]: unknown };
}

function parseSavedRequest(body: unknown): SavedRequestBody {
  if (typeof body !== 'string') throw new Error('Expected a serialized JSON request body');
  return JSON.parse(body) as SavedRequestBody;
}

type TestElement = HTMLElement & {
  value: string;
  disabled: boolean;
  options: HTMLOptionsCollection;
};

function requiredElement(selector: string): TestElement {
  const element = document.querySelector<TestElement>(selector);
  if (!element) throw new Error(`Missing test element: ${selector}`);
  return element;
}

function makeMarker({ id, type, defaultAlias, cubeId = '' }: MarkerOptions): TestNode {
  return {
    id,
    type,
    pos: [0, 0],
    size: [140, 46],
    inputs: [{ name: 'value', link: null }],
    outputs: [{ type: 'IMAGE', links: [] }],
    widgets: [
      { name: 'cube_id', value: cubeId },
      { name: 'default_alias', value: defaultAlias },
      { name: 'instance_alias', value: defaultAlias },
      { name: 'instance_id', value: '' },
    ],
  };
}

function widgetValue(node: TestNode, name: string): unknown {
  return node.widgets.find((widget) => widget.name === name)?.value || '';
}

function setWidgetValue(node: TestNode, name: string, value: unknown): void {
  const widget = node.widgets.find((entry) => entry.name === name);
  if (widget) {
    widget.value = value;
  }
}

function makeGraph() {
  const graph = {
    _nodes: [] as TestNode[],
    _groups: [] as ComfyGroup[],
    links: {} as Record<string, ComfyLink>,
    setDirtyCanvas: jest.fn(),
  } as TestGraph;
  graph.add = jest.fn((entry: ComfyGroup) => {
    graph._groups.push(entry);
  });
  graph.remove = jest.fn((entry: ComfyGroup) => {
    const index = graph._groups.indexOf(entry);
    if (index >= 0) {
      graph._groups.splice(index, 1);
    }
  });
  const inputMarker = makeMarker({
    id: 1,
    type: 'SugarCubes.CubeInput',
    defaultAlias: 'Text to Image',
  });
  const node: TestNode = {
    id: 2,
    type: 'KSampler',
    class_type: 'KSampler',
    pos: [200, 0],
    size: [180, 60],
    inputs: [{ name: 'image', link: 10 }],
    outputs: [{ type: 'IMAGE', links: [11] }],
    widgets: [],
  };
  const outputMarker = makeMarker({
    id: 3,
    type: 'SugarCubes.CubeOutput',
    defaultAlias: 'Text to Image',
  });
  outputMarker.inputs = [{ name: 'value', link: 11 }];
  outputMarker.outputs = [{ type: 'IMAGE', links: [] }];
  const unrelatedMarker = makeMarker({
    id: 4,
    type: 'SugarCubes.CubeInput',
    defaultAlias: 'Other Cube',
  });
  graph._nodes = [inputMarker, node, outputMarker, unrelatedMarker];
  graph.links = {
    10: {
      id: 10,
      origin_id: inputMarker.id,
      origin_slot: 0,
      target_id: node.id,
      target_slot: 0,
      type: 'IMAGE',
    },
    11: {
      id: 11,
      origin_id: node.id,
      origin_slot: 0,
      target_id: outputMarker.id,
      target_slot: 0,
      type: 'IMAGE',
    },
  };
  for (const entry of graph._nodes) {
    entry.graph = graph;
  }
  return { graph, inputMarker, node, outputMarker, unrelatedMarker };
}

function makePromptFromGraph(graph: TestGraph): Record<string, PromptNode> {
  const prompt: Record<string, PromptNode> = {};
  for (const node of graph._nodes) {
    prompt[String(node.id)] = {
      class_type: node.type,
      inputs: {
        cube_id: widgetValue(node, 'cube_id'),
        default_alias: widgetValue(node, 'default_alias'),
      },
    };
  }
  prompt['2'].inputs.image = ['1', 0];
  prompt['3'].inputs.value = ['2', 0];
  return prompt;
}

function makeCanvasContext(): CanvasRenderingContext2D {
  const ctx = {
    save: jest.fn(),
    restore: jest.fn(),
    beginPath: jest.fn(),
    rect: jest.fn(),
    fill: jest.fn(),
    stroke: jest.fn(),
    fillRect: jest.fn(),
    strokeRect: jest.fn(),
    fillText: jest.fn(),
  };
  Object.defineProperty(ctx, 'globalAlpha', { writable: true, value: 1 });
  Object.defineProperty(ctx, 'fillStyle', { writable: true, value: '' });
  Object.defineProperty(ctx, 'strokeStyle', { writable: true, value: '' });
  Object.defineProperty(ctx, 'font', { writable: true, value: '' });
  Object.defineProperty(ctx, 'textAlign', { writable: true, value: '' });
  Object.defineProperty(ctx, 'lineWidth', { writable: true, value: 1 });
  return ctx as unknown as CanvasRenderingContext2D;
}

beforeEach(() => {
  document.body.innerHTML = '';
  delete window.LGraphCanvas;
  delete window.app;
});

function makePointerEvent(type: string, options: PointerOptions = {}): PointerEvent {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    clientX: options.clientX ?? 0,
    clientY: options.clientY ?? 0,
    button: options.button ?? 0,
  });
  Object.defineProperty(event, 'isPrimary', { value: options.isPrimary ?? true });
  Object.defineProperty(event, 'pointerId', { value: options.pointerId ?? 1 });
  return event as PointerEvent;
}

function deriveExactFilename(defaultAlias: string): string {
  const trimmed = defaultAlias.trim();
  return trimmed.toLowerCase().endsWith('.cube') ? trimmed : `${trimmed}.cube`;
}

function openCreateModalWithAdapter(
  adapterOverrides: ModalAdapter & { getCanvas?: () => ComfyCanvas | null } = {},
  candidateOverrides: Partial<CreateCubeModalCandidate> = {},
) {
  const modal = new CreateCubeModal({
    adapter: {
      getDocument: () => document,
      getWindow: () => window,
      ...adapterOverrides,
    },
  });
  const promise = modal.open({
    candidate: {
      defaultAlias: 'Text to Image',
      cubeId: 'local/personal/Text to Image.cube',
      markerIds: [1, 3],
      nodeIds: [2],
      warnings: [],
      ...candidateOverrides,
    },
    deriveCubeId: (defaultAlias) => `local/personal/${deriveExactFilename(defaultAlias)}`,
  });
  const overlay = requiredElement('.sugarcubes-create-cube-overlay');
  const dialog = requiredElement('.sugarcubes-create-cube-dialog');
  return { modal, promise, overlay, dialog };
}

function openCreateModalWithCanvas(
  canvas: ComfyCanvas,
  candidateOverrides: Partial<CreateCubeModalCandidate> = {},
) {
  return openCreateModalWithAdapter({ getCanvas: () => canvas }, candidateOverrides);
}

function selectDestination(key: string): HTMLSelectElement {
  const select = document.querySelector<HTMLSelectElement>(
    '.sugarcubes-create-cube__destination-select',
  );
  if (!select) throw new Error('Missing destination select');
  select.value = key;
  select.dispatchEvent(new window.Event('change', { bubbles: true }));
  return select;
}

function selectTargetModelByLabel(label: string): HTMLSelectElement {
  const select = document.querySelector<HTMLSelectElement>(
    '.sugarcubes-create-cube__target-model-select',
  );
  if (!select) throw new Error('Missing target model select');
  const option = Array.from(select.options).find((entry) => entry.textContent === label);
  expect(option).toBeDefined();
  select.value = option!.value;
  select.dispatchEvent(new window.Event('change', { bubbles: true }));
  return select;
}

describe('marker create cube flow', () => {
  test('modal uses lower viewport variant and derives identity preview', async () => {
    const modal = new CreateCubeModal({
      adapter: {
        getDocument: () => document,
        getWindow: () => window,
      },
    });

    const promise = modal.open({
      candidate: {
        defaultAlias: 'Text to Image',
        cubeId: 'local/personal/Text to Image.cube',
        markerIds: [1, 3],
        nodeIds: [2],
        warnings: [],
      },
      deriveCubeId: (defaultAlias) => `local/personal/${deriveExactFilename(defaultAlias)}`,
    });

    const overlay = requiredElement('.sugarcubes-create-cube-overlay');
    expect(overlay).not.toBeNull();
    expect(overlay.classList.contains('is-visible')).toBe(true);
    expect(document.body.textContent).toContain('local/personal/SDXL/Text to Image.cube');

    const confirmButton = requiredElement('.sugarcubes-create-cube-dialog button:last-child');
    confirmButton.click();
    await expect(promise).resolves.toEqual({
      defaultAlias: 'SDXL/Text to Image',
      cubeName: 'Text to Image',
      cubeId: 'local/personal/SDXL/Text to Image.cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL', 'SD 1.5'],
      description: '',
    });
  });

  test('modal normalizes authored title before deriving and returning identity', async () => {
    const { promise } = openCreateModalWithAdapter(
      {},
      {
        defaultAlias: 'text to image',
        cubeId: 'local/personal/text to image.cube',
      },
    );

    const nameInput = requiredElement('.sugarcubes-create-cube__form input');
    expect(nameInput?.value).toBe('Text to Image');
    expect(document.body.textContent).toContain('local/personal/SDXL/Text to Image.cube');

    const confirmButton = requiredElement('.sugarcubes-create-cube-dialog button:last-child');
    confirmButton.click();
    await expect(promise).resolves.toEqual({
      defaultAlias: 'SDXL/Text to Image',
      cubeName: 'Text to Image',
      cubeId: 'local/personal/SDXL/Text to Image.cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL', 'SD 1.5'],
      description: '',
    });
  });

  test('modal saves Anima as a built-in target model', async () => {
    const { promise } = openCreateModalWithAdapter();

    selectTargetModelByLabel('Anima');
    expect(document.body.textContent).toContain('local/personal/Anima/Text to Image.cube');

    const confirmButton = requiredElement('.sugarcubes-create-cube-dialog button:last-child');
    confirmButton.click();
    await expect(promise).resolves.toEqual({
      defaultAlias: 'Anima/Text to Image',
      cubeName: 'Text to Image',
      cubeId: 'local/personal/Anima/Text to Image.cube',
      targetModel: 'Anima',
      supportedModels: ['Anima'],
      description: '',
    });
  });

  test('modal accepts a custom target model after selecting a different model', async () => {
    const { promise } = openCreateModalWithAdapter();

    selectTargetModelByLabel('A different model');
    const customInput = requiredElement('.sugarcubes-create-cube__custom-target-model');
    expect(customInput?.disabled).toBe(false);
    customInput.value = 'Custom Model';
    customInput.dispatchEvent(new Event('input', { bubbles: true }));
    expect(document.body.textContent).toContain('local/personal/Custom Model/Text to Image.cube');

    const confirmButton = requiredElement('.sugarcubes-create-cube-dialog button:last-child');
    confirmButton.click();
    await expect(promise).resolves.toEqual({
      defaultAlias: 'Custom Model/Text to Image',
      cubeName: 'Text to Image',
      cubeId: 'local/personal/Custom Model/Text to Image.cube',
      targetModel: 'Custom Model',
      supportedModels: ['Custom Model'],
      description: '',
    });
  });

  test('modal reports invalid custom target models without closing', async () => {
    const { promise } = openCreateModalWithAdapter();

    selectTargetModelByLabel('A different model');
    const customInput = requiredElement('.sugarcubes-create-cube__custom-target-model');
    customInput.value = 'Bad/Model';
    customInput.dispatchEvent(new Event('input', { bubbles: true }));

    const confirmButton = requiredElement('.sugarcubes-create-cube-dialog button:last-child');
    confirmButton.click();
    expect(document.querySelector('.sugarcubes-modal__error')?.textContent).toContain(
      'one path segment',
    );

    const cancelButton = requiredElement('.sugarcubes-create-cube-dialog button');
    cancelButton.click();
    await expect(promise).resolves.toBeNull();
  });

  test('modal derives identity preview from selected cube pack destination', async () => {
    const modal = new CreateCubeModal({
      adapter: {
        getDocument: () => document,
        getWindow: () => window,
      },
    });
    const packDestination = {
      key: 'github/Artificial-Sweetener/Base-Cubes',
      sourceKind: 'github',
      owner: 'Artificial-Sweetener',
      repo: 'Base-Cubes',
      label: 'Base-Cubes',
      detail: 'Artificial-Sweetener',
      writable: true,
    };
    const promise = modal.open({
      candidate: {
        defaultAlias: 'Text to Image',
        cubeId: 'local/personal/Text to Image.cube',
        markerIds: [1, 3],
        nodeIds: [2],
        warnings: [],
      },
      destinations: [
        {
          key: 'local/personal',
          sourceKind: 'local',
          namespace: 'personal',
          label: 'local',
          detail: 'personal',
          writable: true,
        },
        packDestination,
      ],
      deriveCubeId: (defaultAlias, destination) => {
        const trimmed = defaultAlias.trim();
        const filename = trimmed.toLowerCase().endsWith('.cube') ? trimmed : `${trimmed}.cube`;
        if (destination.sourceKind === 'github') {
          return `${destination.owner}/${destination.repo}/${filename}`;
        }
        return `local/${destination.namespace}/${filename}`;
      },
    });

    selectDestination('github/Artificial-Sweetener/Base-Cubes');

    expect(document.body.textContent).toContain(
      'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
    );
    expect(document.body.textContent).toContain('Base-Cubes');
    expect(document.body.textContent).toContain('Artificial-Sweetener');

    requiredElement('.sugarcubes-create-cube-dialog button:last-child').click();
    await expect(promise).resolves.toEqual({
      defaultAlias: 'SDXL/Text to Image',
      cubeName: 'Text to Image',
      cubeId: 'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL', 'SD 1.5'],
      description: '',
    });
  });

  test('modal selects a newly created pack destination', async () => {
    const modal = new CreateCubeModal({
      adapter: {
        getDocument: () => document,
        getWindow: () => window,
      },
    });
    const createdDestination = {
      key: 'github/ExampleUser/Example-Cubes',
      sourceKind: 'github',
      owner: 'ExampleUser',
      repo: 'Example-Cubes',
      label: 'Example-Cubes',
      detail: 'ExampleUser',
      writable: true,
    };
    const promise = modal.open({
      candidate: {
        defaultAlias: 'Text to Image',
        cubeId: 'local/personal/Text to Image.cube',
        markerIds: [1, 3],
        nodeIds: [2],
        warnings: [],
      },
      destinations: [
        {
          key: 'local/personal',
          sourceKind: 'local',
          namespace: 'personal',
          label: 'local',
          detail: 'personal',
          writable: true,
        },
        {
          key: 'new-pack',
          action: 'create-pack',
          label: 'New pack...',
          detail: 'Create a writable cube pack',
        },
      ],
      deriveCubeId: (defaultAlias, destination) => {
        const filename = deriveExactFilename(defaultAlias);
        return destination.sourceKind === 'github'
          ? `${destination.owner}/${destination.repo}/${filename}`
          : `local/${destination.namespace}/${filename}`;
      },
      onCreateDestination: jest.fn(async () => createdDestination),
    });

    selectDestination('new-pack');
    await Promise.resolve();

    expect(document.body.textContent).toContain('Example-Cubes');
    expect(document.body.textContent).toContain(
      'ExampleUser/Example-Cubes/SDXL/Text to Image.cube',
    );

    requiredElement('.sugarcubes-create-cube-dialog button:last-child').click();
    await expect(promise).resolves.toEqual({
      defaultAlias: 'SDXL/Text to Image',
      cubeName: 'Text to Image',
      cubeId: 'ExampleUser/Example-Cubes/SDXL/Text to Image.cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL', 'SD 1.5'],
      description: '',
    });
  });

  test('modal restores the previous destination when new pack creation is canceled', async () => {
    const modal = new CreateCubeModal({
      adapter: {
        getDocument: () => document,
        getWindow: () => window,
      },
    });
    const promise = modal.open({
      candidate: {
        defaultAlias: 'Text to Image',
        cubeId: 'local/personal/Text to Image.cube',
        markerIds: [1, 3],
        nodeIds: [2],
        warnings: [],
      },
      destinations: [
        {
          key: 'local/personal',
          sourceKind: 'local',
          namespace: 'personal',
          label: 'local',
          detail: 'personal',
          writable: true,
        },
        {
          key: 'new-pack',
          action: 'create-pack',
          label: 'New pack...',
          detail: 'Create a writable cube pack',
        },
      ],
      deriveCubeId: (defaultAlias, destination) => {
        const filename = deriveExactFilename(defaultAlias);
        return `local/${destination.namespace || 'personal'}/${filename}`;
      },
      onCreateDestination: jest.fn(async () => null),
    });

    selectDestination('new-pack');
    await Promise.resolve();

    expect(document.body.textContent).toContain('local/personal/SDXL/Text to Image.cube');

    requiredElement('.sugarcubes-create-cube-dialog button:last-child').click();
    await expect(promise).resolves.toEqual({
      defaultAlias: 'SDXL/Text to Image',
      cubeName: 'Text to Image',
      cubeId: 'local/personal/SDXL/Text to Image.cube',
      targetModel: 'SDXL',
      supportedModels: ['SDXL', 'SD 1.5'],
      description: '',
    });
  });

  test('modal keeps name enforcement at save confirmation for unnamed candidates', async () => {
    const { promise } = openCreateModalWithAdapter({}, { defaultAlias: '' });
    const nameInput = requiredElement('.sugarcubes-create-cube__form input');
    const confirmButton = requiredElement('.sugarcubes-create-cube-dialog button:last-child');
    let resolved = false;
    promise.then(() => {
      resolved = true;
    });

    expect(nameInput.value).toBe('SugarCube');
    nameInput.value = '';
    nameInput.dispatchEvent(new Event('input', { bubbles: true }));
    confirmButton.click();
    await Promise.resolve();

    expect(resolved).toBe(false);
    expect(document.querySelector('.sugarcubes-modal__error')?.textContent).toBe(
      'Name is required.',
    );

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('unnamed markers open create candidate discovery and match blank names', async () => {
    const { graph, inputMarker, outputMarker } = makeGraph();
    setWidgetValue(inputMarker, 'default_alias', '');
    setWidgetValue(inputMarker, 'instance_alias', '');
    setWidgetValue(outputMarker, 'default_alias', '');
    setWidgetValue(outputMarker, 'instance_alias', '');
    const toast = { push: jest.fn() };
    const openCreatePersonalCube = jest.fn(async ({ candidate }) => {
      expect(candidate.defaultAlias).toBe('');
      expect(candidate.cubeId).toBe('local/personal/SugarCube.cube');
      expect(candidate.targetModel).toBe('');
      expect(candidate.supportedModels).toEqual([]);
      expect(candidate.markerIds).toEqual([1, 3]);
      expect(candidate.nodeIds).toEqual([2]);
      expect(candidate.markers).toEqual([inputMarker, outputMarker]);
      expect(candidate.nodes).toEqual([graph._nodes[1]]);
      return null;
    });
    const service = new CubeCreationService({
      adapter: {
        getApp: () => ({ graph }),
        getConsole: () => ({ error: jest.fn() }),
      },
      dialogs: { openCreatePersonalCube },
      toast,
    });

    const result = await service.startCreateCubeFromMarker(inputMarker);

    expect(result).toBeNull();
    expect(openCreatePersonalCube).toHaveBeenCalledTimes(1);
    expect(toast.push).not.toHaveBeenCalledWith(
      'error',
      'Default alias is required.',
      expect.anything(),
    );
  });

  test('create action saves locally without loading packs or requiring sharing metadata', async () => {
    const { graph, inputMarker } = makeGraph();
    let saveBody: SavedRequestBody | null = null;
    const openCreatePersonalCube = jest.fn(async ({ deriveIdentity }) => {
      expect(deriveIdentity('text to image')).toEqual({
        name: 'Text to Image',
        defaultAlias: 'Text to Image',
        cubeId: 'local/personal/Text to Image.cube',
      });
      return deriveIdentity('text to image');
    });
    const service = new CubeCreationService({
      adapter: {
        getApp: () => ({
          graph,
          graphToPrompt: () => ({
            output: makePromptFromGraph(graph),
            workflow: { nodes: [], groups: [], version: 1 },
          }),
        }),
        getConsole: () => ({ error: jest.fn() }),
      },
      api: {
        saveImplementation: jest.fn(async (body) => {
          saveBody = parseSavedRequest(body);
          return {
            response: { ok: true },
            data: { saved: [{ cube_id: 'local/personal/Text to Image.cube' }] },
          };
        }),
      },
      toast: { push: jest.fn() },
      dialogs: { openCreatePersonalCube },
      instanceManager: { refresh: jest.fn(), scheduleRefresh: jest.fn() },
      saveReconciler: { reconcile: jest.fn(async () => {}) },
      cubeBrowser: { refresh: jest.fn(async () => {}) },
    });

    const result = await service.startCreateCubeFromMarker(inputMarker);

    expect(result!.cubeId).toBe('local/personal/Text to Image.cube');
    expect(result!.defaultAlias).toBe('Text to Image');
    expect(result!.targetModel).toBe('');
    expect(result!.supportedModels).toEqual([]);
    expect(saveBody!.actor).toBeUndefined();
    expect(saveBody!.cubes[0].cube_id).toBe('local/personal/Text to Image.cube');
    expect(saveBody!.cubes[0].metadata).toEqual({
      default_alias: 'Text to Image',
    });
  });

  test('named marker discovery stops at nonmatching marker boundaries', () => {
    const { graph, inputMarker, outputMarker } = makeGraph();
    const matchingOutput = makeMarker({
      id: 5,
      type: 'SugarCubes.CubeOutput',
      defaultAlias: 'Text to Image',
    });
    matchingOutput.inputs = [{ name: 'value', link: 12 }];
    const downstream = {
      id: 6,
      type: 'PreviewImage',
      class_type: 'PreviewImage',
      pos: [640, 0],
      size: [180, 60],
      inputs: [{ name: 'images', link: 13 }],
      outputs: [],
      widgets: [],
      graph,
    };
    setWidgetValue(outputMarker, 'default_alias', 'Other Cube');
    graph._nodes.push(matchingOutput, downstream);
    matchingOutput.graph = graph;
    outputMarker.outputs = [{ type: 'IMAGE', links: [12] }];
    graph.links[12] = {
      id: 12,
      origin_id: outputMarker.id,
      origin_slot: 0,
      target_id: matchingOutput.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    matchingOutput.outputs = [{ type: 'IMAGE', links: [13] }];
    graph.links[13] = {
      id: 13,
      origin_id: matchingOutput.id,
      origin_slot: 0,
      target_id: downstream.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    const service = new CubeCreationService({
      adapter: { getApp: () => ({ graph }) },
    });

    const candidate = service.analyzeCreateCandidateFromMarker(inputMarker);

    expect(candidate.defaultAlias).toBe('Text to Image');
    expect(candidate.markerIds).toEqual([1]);
    expect(candidate.nodeIds).toEqual([2]);
  });

  test('unnamed marker discovery stops at nonmatching named marker boundaries', () => {
    const { graph, inputMarker, outputMarker } = makeGraph();
    setWidgetValue(inputMarker, 'default_alias', '');
    setWidgetValue(inputMarker, 'instance_alias', '');
    setWidgetValue(outputMarker, 'default_alias', 'Other Cube');
    const downstream = {
      id: 5,
      type: 'PreviewImage',
      class_type: 'PreviewImage',
      pos: [640, 0],
      size: [180, 60],
      inputs: [{ name: 'images', link: 12 }],
      outputs: [],
      widgets: [],
      graph,
    };
    outputMarker.outputs = [{ type: 'IMAGE', links: [12] }];
    graph._nodes.push(downstream);
    graph.links[12] = {
      id: 12,
      origin_id: outputMarker.id,
      origin_slot: 0,
      target_id: downstream.id,
      target_slot: 0,
      type: 'IMAGE',
    };
    const service = new CubeCreationService({
      adapter: { getApp: () => ({ graph }) },
    });

    const candidate = service.analyzeCreateCandidateFromMarker(inputMarker);

    expect(candidate.defaultAlias).toBe('');
    expect(candidate.markerIds).toEqual([1]);
    expect(candidate.nodeIds).toEqual([2]);
  });

  test('create action saves one candidate and leaves unrelated cubes alone', async () => {
    const { graph, inputMarker, node, outputMarker, unrelatedMarker } = makeGraph();
    let saveBody: SavedRequestBody | null = null;
    const canvas = { setDirty: jest.fn(), onDrawBackground: null };
    const service = new CubeCreationService({
      adapter: {
        getApp: () => ({
          graph,
          canvas,
          graphToPrompt: () => ({
            output: makePromptFromGraph(graph),
            workflow: { nodes: [], groups: [], version: 1 },
          }),
        }),
        getConsole: () => ({ error: jest.fn() }),
      },
      api: {
        saveImplementation: jest.fn(async (body) => {
          saveBody = parseSavedRequest(body);
          return {
            response: { ok: true },
            data: {
              saved: [
                {
                  cube_id: 'local/personal/Text to Image.cube',
                  default_alias: 'Text to Image',
                  path: 'local/personal/Text to Image.cube',
                  committed: false,
                },
              ],
            },
          };
        }),
      },
      toast: { push: jest.fn() },
      dialogs: {
        openCreatePersonalCube: jest.fn(async ({ candidate }) => {
          expect(candidate.markerIds.sort()).toEqual([1, 3]);
          expect(candidate.nodeIds).toEqual([2]);
          return {
            defaultAlias: 'Text to Image',
            cubeId: 'local/personal/Text to Image.cube',
            description: '',
          };
        }),
      },
      instanceManager: { scheduleRefresh: jest.fn() },
      saveReconciler: { reconcile: jest.fn(async () => {}) },
      cubeBrowser: { refresh: jest.fn(async () => {}) },
    });

    const result = await service.startCreateCubeFromMarker(inputMarker);

    expect(result!.cubeId).toBe('local/personal/Text to Image.cube');
    expect(saveBody!.cubes).toEqual([
      expect.objectContaining({
        cube_id: 'local/personal/Text to Image.cube',
        description: '',
      }),
    ]);
    expect(widgetValue(inputMarker, 'cube_id')).toBe('local/personal/Text to Image.cube');
    expect(widgetValue(outputMarker, 'cube_id')).toBe('local/personal/Text to Image.cube');
    expect(widgetValue(unrelatedMarker, 'cube_id')).toBe('');
    expect(canvas.onDrawBackground).toBeNull();
    expect(node.boxcolor).toBeUndefined();
    expect(inputMarker.boxcolor).toBeUndefined();
    expect(graph._groups).toEqual([]);
  });

  test('create action writes confirmed modal name to unnamed markers', async () => {
    const { graph, inputMarker, outputMarker } = makeGraph();
    setWidgetValue(inputMarker, 'default_alias', '');
    setWidgetValue(inputMarker, 'instance_alias', '');
    setWidgetValue(outputMarker, 'default_alias', '');
    setWidgetValue(outputMarker, 'instance_alias', '');
    let saveBody: SavedRequestBody | null = null;
    const service = new CubeCreationService({
      adapter: {
        getApp: () => ({
          graph,
          graphToPrompt: () => ({
            output: makePromptFromGraph(graph),
            workflow: { nodes: [], groups: [], version: 1 },
          }),
        }),
        getConsole: () => ({ error: jest.fn() }),
      },
      api: {
        saveImplementation: jest.fn(async (body) => {
          saveBody = parseSavedRequest(body);
          return {
            response: { ok: true },
            data: { saved: [{ cube_id: 'local/personal/Text to Image.cube' }] },
          };
        }),
      },
      toast: { push: jest.fn() },
      dialogs: {
        openCreatePersonalCube: jest.fn(async ({ candidate }) => {
          expect(candidate.defaultAlias).toBe('');
          expect(candidate.markerIds).toEqual([1, 3]);
          return {
            defaultAlias: 'Text to Image',
            cubeName: 'Text to Image',
            cubeId: 'local/personal/Text to Image.cube',
            description: '',
          };
        }),
      },
      instanceManager: { refresh: jest.fn(), scheduleRefresh: jest.fn() },
      saveReconciler: { reconcile: jest.fn(async () => {}) },
      cubeBrowser: { refresh: jest.fn(async () => {}) },
    });

    const result = await service.startCreateCubeFromMarker(inputMarker);

    expect(result!.cubeId).toBe('local/personal/Text to Image.cube');
    expect(widgetValue(inputMarker, 'default_alias')).toBe('Text to Image');
    expect(widgetValue(outputMarker, 'default_alias')).toBe('Text to Image');
    expect(widgetValue(inputMarker, 'instance_alias')).toBe('Text to Image');
    expect(widgetValue(outputMarker, 'instance_alias')).toBe('Text to Image');
    expect(saveBody!.cubes[0].cube_id).toBe('local/personal/Text to Image.cube');
  });

  test('create action refreshes managed cube chrome before graph serialization', async () => {
    const { graph, inputMarker, outputMarker } = makeGraph();
    const order: string[] = [];
    let saveBody: SavedRequestBody | null = null;
    const managedGroup = {
      title: 'Text to Image',
      bounding: [198, -34, 420, 260],
      properties: {
        sugarcubes: {
          managed: true,
          cube_id: 'local/personal/Text to Image.cube',
          default_alias: 'Text to Image',
        },
      },
    };
    const instanceManager = {
      refresh: jest.fn(({ graph: refreshedGraph }: { graph: ComfyGraph }) => {
        order.push('refresh');
        refreshedGraph._groups = [managedGroup];
      }),
      scheduleRefresh: jest.fn(),
    };
    const saveReconciler = {
      reconcile: jest.fn(async () => order.push('reconcile')),
    };
    const service = new CubeCreationService({
      adapter: {
        getApp: () => ({
          graph,
          graphToPrompt: () => {
            order.push('graphToPrompt');
            return {
              output: makePromptFromGraph(graph),
              workflow: { nodes: [], groups: graph._groups, version: 1 },
            };
          },
        }),
        getConsole: () => ({ error: jest.fn() }),
      },
      api: {
        saveImplementation: jest.fn(async (body) => {
          saveBody = parseSavedRequest(body);
          return {
            response: { ok: true },
            data: { saved: [{ cube_id: 'local/personal/Text to Image.cube' }] },
          };
        }),
      },
      instanceManager,
      saveReconciler,
      cubeBrowser: { refresh: jest.fn(async () => {}) },
    });

    await service.saveCreatedCubeCandidate(
      {
        graph,
        markers: [inputMarker, outputMarker],
        markerIds: [inputMarker.id, outputMarker.id],
        filename: 'Text to Image.cube',
      },
      {
        defaultAlias: 'Text to Image',
        cubeId: 'local/personal/Text to Image.cube',
        description: '',
      },
    );

    expect(order).toEqual(['refresh', 'graphToPrompt', 'reconcile']);
    expect(instanceManager.refresh).toHaveBeenCalledWith({ graph, reason: 'cube-create' });
    expect(saveBody!.workflow.groups).toEqual([managedGroup]);
    expect(saveReconciler.reconcile).toHaveBeenCalledWith({
      graph,
      saved: [{ cube_id: 'local/personal/Text to Image.cube' }],
      fallbackCubeIds: ['local/personal/Text to Image.cube'],
      markerIdsByCubeId: { 'local/personal/Text to Image.cube': [1, 3] },
      reason: 'cube-create',
    });
  });

  test('create action restores markers and managed chrome when serialization fails', async () => {
    const { graph, inputMarker, outputMarker } = makeGraph();
    const managedGroup = {
      title: 'Text to Image',
      bounding: [198, -34, 420, 260],
      properties: { sugarcubes: { managed: true } },
    };
    const instanceManager = {
      refresh: jest.fn(({ reason }: { reason: string }) => {
        if (reason === 'cube-create') {
          graph._groups = [managedGroup];
        } else if (reason === 'cube-create-rollback') {
          graph._groups = [];
        }
      }),
      scheduleRefresh: jest.fn(),
    };
    const service = new CubeCreationService({
      adapter: {
        getApp: () => ({
          graph,
          graphToPrompt: () => {
            throw new Error('Unable to serialize graph');
          },
        }),
        getConsole: () => ({ error: jest.fn() }),
      },
      api: { saveImplementation: jest.fn<CreationApi['saveImplementation']>() },
      instanceManager,
    });

    await expect(
      service.saveCreatedCubeCandidate(
        {
          graph,
          markers: [inputMarker, outputMarker],
          markerIds: [inputMarker.id, outputMarker.id],
          filename: 'Text to Image.cube',
        },
        {
          defaultAlias: 'Text to Image',
          cubeId: 'local/personal/Text to Image.cube',
          description: '',
        },
      ),
    ).rejects.toThrow('Unable to serialize graph');

    expect(widgetValue(inputMarker, 'cube_id')).toBe('');
    expect(widgetValue(outputMarker, 'cube_id')).toBe('');
    expect(widgetValue(inputMarker, 'default_alias')).toBe('Text to Image');
    expect(graph._groups).toEqual([]);
    expect(instanceManager.refresh).toHaveBeenNthCalledWith(1, { graph, reason: 'cube-create' });
    expect(instanceManager.refresh).toHaveBeenNthCalledWith(2, {
      graph,
      reason: 'cube-create-rollback',
    });
    expect(instanceManager.scheduleRefresh).not.toHaveBeenCalled();
  });

  test('create action serializes through window app when adapter predates app', async () => {
    const { graph, inputMarker } = makeGraph();
    let saveBody: SavedRequestBody | null = null;
    const adapter = new ComfyAdapter({ window: window as HostWindow });
    window.app = {
      graph,
      canvas: { setDirty: jest.fn() },
      graphToPrompt: () => ({
        output: makePromptFromGraph(graph),
        workflow: { nodes: [], groups: [], version: 1 },
      }),
    };
    const service = new CubeCreationService({
      adapter,
      api: {
        saveImplementation: jest.fn(async (body) => {
          saveBody = parseSavedRequest(body);
          return {
            response: { ok: true },
            data: { saved: [{ cube_id: 'local/personal/Text to Image.cube' }] },
          };
        }),
      },
      toast: { push: jest.fn() },
      dialogs: {
        openCreatePersonalCube: jest.fn(async () => ({
          defaultAlias: 'Text to Image',
          cubeId: 'local/personal/Text to Image.cube',
          description: '',
        })),
      },
      instanceManager: { scheduleRefresh: jest.fn() },
      saveReconciler: { reconcile: jest.fn(async () => {}) },
      cubeBrowser: { refresh: jest.fn(async () => {}) },
    });

    const result = await service.startCreateCubeFromMarker(inputMarker);

    expect(result!.cubeId).toBe('local/personal/Text to Image.cube');
    expect(saveBody!.graph).toEqual(expect.objectContaining({ 2: expect.any(Object) }));
    expect(saveBody!.workflow).toEqual(expect.objectContaining({ version: 1 }));
  });

  test('create preview is transient and never mutates persisted graph state', async () => {
    const { graph, inputMarker, node } = makeGraph();
    const canvas = {
      editor_alpha: 1,
      onDrawBackground: jest.fn(),
      onDrawForeground: jest.fn(),
      setDirty: jest.fn(),
    };
    const originalBackground = canvas.onDrawBackground;
    const originalForeground = canvas.onDrawForeground;
    let resolveModal: (value: CreateCubeValues | null) => void = () => {};
    const service = new CubeCreationService({
      adapter: {
        getWindow: () => window,
        getApp: () => ({
          graph,
          canvas,
        }),
        getConsole: () => ({ error: jest.fn() }),
      },
      dialogs: {
        openCreatePersonalCube: jest.fn(
          () =>
            new Promise<CreateCubeValues | null>((resolve) => {
              resolveModal = resolve;
            }),
        ),
      },
      toast: { push: jest.fn() },
    });

    const resultPromise = service.startCreateCubeFromMarker(inputMarker);
    await Promise.resolve();

    expect(node.boxcolor).toBeUndefined();
    expect(node.color).toBeUndefined();
    expect(graph._groups).toEqual([]);
    expect(graph.add).not.toHaveBeenCalled();
    expect(canvas.onDrawBackground).not.toBe(originalBackground);
    expect(canvas.onDrawForeground).toBe(originalForeground);

    const ctx = makeCanvasContext();
    canvas.onDrawBackground(ctx);

    expect(originalBackground).toHaveBeenCalledWith(ctx);
    expect(originalForeground).not.toHaveBeenCalled();
    expect(ctx.fillText).toHaveBeenCalledWith(
      'Text to Image Preview',
      expect.any(Number),
      expect.any(Number),
    );

    expect(node.boxcolor).toBeUndefined();
    expect(node.color).toBeUndefined();
    expect(graph._groups).toEqual([]);
    expect(graph.remove).not.toHaveBeenCalled();

    resolveModal(null);
    await expect(resultPromise).resolves.toBeNull();
    expect(node.boxcolor).toBeUndefined();
    expect(graph._groups).toEqual([]);
    expect(canvas.onDrawBackground).toBe(originalBackground);
    expect(canvas.onDrawForeground).toBe(originalForeground);
  });

  test('create preview resolves canvas from window app when adapter app is unavailable', async () => {
    const { graph, inputMarker } = makeGraph();
    const canvas = {
      editor_alpha: 1,
      onDrawBackground: null,
      onDrawForeground: null,
      setDirty: jest.fn(),
    };
    window.app = { canvas };
    let resolveModal: (value: CreateCubeValues | null) => void = () => {};
    const service = new CubeCreationService({
      adapter: {
        getWindow: () => window,
        getApp: () => null,
        getCanvas: () => null,
        getConsole: () => ({ error: jest.fn() }),
      },
      dialogs: {
        openCreatePersonalCube: jest.fn(
          () =>
            new Promise<CreateCubeValues | null>((resolve) => {
              resolveModal = resolve;
            }),
        ),
      },
      toast: { push: jest.fn() },
    });

    const resultPromise = service.startCreateCubeFromMarker(inputMarker);
    await Promise.resolve();

    expect(canvas.onDrawBackground).toEqual(expect.any(Function));
    expect(canvas.onDrawForeground).toBeNull();
    expect(canvas.setDirty).toHaveBeenCalledWith(true, true);

    resolveModal(null);
    await expect(resultPromise).resolves.toBeNull();
    expect(canvas.onDrawBackground).toBeNull();
    expect(canvas.onDrawForeground).toBeNull();
  });

  test('create preview uses managed cube bounds for collapsed nodes', () => {
    const originalLiteGraph = globalThis.LiteGraph;
    globalThis.LiteGraph = {
      ...originalLiteGraph,
      NODE_TITLE_HEIGHT: 30,
      NODE_COLLAPSED_WIDTH: 80,
    };
    const graph = { setDirtyCanvas: jest.fn() };
    const canvas = {
      editor_alpha: 1,
      onDrawBackground: null,
      onDrawForeground: null,
      setDirty: jest.fn(),
    };
    const node = {
      id: 12,
      flags: { collapsed: true },
      pos: [200, 90],
      size: [300, 200],
      _collapsed_width: 90,
      getBounding: () => [200, 60, 90, 30],
    };
    const service = new CubeCreationService({
      adapter: {
        getApp: () => ({ graph, canvas }),
      },
    });

    try {
      service.startCreateCubePreview({
        graph,
        defaultAlias: 'Collapsed',
        nodes: [node],
        markers: [],
      });
      const ctx = makeCanvasContext();
      (canvas.onDrawBackground as unknown as (context: CanvasRenderingContext2D) => void)(ctx);

      expect(ctx.rect).toHaveBeenCalledWith(190.5, 0.5, 110, expect.any(Number));
      expect(ctx.rect).toHaveBeenCalledWith(190.5, 0.5, 110, 100);
      expect(ctx.fillText).toHaveBeenCalledWith('Collapsed Preview', 194, 24);
    } finally {
      service.clearCreateCubePreview();
      globalThis.LiteGraph = originalLiteGraph;
    }
  });

  test('create modal backdrop drag pans graph viewport only', async () => {
    const canvas = {
      ds: { offset: [0, 0], scale: 2, changeScale: jest.fn() },
      setDirty: jest.fn(),
      processMouseDown: jest.fn(),
      processMouseMove: jest.fn(),
    };
    const { promise, overlay } = openCreateModalWithCanvas(canvas);
    overlay.setPointerCapture = jest.fn();
    overlay.releasePointerCapture = jest.fn();

    expect(overlay.style.cursor).toBe('grab');
    overlay.dispatchEvent(makePointerEvent('pointerdown', { clientX: 100, clientY: 100 }));
    expect(overlay.style.cursor).toBe('grabbing');
    overlay.dispatchEvent(makePointerEvent('pointermove', { clientX: 120, clientY: 80 }));
    overlay.dispatchEvent(makePointerEvent('pointerup', { clientX: 120, clientY: 80 }));

    expect(canvas.ds.offset).toEqual([10, -10]);
    expect(canvas.setDirty).toHaveBeenCalledWith(true, true);
    expect(canvas.processMouseDown).not.toHaveBeenCalled();
    expect(canvas.processMouseMove).not.toHaveBeenCalled();
    expect(overlay.setPointerCapture).toHaveBeenCalledWith(1);
    expect(overlay.releasePointerCapture).toHaveBeenCalledWith(1);
    expect(overlay.style.cursor).toBe('grab');

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('create modal backdrop drag keeps panning when pointer moves retarget to window', async () => {
    const canvas = {
      ds: { offset: [0, 0], scale: 2, changeScale: jest.fn() },
      setDirty: jest.fn(),
    };
    const { promise, overlay } = openCreateModalWithCanvas(canvas);
    overlay.setPointerCapture = jest.fn();
    overlay.releasePointerCapture = jest.fn();

    overlay.dispatchEvent(makePointerEvent('pointerdown', { clientX: 100, clientY: 100 }));
    window.dispatchEvent(makePointerEvent('pointermove', { clientX: 120, clientY: 80 }));
    window.dispatchEvent(makePointerEvent('pointerup', { clientX: 120, clientY: 80 }));

    expect(canvas.ds.offset).toEqual([10, -10]);
    expect(canvas.setDirty).toHaveBeenCalledWith(true, true);
    expect(overlay.releasePointerCapture).toHaveBeenCalledWith(1);

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('create modal backdrop supports middle mouse panning', async () => {
    const canvas = {
      ds: { offset: [0, 0], scale: 2, changeScale: jest.fn() },
      setDirty: jest.fn(),
    };
    const { promise, overlay } = openCreateModalWithCanvas(canvas);
    overlay.setPointerCapture = jest.fn();
    overlay.releasePointerCapture = jest.fn();

    overlay.dispatchEvent(
      makePointerEvent('pointerdown', { clientX: 100, clientY: 100, button: 1 }),
    );
    window.dispatchEvent(makePointerEvent('pointermove', { clientX: 120, clientY: 80, button: 1 }));
    window.dispatchEvent(makePointerEvent('pointerup', { clientX: 120, clientY: 80, button: 1 }));

    expect(canvas.ds.offset).toEqual([10, -10]);
    expect(canvas.setDirty).toHaveBeenCalledWith(true, true);

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('create modal backdrop wheel zooms without calling LiteGraph wheel handling', async () => {
    const canvas = {
      zoom_speed: 1.1,
      ds: { offset: [0, 0], scale: 1, changeScale: jest.fn() },
      setDirty: jest.fn(),
      processMouseWheel: jest.fn(),
    };
    const { promise, overlay } = openCreateModalWithCanvas(canvas);
    const wheel = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: 150,
      clientY: 175,
      deltaY: -100,
    });

    overlay.dispatchEvent(wheel);

    expect(wheel.defaultPrevented).toBe(true);
    expect(canvas.ds.changeScale).toHaveBeenCalledWith(1.1, [150, 175], false);
    expect(canvas.processMouseWheel).not.toHaveBeenCalled();
    expect(canvas.setDirty).toHaveBeenCalledWith(true, true);

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('create modal graph navigator uses active LiteGraph canvas when adapter has no canvas', async () => {
    const canvas = {
      zoom_speed: 1.2,
      ds: { offset: [0, 0], scale: 1, changeScale: jest.fn() },
      setDirty: jest.fn(),
    };
    window.LGraphCanvas = { active_canvas: canvas };
    const { promise, overlay } = openCreateModalWithAdapter({ getCanvas: () => null });
    const wheel = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: 140,
      clientY: 165,
      deltaY: -100,
    });

    overlay.dispatchEvent(wheel);

    expect(canvas.ds.changeScale).toHaveBeenCalledWith(1.2, [140, 165], false);
    expect(canvas.setDirty).toHaveBeenCalledWith(true, true);

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('create modal graph navigator leaves dialog interactions alone', async () => {
    const canvas = {
      ds: { offset: [0, 0], scale: 1, changeScale: jest.fn() },
      setDirty: jest.fn(),
    };
    const { promise, dialog } = openCreateModalWithCanvas(canvas);
    const wheel = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: 150,
      clientY: 175,
      deltaY: -100,
    });

    dialog.dispatchEvent(makePointerEvent('pointerdown', { clientX: 100, clientY: 100 }));
    dialog.dispatchEvent(wheel);

    expect(canvas.ds.offset).toEqual([0, 0]);
    expect(canvas.ds.changeScale).not.toHaveBeenCalled();
    expect(canvas.setDirty).not.toHaveBeenCalled();
    expect(wheel.defaultPrevented).toBe(false);
    expect(dialog.style.cursor).toBe('auto');

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('create modal blocks backdrop context menu and double click', async () => {
    const canvas = {
      ds: { offset: [0, 0], scale: 1, changeScale: jest.fn() },
      setDirty: jest.fn(),
    };
    const { promise, overlay } = openCreateModalWithCanvas(canvas);
    const contextMenu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    const doubleClick = new MouseEvent('dblclick', { bubbles: true, cancelable: true });

    overlay.dispatchEvent(contextMenu);
    overlay.dispatchEvent(doubleClick);

    expect(contextMenu.defaultPrevented).toBe(true);
    expect(doubleClick.defaultPrevented).toBe(true);

    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();
  });

  test('create modal graph navigator detaches after close', async () => {
    const canvas = {
      zoom_speed: 1.1,
      ds: { offset: [0, 0], scale: 1, changeScale: jest.fn() },
      setDirty: jest.fn(),
    };
    const { promise, overlay } = openCreateModalWithCanvas(canvas);

    expect(overlay.style.cursor).toBe('grab');
    requiredElement('.sugarcubes-create-cube-dialog button:first-child').click();
    await expect(promise).resolves.toBeNull();

    expect(overlay.style.cursor).toBe('');
    overlay.dispatchEvent(
      new WheelEvent('wheel', {
        bubbles: true,
        cancelable: true,
        clientX: 150,
        clientY: 175,
        deltaY: -100,
      }),
    );

    expect(canvas.ds.changeScale).not.toHaveBeenCalled();
    expect(canvas.setDirty).not.toHaveBeenCalled();
  });
});
