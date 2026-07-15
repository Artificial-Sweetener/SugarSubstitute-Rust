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
import { describe, expect, test, beforeEach, jest } from '@jest/globals';
import { app as hostApp } from '/scripts/app.js';
import { api as hostApi } from '/scripts/api.js';
import type { SugarCubesUI } from '../../frontend/comfyui/ui/SugarCubesUI.js';
import type { UnknownRecord } from '../../frontend/comfyui/ui/types/common.js';
import type { ComfyGroup, ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

let app = hostApp as unknown as (typeof import('./mocks/app.js'))['app'];
let api = hostApi as unknown as (typeof import('./mocks/api.js'))['api'];

const CANONICAL_DEMO_ID = 'artificial-sweetener/base-cubes/demo.cube';
const PERSONAL_DEMO_ID = 'local/personal/Demo Cube.cube';
const PERSONAL_LOOSE_ID = 'local/personal/Loose Cube.cube';
const PERSONAL_DEMO_FORK_ID = 'local/personal/Demo (Fork).cube';
const HISTORICAL_REVISION_REF = 'abc123456789';
const CURRENT_REVISION_REF = 'WORKTREE';
let loadedUi: SugarCubesUI | null = null;

function getLoadedUi(): SugarCubesUI {
  if (!loadedUi) throw new Error('SugarCubes UI module is not loaded');
  return loadedUi;
}

interface MarkerOptions {
  id: number;
  type: string;
  cubeId: string;
  defaultAlias: string;
  version: string;
  revisionRef: string;
}

interface CubeGroupOptions {
  cubeId: string;
  defaultAlias: string;
  version: string;
  revisionRef: string;
  markerIds: [number, number];
  instanceId: string;
}

interface SavedWorkflow extends UnknownRecord {
  nodes?: unknown[];
  definitions?: { subgraphs?: SavedSubgraph[] };
}

interface SavedSubgraph extends UnknownRecord {
  id?: unknown;
  name?: unknown;
  inputNode?: unknown;
  outputNode?: unknown;
  inputs?: UnknownRecord[];
  nodes?: unknown[];
}

interface SavePayload extends UnknownRecord {
  cubes: UnknownRecord[];
  workflow?: SavedWorkflow;
  workflow_version?: unknown;
}

interface TestMarker extends ComfyNode {
  id: number;
  type: string;
  widgets: Array<{ name: string; value: string }>;
  properties: {
    sugarcubes_cube_version: string;
    sugarcubes_cube_revision_ref: string;
  };
}

interface TestCubeMetadata extends UnknownRecord {
  managed: boolean;
  cube_id: string;
  default_alias: string;
  cube_version: string;
  cube_revision_ref: string;
  cube_definition_key: string;
  markers: { inputs: number[]; outputs: number[] };
  nodes: number[];
  instance_id: string;
}

interface TestCubeGroup extends ComfyGroup {
  title: string;
  properties: { sugarcubes: TestCubeMetadata };
}

function parseSavePayload(body: BodyInit | null | undefined): SavePayload {
  if (typeof body !== 'string') throw new Error('Expected a JSON save request body');
  const payload = JSON.parse(body) as SavePayload;
  if (!Array.isArray(payload.cubes)) payload.cubes = [];
  return payload;
}

function requireSavePayload(payload: SavePayload | null): SavePayload {
  if (!payload) throw new Error('Save request was not captured');
  return payload;
}

function requireSavedWorkflow(payload: SavePayload | null): SavedWorkflow {
  const workflow = requireSavePayload(payload).workflow;
  if (!workflow) throw new Error('Save request did not include a workflow');
  return workflow;
}

function requireSavedSubgraphs(payload: SavePayload | null): SavedSubgraph[] {
  const subgraphs = requireSavedWorkflow(payload).definitions?.subgraphs;
  if (!subgraphs) throw new Error('Save request did not include subgraph definitions');
  return subgraphs;
}

function requireSavedCube(payload: SavePayload | null, index = 0): UnknownRecord {
  const cube = requireSavePayload(payload).cubes[index];
  if (!cube) throw new Error(`Save request did not include cube ${index}`);
  return cube;
}

function setupBaseApp() {
  app.reset();
  app.graph = { _nodes: [], _groups: [] };
  app.canvas = {
    graph: app.graph,
    setDirty: jest.fn(),
    onAfterChange: () => {},
    onDrawForeground: () => {},
    onDrawBackground: () => {},
    processMouseMove: () => {},
    processMouseDown: () => {},
  };
  app.extensionManager = { registerSidebarTab: () => {} };
  app.clean = () => {};
}

async function loadUi() {
  const loaded: { value?: typeof import('../../frontend/comfyui/ui.js') } = {};
  await jest.isolateModulesAsync(async () => {
    loaded.value = await import('../../frontend/comfyui/ui.js');
  });
  if (!loaded.value) throw new Error('SugarCubes UI module did not load');
  const { sugarCubesExtension, sugarCubesUI } = loaded.value;
  loadedUi = sugarCubesUI;
  const runtimeApp = sugarCubesUI.adapter.getApp() as unknown as typeof app;
  const runtimeApi = sugarCubesUI.adapter.getApi() as unknown as typeof api;
  if (runtimeApp !== app) {
    Object.assign(runtimeApp, app);
    app = runtimeApp;
  }
  if (runtimeApi !== api) {
    Object.assign(runtimeApi, api);
    api = runtimeApi;
  }
  if (!app._extensions.includes(sugarCubesExtension)) {
    app.registerExtension(sugarCubesExtension);
  }
  return sugarCubesExtension;
}

async function flushPromises() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

async function setupRegisteredExtension(): Promise<void> {
  const extension = app._extensions[0];
  if (!extension?.setup) throw new Error('SugarCubes extension did not register setup');
  await extension.setup();
}

function makeMarker({
  id,
  type,
  cubeId,
  defaultAlias,
  version,
  revisionRef,
}: MarkerOptions): TestMarker {
  return {
    id,
    type,
    pos: [0, 0],
    size: [140, 46],
    inputs: [],
    outputs: [],
    properties: {
      sugarcubes_cube_version: version,
      sugarcubes_cube_revision_ref: revisionRef,
    },
    widgets: [
      { name: 'cube_id', value: cubeId },
      { name: 'default_alias', value: defaultAlias },
    ],
    graph: app.graph,
  };
}

function makeCubeGroup({
  cubeId,
  defaultAlias,
  version,
  revisionRef,
  markerIds,
  instanceId,
}: CubeGroupOptions): TestCubeGroup {
  return {
    title: defaultAlias,
    properties: {
      sugarcubes: {
        managed: true,
        cube_id: cubeId,
        default_alias: defaultAlias,
        cube_version: version,
        cube_revision_ref: revisionRef,
        cube_definition_key: `${cubeId}@${version}`,
        markers: {
          inputs: [markerIds[0]],
          outputs: [markerIds[1]],
        },
        nodes: [],
        instance_id: instanceId,
      },
    },
  };
}

function makeFinalizedDefinition(cubeId: string, version: string): UnknownRecord {
  return {
    cube: {
      cube_id: cubeId,
      default_alias: 'Demo',
      version,
      surface: null,
      flavors: { authored: [{ id: 'default', name: 'Default', values: {} }] },
    },
    nodes: [],
    markers: [],
    connections: [],
    layout: { groups: [] },
  };
}

beforeEach(() => {
  jest.resetModules();
  loadedUi = null;
  jest.restoreAllMocks();
  setupBaseApp();
  localStorage.clear();
  document.body.innerHTML = '';
  globalThis.requestAnimationFrame = () => 1;
  globalThis.cancelAnimationFrame = () => {};
  window.requestAnimationFrame = globalThis.requestAnimationFrame;
  window.cancelAnimationFrame = globalThis.cancelAnimationFrame;
  window.SugarCubes = {} as SugarCubesPublicApi;
  window.alert = () => {};
  window.comfyAPI = { vueApp: { config: { globalProperties: { $toast: null } } } };
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = () => {};
  }
  HTMLCanvasElement.prototype.getContext = (() => ({
    clearRect: () => {},
    fillRect: () => {},
    strokeRect: () => {},
    beginPath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    arc: () => {},
    fill: () => {},
    stroke: () => {},
    save: () => {},
    restore: () => {},
    setLineDash: () => {},
    drawImage: () => {},
    fillText: () => {},
  })) as unknown as typeof HTMLCanvasElement.prototype.getContext;
  class TestLGraphCanvas {
    drawConnections(): void {}
    drawForeground(): void {}
  }
  class TestLGraphGroup {}
  globalThis.LiteGraph = {
    LGraphCanvas: TestLGraphCanvas,
    LGraphGroup: TestLGraphGroup,
    LinkDirection: { LEFT: 3, RIGHT: 4 },
  } as unknown as LiteGraphHost;
});

describe('save workflow payload', () => {
  const configureGraphToPrompt = ({ withWorkflow }: { withWorkflow: boolean }): void => {
    app.graphToPrompt = () => {
      const output = {
        1: { class_type: 'KSampler', inputs: {}, _meta: { title: 'Node' } },
      };
      const workflow = withWorkflow
        ? {
            nodes: [{ id: 1, type: 'KSampler', pos: [10, 20], size: [180, 60] }],
            groups: [],
            extra: { ds: { scale: 1, offset: [0, 0] } },
            version: 1,
          }
        : null;
      return { output, workflow };
    };
  };

  test('save includes workflow payload', async () => {
    configureGraphToPrompt({ withWorkflow: true });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [];

    await ui.cubeSave.save();
    await flushPromises();

    expect(saveBody).not.toBeNull();
    expect(Array.isArray(requireSavedWorkflow(saveBody).nodes)).toBe(true);
    expect(requireSavePayload(saveBody).workflow_version).toBe(1);
  });

  test('save includes target metadata from browser catalog entries', async () => {
    const targetCubeId = 'artificial-sweetener/base-cubes/SDXL/demo.cube';
    configureGraphToPrompt({ withWorkflow: true });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([targetCubeId]);
    ui.cubeBrowser.getCubes = () => [
      {
        cube_id: targetCubeId,
        target_model: 'SDXL',
        supported_models: ['SD 1.5'],
        is_writable: true,
      },
    ];

    await ui.cubeSave.save();
    await flushPromises();

    expect(requireSavedCube(saveBody).metadata).toEqual({
      default_alias: 'SDXL/demo',
      target_model: 'SDXL',
      supported_models: ['SDXL', 'SD 1.5'],
    });
  });

  test('save enriches workflow subgraphs from live graph state', async () => {
    const subgraphId = '94f725d5-39bf-4060-be68-f573214a2055';
    app.graph!._subgraphs = new Map([
      [
        subgraphId,
        {
          asSerialisable: () => ({
            id: subgraphId,
            name: 'Schedule & Encode Prompts',
            inputNode: { id: -10, bounding: [0, 0, 75, 100] },
            outputNode: { id: -20, bounding: [240, 0, 75, 100] },
            inputs: [
              {
                id: `${subgraphId}:input:0`,
                name: 'value',
                label: 'Prompt Text',
                type: 'STRING',
                linkIds: [1],
              },
            ],
            outputs: [
              { id: `${subgraphId}:output:0`, name: 'STRING', type: 'STRING', linkIds: [2] },
            ],
            widgets: [],
            version: 1,
            revision: 0,
            state: { lastGroupId: 0, lastNodeId: 1, lastLinkId: 2, lastRerouteId: 0 },
            nodes: [{ id: 1, type: 'KSampler' }],
            links: [
              {
                id: 1,
                origin_id: -10,
                origin_slot: 0,
                target_id: 1,
                target_slot: 0,
                type: 'STRING',
              },
            ],
            groups: [],
            extra: {},
            config: {},
          }),
          serialize: () => ({
            id: subgraphId,
            nodes: [{ id: 1, type: 'KSampler' }],
            links: [[1, 1, 0, 2, 0, 'ANY']],
            inputs: [],
            outputs: [],
          }),
        },
      ],
    ]);
    app.graphToPrompt = () => ({
      output: {
        1: { class_type: 'KSampler', inputs: {}, _meta: { title: 'Node' } },
      },
      workflow: {
        nodes: [{ id: 1, type: subgraphId, pos: [10, 20], size: [180, 60] }],
        groups: [],
        extra: { ds: { scale: 1, offset: [0, 0] } },
        version: 1,
        definitions: {
          subgraphs: [{ id: subgraphId, nodes: [], links: [], inputs: [], outputs: [] }],
        },
      },
    });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [];

    await ui.cubeSave.save();
    await flushPromises();

    const subgraphs = requireSavedSubgraphs(saveBody);
    expect(Array.isArray(subgraphs)).toBe(true);
    const subgraph = subgraphs[0];
    if (!subgraph) throw new Error('Missing serialized subgraph');
    expect(subgraph.id).toBe(subgraphId);
    expect(subgraph.name).toBe('Schedule & Encode Prompts');
    expect(subgraph.inputNode).toEqual({ id: -10, bounding: [0, 0, 75, 100], pinned: false });
    expect(subgraph.outputNode).toEqual({
      id: -20,
      bounding: [240, 0, 75, 100],
      pinned: false,
    });
    expect(subgraph.inputs?.[0]).toEqual(
      expect.objectContaining({ name: 'value', label: 'Prompt Text' }),
    );
    expect(Array.isArray(subgraph.nodes)).toBe(true);
    expect(subgraph.nodes?.length).toBe(1);
  });

  test('save drops subgraphs with empty ids during normalization', async () => {
    const validSubgraphId = '94f725d5-39bf-4060-be68-f573214a2055';
    app.graph!._subgraphs = new Map([
      [
        '',
        {
          serialize: () => ({
            id: '',
            nodes: [{ id: 1, type: 'KSampler' }],
            links: [],
            inputs: [],
            outputs: [],
          }),
        },
      ],
      [
        validSubgraphId,
        {
          serialize: () => ({
            id: validSubgraphId,
            nodes: [{ id: 2, type: 'KSampler' }],
            links: [],
            inputs: [],
            outputs: [],
          }),
        },
      ],
    ]);
    app.graphToPrompt = () => ({
      output: {
        1: { class_type: 'KSampler', inputs: {}, _meta: { title: 'Node' } },
      },
      workflow: {
        nodes: [{ id: 1, type: validSubgraphId, pos: [10, 20], size: [180, 60] }],
        groups: [],
        extra: { ds: { scale: 1, offset: [0, 0] } },
        version: 1,
        definitions: {
          subgraphs: [{ id: '', nodes: [], links: [], inputs: [], outputs: [] }],
        },
      },
    });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [];

    await ui.cubeSave.save();
    await flushPromises();

    const subgraphIds = (requireSavePayload(saveBody).workflow?.definitions?.subgraphs || []).map(
      (entry) => entry.id,
    );
    expect(subgraphIds).toContain(validSubgraphId);
    expect(subgraphIds).not.toContain('');
  });

  test('save assigns cube_id for unsaved cubes', async () => {
    configureGraphToPrompt({ withWorkflow: true });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    app.graph = { _nodes: [], _groups: [], links: {} };
    const defaultAlias = 'Demo Cube';
    const node = {
      id: 1,
      type: 'KSampler',
      pos: [0, 0],
      size: [140, 60],
      inputs: [{ name: 'value', link: 1 }],
      outputs: [{ type: 'IMAGE', links: [2] }],
      graph: app.graph,
    };
    const inputMarker = {
      id: 2,
      type: 'SugarCubes.CubeInput',
      pos: [0, 0],
      size: [140, 46],
      inputs: [{ name: 'value', link: null }],
      outputs: [{ type: 'IMAGE', links: [1] }],
      widgets: [
        { name: 'cube_id', value: '' },
        { name: 'default_alias', value: defaultAlias },
      ],
      graph: app.graph,
    };
    const outputMarker = {
      id: 3,
      type: 'SugarCubes.CubeOutput',
      pos: [0, 0],
      size: [140, 46],
      inputs: [{ type: 'IMAGE', link: 2 }],
      outputs: [{ type: 'IMAGE', links: [] }],
      widgets: [
        { name: 'cube_id', value: '' },
        { name: 'default_alias', value: defaultAlias },
      ],
      graph: app.graph,
    };
    app.graph._nodes = [node, inputMarker, outputMarker];
    app.graph.links = {
      1: {
        id: 1,
        origin_id: inputMarker.id,
        origin_slot: 0,
        target_id: node.id,
        target_slot: 0,
        type: 'IMAGE',
      },
      2: {
        id: 2,
        origin_id: node.id,
        origin_slot: 0,
        target_id: outputMarker.id,
        target_slot: 0,
        type: 'IMAGE',
      },
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set();
    ui.cubeBrowser.getCubes = () => [];
    await ui.cubeSave.save();

    const cubeIdWidget = inputMarker.widgets.find((widget) => widget.name === 'cube_id');
    const assignedCubeId = cubeIdWidget?.value || '';
    expect(assignedCubeId).toBe(PERSONAL_DEMO_ID);
    expect(requireSavedCube(saveBody).cube_id).toBe(PERSONAL_DEMO_ID);
  });

  test('save assigns cube_id when markers are disconnected', async () => {
    configureGraphToPrompt({ withWorkflow: true });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    app.graph = { _nodes: [], _groups: [], links: {} };
    const defaultAlias = 'Loose Cube';
    const inputMarker = {
      id: 2,
      type: 'SugarCubes.CubeInput',
      pos: [0, 0],
      size: [140, 46],
      inputs: [{ name: 'value', link: null }],
      outputs: [{ type: 'IMAGE', links: [] }],
      widgets: [
        { name: 'cube_id', value: '' },
        { name: 'default_alias', value: defaultAlias },
      ],
      graph: app.graph,
    };
    const outputMarker = {
      id: 3,
      type: 'SugarCubes.CubeOutput',
      pos: [0, 0],
      size: [140, 46],
      inputs: [{ type: 'IMAGE', link: null }],
      outputs: [{ type: 'IMAGE', links: [] }],
      widgets: [
        { name: 'cube_id', value: '' },
        { name: 'default_alias', value: defaultAlias },
      ],
      graph: app.graph,
    };
    app.graph._nodes = [inputMarker, outputMarker];
    app.graph.links = {};

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set();
    ui.cubeBrowser.getCubes = () => [];
    await ui.cubeSave.save();

    const assignedCubeId =
      inputMarker.widgets.find((widget) => widget.name === 'cube_id')?.value || '';
    expect(assignedCubeId).toBe(PERSONAL_LOOSE_ID);
    expect(requireSavedCube(saveBody).cube_id).toBe(PERSONAL_LOOSE_ID);
  });

  test('save reports missing workflow payload', async () => {
    configureGraphToPrompt({ withWorkflow: false });

    api.fetchApi = async () => ({ ok: true, json: async () => ({ saved: [] }) });
    const errorSpy = jest.spyOn(console, 'error').mockImplementation(() => {});

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [];

    await ui.cubeSave.save();
    await flushPromises();

    expect(errorSpy).toHaveBeenCalled();
    const message = String(errorSpy.mock.calls[0]?.[0] || '');
    expect(message).toContain('Workflow payload unavailable');
    errorSpy.mockRestore();
  });

  test('save delegates finalized graph state to the save reconciler', async () => {
    configureGraphToPrompt({ withWorkflow: true });

    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        const body = parseSavePayload(options.body);
        return {
          ok: true,
          json: async () => ({
            saved: body.cubes.map((entry) => ({
              cube_id: entry.cube_id,
              default_alias: entry.cube_id,
              path: 'E:\\\\ComfyUI\\\\custom_nodes\\\\ComfyUI-SugarCubes\\\\cubes\\\\demo.cube',
              forked: false,
            })),
          }),
        };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [];
    const reconcileSpy = jest
      .spyOn(ui.saveReconciler, 'reconcile')
      .mockResolvedValue({ cubeIds: [], entries: [] });

    await ui.cubeSave.save();
    await flushPromises();

    expect(reconcileSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        graph: app.graph,
        fallbackCubeIds: [CANONICAL_DEMO_ID],
        reason: 'save',
      }),
    );
  });

  test('save toast reports committed entries distinctly', async () => {
    const toastAdd = jest.fn();
    window.comfyAPI = { vueApp: { config: { globalProperties: { $toast: { add: toastAdd } } } } };
    configureGraphToPrompt({ withWorkflow: true });

    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        const body = parseSavePayload(options.body);
        return {
          ok: true,
          json: async () => ({
            saved: body.cubes.map((entry) => ({
              cube_id: entry.cube_id,
              default_alias: 'demo.cube',
              path: 'E:\\ComfyUI\\custom_nodes\\ComfyUI-SugarCubes\\.sugarcubes\\local\\personal\\demo.cube',
              forked: false,
              committed: true,
              commit_sha: 'abcdef1234567890',
              commit_short_sha: 'abcdef1',
              commit_message: 'update demo.cube v1.0.1',
              commit_error: '',
            })),
          }),
        };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [];

    await ui.cubeSave.save();
    await flushPromises();

    expect(toastAdd).toHaveBeenCalledWith(
      expect.objectContaining({
        severity: 'success',
        summary: 'SugarCubes exported',
        detail: expect.stringContaining('saved and committed: demo.cube'),
      }),
    );
    expect(toastAdd).toHaveBeenCalledWith(
      expect.objectContaining({
        detail: expect.stringContaining('abcdef1: update demo.cube v1.0.1'),
      }),
    );
  });

  test('version-bumped save updates only the saved instance chrome metadata', async () => {
    app.extensionManager.registerSidebarTab = () => {};
    configureGraphToPrompt({ withWorkflow: true });

    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/save_implementation') {
        return {
          ok: true,
          json: async () => ({
            saved: [
              {
                cube_id: CANONICAL_DEMO_ID,
                default_alias: 'Demo',
                path: 'demo.cube',
                forked: false,
                committed: true,
                commit_message: 'update demo.cube v1.1.1',
                version: '1.1.1',
                definition: makeFinalizedDefinition(CANONICAL_DEMO_ID, '1.1.1'),
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    app.graph = { _nodes: [], _groups: [], links: {} };
    const savedInput = makeMarker({
      id: 101,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
    });
    const savedOutput = makeMarker({
      id: 102,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
    });
    const untouchedInput = makeMarker({
      id: 201,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
    });
    const untouchedOutput = makeMarker({
      id: 202,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
    });
    const savedGroup = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
      markerIds: [101, 102],
      instanceId: 'saved-instance',
    });
    const untouchedGroup = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
      markerIds: [201, 202],
      instanceId: 'untouched-instance',
    });
    app.graph._nodes = [savedInput, savedOutput, untouchedInput, untouchedOutput];
    app.graph._groups = [savedGroup, untouchedGroup];

    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      { cube_id: CANONICAL_DEMO_ID, name: 'Demo', version: '1.1.0', is_writable: true },
    ];
    ui.instanceManager.scheduleRefresh = jest.fn();

    await ui.cubeSave.save();

    expect(savedGroup.properties.sugarcubes).toEqual(
      expect.objectContaining({
        cube_version: '1.1.1',
        cube_revision_ref: CURRENT_REVISION_REF,
        cube_definition_key: `${CANONICAL_DEMO_ID}@1.1.1`,
      }),
    );
    expect(savedInput.properties.sugarcubes_cube_version).toBe('1.1.1');
    expect(savedInput.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(savedOutput.properties.sugarcubes_cube_version).toBe('1.1.1');
    expect(savedOutput.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(untouchedGroup.properties.sugarcubes).toEqual(
      expect.objectContaining({
        cube_version: '1.1.0',
        cube_revision_ref: CURRENT_REVISION_REF,
        cube_definition_key: `${CANONICAL_DEMO_ID}@1.1.0`,
      }),
    );
    expect(untouchedInput.properties.sugarcubes_cube_version).toBe('1.1.0');
    expect(untouchedInput.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(untouchedOutput.properties.sugarcubes_cube_version).toBe('1.1.0');
    expect(untouchedOutput.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(ui.instanceManager.scheduleRefresh).not.toHaveBeenCalled();
  });

  test('versionless save response leaves chrome metadata unchanged', async () => {
    app.extensionManager.registerSidebarTab = () => {};
    configureGraphToPrompt({ withWorkflow: true });

    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/save_implementation') {
        return {
          ok: true,
          json: async () => ({
            saved: [
              {
                cube_id: CANONICAL_DEMO_ID,
                default_alias: 'Demo',
                path: 'demo.cube',
                forked: false,
                committed: false,
                commit_message: '',
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    app.graph = { _nodes: [], _groups: [], links: {} };
    const inputMarker = makeMarker({
      id: 101,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
    });
    const outputMarker = makeMarker({
      id: 102,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
    });
    const group = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.1.0',
      revisionRef: CURRENT_REVISION_REF,
      markerIds: [101, 102],
      instanceId: 'saved-instance',
    });
    app.graph._nodes = [inputMarker, outputMarker];
    app.graph._groups = [group];

    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      { cube_id: CANONICAL_DEMO_ID, name: 'Demo', version: '1.1.0', is_writable: true },
    ];
    ui.instanceManager.scheduleRefresh = jest.fn();

    await ui.cubeSave.save();

    expect(group.properties.sugarcubes).toEqual(
      expect.objectContaining({
        cube_version: '1.1.0',
        cube_revision_ref: CURRENT_REVISION_REF,
        cube_definition_key: `${CANONICAL_DEMO_ID}@1.1.0`,
      }),
    );
    expect(inputMarker.properties.sugarcubes_cube_version).toBe('1.1.0');
    expect(inputMarker.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(outputMarker.properties.sugarcubes_cube_version).toBe('1.1.0');
    expect(outputMarker.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(ui.instanceManager.scheduleRefresh).not.toHaveBeenCalledWith({
      graph: app.graph,
      reason: 'save-version-metadata',
    });
  });

  test('save forks read-only tracked cubes even when author metadata matches', async () => {
    configureGraphToPrompt({ withWorkflow: true });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    app.graph = { _nodes: [], _groups: [], links: {} };
    const inputMarker = {
      id: 2,
      type: 'SugarCubes.CubeInput',
      pos: [0, 0],
      size: [140, 46],
      inputs: [{ name: 'value', link: null }],
      outputs: [{ type: 'IMAGE', links: [] }],
      widgets: [
        { name: 'cube_id', value: CANONICAL_DEMO_ID },
        { name: 'default_alias', value: 'Demo' },
      ],
      graph: app.graph,
    };
    const outputMarker = {
      id: 3,
      type: 'SugarCubes.CubeOutput',
      pos: [0, 0],
      size: [140, 46],
      inputs: [{ type: 'IMAGE', link: null }],
      outputs: [{ type: 'IMAGE', links: [] }],
      widgets: [
        { name: 'cube_id', value: CANONICAL_DEMO_ID },
        { name: 'default_alias', value: 'Demo' },
      ],
      graph: app.graph,
    };
    app.graph._nodes = [inputMarker, outputMarker];
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      {
        cube_id: CANONICAL_DEMO_ID,
        name: 'Demo',
        author: 'Tester',
        author_url: '',
        is_writable: false,
        write_block_reason: 'Tracked GitHub repos are read-only until you claim one GitHub owner.',
      },
    ];
    await ui.cubeSave.save();
    await flushPromises();

    expect(requireSavePayload(saveBody).cubes).toEqual([
      expect.objectContaining({
        cube_id: PERSONAL_DEMO_FORK_ID,
        forked: true,
        previous_cube_id: CANONICAL_DEMO_ID,
      }),
    ]);
  });

  test('current save does not show historical save modal', async () => {
    app.extensionManager.registerSidebarTab = () => {};
    configureGraphToPrompt({ withWorkflow: true });

    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        const body = parseSavePayload(options.body);
        return {
          ok: true,
          json: async () => ({
            saved: body.cubes.map((entry) => ({
              cube_id: entry.cube_id,
              default_alias: 'Demo',
              path: 'demo.cube',
              forked: false,
              version: '1.2.2',
            })),
          }),
        };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      { cube_id: CANONICAL_DEMO_ID, name: 'Demo', version: '1.2.1', is_writable: true },
    ];
    ui.dialogs.chooseHistoricalVersionSaveAction = jest.fn(async () => 'latest');

    await ui.cubeSave.save();

    expect(ui.dialogs.chooseHistoricalVersionSaveAction).not.toHaveBeenCalled();
  });

  test('stale historical save as latest sends source metadata and refreshes graph identity', async () => {
    app.extensionManager.registerSidebarTab = () => {};
    configureGraphToPrompt({ withWorkflow: true });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return {
          ok: true,
          json: async () => ({
            saved: [
              {
                cube_id: CANONICAL_DEMO_ID,
                default_alias: 'Demo',
                path: 'demo.cube',
                forked: false,
                commit_message: 'update demo.cube v1.2.2',
                version: '1.2.2',
                definition: makeFinalizedDefinition(CANONICAL_DEMO_ID, '1.2.2'),
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    app.graph = { _nodes: [], _groups: [], links: {} };
    const inputMarker = makeMarker({
      id: 2,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const outputMarker = makeMarker({
      id: 3,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const group = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
      markerIds: [2, 3],
      instanceId: 'inst-old',
    });
    app.graph._nodes = [inputMarker, outputMarker];
    app.graph._groups = [group];

    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      { cube_id: CANONICAL_DEMO_ID, name: 'Demo', version: '1.2.1', is_writable: true },
    ];
    ui.dialogs.chooseHistoricalVersionSaveAction = jest.fn(async () => 'latest');

    await ui.cubeSave.save();

    expect(ui.dialogs.chooseHistoricalVersionSaveAction).toHaveBeenCalledWith({
      entries: [
        expect.objectContaining({
          cubeId: CANONICAL_DEMO_ID,
          defaultAlias: 'Demo',
          sourceVersion: '1.0.1',
          sourceRevisionRef: HISTORICAL_REVISION_REF,
        }),
      ],
    });
    expect(requireSavedCube(saveBody)).toEqual(
      expect.objectContaining({
        cube_id: CANONICAL_DEMO_ID,
        source_revision_ref: HISTORICAL_REVISION_REF,
        source_version: '1.0.1',
        source_definition_key: `${CANONICAL_DEMO_ID}@1.0.1`,
        stale_save_mode: 'latest',
      }),
    );
    expect(group.properties.sugarcubes).toEqual(
      expect.objectContaining({
        cube_id: CANONICAL_DEMO_ID,
        cube_version: '1.2.2',
        cube_revision_ref: CURRENT_REVISION_REF,
        cube_definition_key: `${CANONICAL_DEMO_ID}@1.2.2`,
      }),
    );
    expect(inputMarker.properties.sugarcubes_cube_version).toBe('1.2.2');
    expect(inputMarker.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(outputMarker.properties.sugarcubes_cube_version).toBe('1.2.2');
    expect(outputMarker.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
  });

  test('stale historical save cancel skips backend save and graph mutation', async () => {
    app.extensionManager.registerSidebarTab = () => {};
    configureGraphToPrompt({ withWorkflow: true });

    let saveCalls = 0;
    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/save_implementation') {
        saveCalls += 1;
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    app.graph = { _nodes: [], _groups: [], links: {} };
    const inputMarker = makeMarker({
      id: 2,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const outputMarker = makeMarker({
      id: 3,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const group = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
      markerIds: [2, 3],
      instanceId: 'inst-old',
    });
    app.graph._nodes = [inputMarker, outputMarker];
    app.graph._groups = [group];

    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      { cube_id: CANONICAL_DEMO_ID, name: 'Demo', version: '1.2.1', is_writable: true },
    ];
    ui.dialogs.chooseHistoricalVersionSaveAction = jest.fn(async () => null);
    ui.dirtyManager.markClean = jest.fn();

    await ui.cubeSave.save();

    expect(saveCalls).toBe(0);
    expect(ui.dirtyManager.markClean).not.toHaveBeenCalled();
    expect(group.properties.sugarcubes.cube_version).toBe('1.0.1');
    expect(group.properties.sugarcubes.cube_revision_ref).toBe(HISTORICAL_REVISION_REF);
    expect(inputMarker.properties.sugarcubes_cube_version).toBe('1.0.1');
    expect(inputMarker.properties.sugarcubes_cube_revision_ref).toBe(HISTORICAL_REVISION_REF);
  });

  test('stale historical save as latest uses the finalized persisted version', async () => {
    app.extensionManager.registerSidebarTab = () => {};
    configureGraphToPrompt({ withWorkflow: true });

    api.fetchApi = async (url) => {
      if (url === '/sugarcubes/save_implementation') {
        return {
          ok: true,
          json: async () => ({
            saved: [
              {
                cube_id: CANONICAL_DEMO_ID,
                default_alias: 'Demo',
                path: 'demo.cube',
                forked: false,
                committed: false,
                commit_message: '',
                version: '1.2.1',
                definition: makeFinalizedDefinition(CANONICAL_DEMO_ID, '1.2.1'),
              },
            ],
          }),
        };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    app.graph = { _nodes: [], _groups: [], links: {} };
    const inputMarker = makeMarker({
      id: 2,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const outputMarker = makeMarker({
      id: 3,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const group = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
      markerIds: [2, 3],
      instanceId: 'inst-old',
    });
    app.graph._nodes = [inputMarker, outputMarker];
    app.graph._groups = [group];

    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      { cube_id: CANONICAL_DEMO_ID, name: 'Demo', version: '1.2.1', is_writable: true },
    ];
    ui.dialogs.chooseHistoricalVersionSaveAction = jest.fn(async () => 'latest');

    await ui.cubeSave.save();

    expect(group.properties.sugarcubes).toEqual(
      expect.objectContaining({
        cube_version: '1.2.1',
        cube_revision_ref: CURRENT_REVISION_REF,
        cube_definition_key: `${CANONICAL_DEMO_ID}@1.2.1`,
      }),
    );
    expect(inputMarker.properties.sugarcubes_cube_version).toBe('1.2.1');
    expect(inputMarker.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(outputMarker.properties.sugarcubes_cube_version).toBe('1.2.1');
    expect(outputMarker.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
  });

  test('stale historical fork updates only stale instance markers and preserves revision lineage', async () => {
    app.extensionManager.registerSidebarTab = () => {};
    configureGraphToPrompt({ withWorkflow: true });

    let saveBody: SavePayload | null = null;
    api.fetchApi = async (url, options = {}) => {
      if (url === '/sugarcubes/save_implementation') {
        saveBody = parseSavePayload(options.body);
        return { ok: true, json: async () => ({ saved: [] }) };
      }
      return { ok: true, json: async () => ({ cubes: [] }) };
    };

    await loadUi();
    await setupRegisteredExtension();

    const ui = getLoadedUi();
    app.graph = { _nodes: [], _groups: [], links: {} };
    const staleInput = makeMarker({
      id: 2,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const staleOutput = makeMarker({
      id: 3,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
    });
    const currentInput = makeMarker({
      id: 4,
      type: 'SugarCubes.CubeInput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.2.1',
      revisionRef: CURRENT_REVISION_REF,
    });
    const currentOutput = makeMarker({
      id: 5,
      type: 'SugarCubes.CubeOutput',
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.2.1',
      revisionRef: CURRENT_REVISION_REF,
    });
    const staleGroup = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.0.1',
      revisionRef: HISTORICAL_REVISION_REF,
      markerIds: [2, 3],
      instanceId: 'inst-old',
    });
    const currentGroup = makeCubeGroup({
      cubeId: CANONICAL_DEMO_ID,
      defaultAlias: 'Demo',
      version: '1.2.1',
      revisionRef: CURRENT_REVISION_REF,
      markerIds: [4, 5],
      instanceId: 'inst-current',
    });
    app.graph._nodes = [staleInput, staleOutput, currentInput, currentOutput];
    app.graph._groups = [staleGroup, currentGroup];

    ui.dirtyManager.getDirtyCubeIds = () => new Set([CANONICAL_DEMO_ID]);
    ui.cubeBrowser.getCubes = () => [
      {
        cube_id: CANONICAL_DEMO_ID,
        name: 'Demo',
        version: '1.2.1',
        author: 'Tester',
        author_url: 'https://example.test/tester',
        is_writable: true,
      },
    ];
    ui.dialogs.chooseHistoricalVersionSaveAction = jest.fn(async () => 'fork');
    await ui.cubeSave.save();

    expect(requireSavedCube(saveBody)).toEqual(
      expect.objectContaining({
        cube_id: PERSONAL_DEMO_FORK_ID,
        forked: true,
        previous_cube_id: CANONICAL_DEMO_ID,
        lineage: expect.objectContaining({
          id: CANONICAL_DEMO_ID,
          name: 'Demo',
          version: '1.0.1',
          revision_ref: HISTORICAL_REVISION_REF,
          author: 'Tester',
          author_url: 'https://example.test/tester',
        }),
      }),
    );
    expect(staleInput.widgets.find((widget) => widget.name === 'cube_id')?.value).toBe(
      PERSONAL_DEMO_FORK_ID,
    );
    expect(staleOutput.widgets.find((widget) => widget.name === 'cube_id')?.value).toBe(
      PERSONAL_DEMO_FORK_ID,
    );
    expect(staleGroup.properties.sugarcubes).toEqual(
      expect.objectContaining({
        cube_id: PERSONAL_DEMO_FORK_ID,
        default_alias: 'Demo (fork)',
        cube_version: '',
        cube_revision_ref: CURRENT_REVISION_REF,
        cube_definition_key: PERSONAL_DEMO_FORK_ID,
      }),
    );
    expect(staleInput.properties.sugarcubes_cube_version).toBe('');
    expect(staleInput.properties.sugarcubes_cube_revision_ref).toBe(CURRENT_REVISION_REF);
    expect(currentInput.widgets.find((widget) => widget.name === 'cube_id')?.value).toBe(
      CANONICAL_DEMO_ID,
    );
    expect(currentOutput.widgets.find((widget) => widget.name === 'cube_id')?.value).toBe(
      CANONICAL_DEMO_ID,
    );
    expect(currentGroup.properties.sugarcubes).toEqual(
      expect.objectContaining({
        cube_id: CANONICAL_DEMO_ID,
        cube_version: '1.2.1',
        cube_revision_ref: CURRENT_REVISION_REF,
      }),
    );
  });
});
