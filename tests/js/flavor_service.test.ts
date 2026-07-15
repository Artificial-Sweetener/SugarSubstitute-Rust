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
import { describe, expect, test, jest } from '@jest/globals';
import { FlavorStorage } from '../../frontend/comfyui/ui/flavors/FlavorStorage.js';
import { FlavorService } from '../../frontend/comfyui/ui/flavors/FlavorService.js';
import type { FlavorMetadata } from '../../frontend/comfyui/ui/flavors/FlavorService.js';
import { StorageService } from '../../frontend/comfyui/ui/core/StorageService.js';
import { readWidgetValue } from '../../frontend/comfyui/ui/graph/Markers.js';

interface TestFlavor {
  id: string;
  name: string;
  values: Record<string, unknown>;
  updated_at?: string;
}

interface TestSurfaceState {
  selected_flavor_id: string;
  flavors: TestFlavor[];
}

interface TestCubeState {
  schema_version: number;
  cube_id: string;
  surfaces: Record<string, TestSurfaceState>;
}

interface FlavorRequestBody {
  cube_id: string;
  surface_signature: string;
  flavor_id?: string;
  name: string;
  values: Record<string, unknown>;
  states?: Array<{ cube_id: string; state: TestCubeState }>;
}

function parseFlavorRequest(payload: string): FlavorRequestBody {
  return JSON.parse(payload) as FlavorRequestBody;
}

function flavorMetadata(value: unknown): FlavorMetadata {
  return value as FlavorMetadata;
}

function createStorage() {
  localStorage.clear();
  return new StorageService({ getStorage: () => localStorage });
}

function createFlavorApi() {
  const states = new Map<string, TestCubeState>();
  const readState = (cubeId: string): TestCubeState => ({
    schema_version: 1,
    cube_id: cubeId,
    surfaces: {},
    ...(states.get(cubeId) || {}),
  });
  const writeState = (cubeId: string, state: TestCubeState): TestCubeState => {
    const next = {
      schema_version: 1,
      cube_id: cubeId,
      surfaces: state?.surfaces || {},
    };
    states.set(cubeId, next);
    return next;
  };
  return {
    states,
    getLocalFlavors: jest.fn(async (cubeId: string) => ({
      response: { ok: true },
      data: { state: readState(cubeId) },
    })),
    saveLocalFlavor: jest.fn(async (payload: string) => {
      const body = parseFlavorRequest(payload);
      const state = readState(body.cube_id);
      const surface = state.surfaces[body.surface_signature] || {
        selected_flavor_id: '',
        flavors: [],
      };
      const flavorId = body.flavor_id || body.name.toLowerCase().replace(/[^0-9a-z_-]+/g, '_');
      const flavor = {
        id: flavorId,
        name: body.name,
        values: body.values,
        updated_at: '2026-04-13T12:00:00Z',
      };
      state.surfaces[body.surface_signature] = {
        selected_flavor_id: flavorId,
        flavors: [...surface.flavors.filter((entry) => entry.id !== flavorId), flavor],
      };
      return { response: { ok: true }, data: { state: writeState(body.cube_id, state) } };
    }),
    deleteLocalFlavor: jest.fn(async (payload: string) => {
      const body = parseFlavorRequest(payload);
      const state = readState(body.cube_id);
      const surface = state.surfaces[body.surface_signature] || {
        selected_flavor_id: '',
        flavors: [],
      };
      state.surfaces[body.surface_signature] = {
        selected_flavor_id:
          surface.selected_flavor_id === body.flavor_id ? '' : surface.selected_flavor_id,
        flavors: surface.flavors.filter((entry) => entry.id !== body.flavor_id),
      };
      return { response: { ok: true }, data: { state: writeState(body.cube_id, state) } };
    }),
    selectLocalFlavor: jest.fn(async (payload: string) => {
      const body = parseFlavorRequest(payload);
      const state = readState(body.cube_id);
      const surface = state.surfaces[body.surface_signature] || {
        selected_flavor_id: '',
        flavors: [],
      };
      state.surfaces[body.surface_signature] = {
        ...surface,
        selected_flavor_id: body.flavor_id ?? '',
      };
      return { response: { ok: true }, data: { state: writeState(body.cube_id, state) } };
    }),
    migrateLocalFlavors: jest.fn(async (payload: string) => {
      const body = parseFlavorRequest(payload);
      for (const entry of body.states || []) {
        writeState(entry.cube_id, entry.state);
      }
      return { response: { ok: true }, data: { count: body.states?.length || 0 } };
    }),
    reconcileLocalFlavors: jest.fn(async (payload: string) => {
      const body = parseFlavorRequest(payload);
      const state = readState(body.cube_id);
      return { response: { ok: true }, data: { state, renamed: [], conflict_count: 0 } };
    }),
  };
}

describe('flavor storage', () => {
  test('stores local flavors through the API cache with surface segregation', async () => {
    const api = createFlavorApi();
    const storage = new FlavorStorage({ storage: createStorage(), api });

    const saved = await storage.saveLocalFlavor({
      cubeId: 'cube-1',
      surfaceSignature: 'surface-a',
      name: 'Portrait',
      values: { 'ksampler.cfg': 7 },
    });

    expect(saved!.id).toBe('portrait');
    const payload = storage.readCubeState('cube-1');
    expect(payload.surfaces['surface-a']!.selected_flavor_id).toBe('portrait');
    expect(payload.surfaces['surface-a']!.flavors[0]!.values).toEqual({ 'ksampler.cfg': 7 });
    expect(localStorage.getItem('sugarcubes.local_flavors.cube-1')).toBeNull();
  });
});

describe('flavor service', () => {
  test('readWidgetValue preserves authored string whitespace', () => {
    const value = readWidgetValue(
      { widgets: [{ name: 'value', value: 'STYLE(comfy++) ' }] },
      'value',
    );

    expect(value).toBe('STYLE(comfy++) ');
  });

  test('buildImportedMetadata exposes only authored defaults from cube payload', () => {
    const service = new FlavorService({});

    const metadata = service.buildImportedMetadata({
      surface_signature: 'surface-a',
      surface: {
        default_flavor_id: 'default',
        controls: [
          { control_id: 'ksampler.cfg', symbol: 'ksampler', input_name: 'cfg' },
          { control_id: 'ksampler.seed', symbol: 'ksampler', input_name: 'seed' },
        ],
      },
      flavors: {
        authored: [
          {
            id: 'default',
            name: 'Default',
            values: { 'ksampler.cfg': 7, 'ksampler.seed': 12345 },
          },
          {
            id: 'portrait',
            name: 'Portrait',
            values: { 'ksampler.cfg': 9 },
          },
        ],
      },
    });

    expect(metadata.surface_signature).toBe('surface-a');
    expect(metadata.flavor).toBe('default');
    expect(metadata.active_flavor_values).toEqual({ 'ksampler.cfg': 7 });
    expect(metadata.authored_flavors[0].id).toBe('default');
    expect(metadata.authored_flavors.map((entry) => entry.id)).toEqual(['default', 'portrait']);
    expect(metadata.flavor_options).toEqual([
      expect.objectContaining({ id: 'default', scope: 'authored', selected: true }),
    ]);
    expect(metadata.flavors).toEqual(['Default']);
    expect(metadata.local_flavors).toEqual([]);
  });

  test('setup does not migrate dormant local flavor state', async () => {
    const api = createFlavorApi();
    const events = { on: jest.fn(() => jest.fn()) };
    const service = new FlavorService({ api, events, storage: createStorage() });

    await service.setup();

    expect(api.migrateLocalFlavors).not.toHaveBeenCalled();
    expect(events.on).toHaveBeenCalledWith('cube:instances:updated', expect.any(Function));
    expect(events.on).toHaveBeenCalledWith('cube:flavor:change', expect.any(Function));
    expect(events.on).toHaveBeenCalledWith('cube:definition:loaded', expect.any(Function));
  });

  test('saveCurrentFaceValuesAsLocalFlavor persists current surface values', async () => {
    const storage = createStorage();
    const api = createFlavorApi();
    const dirtyManager = { requestRefresh: jest.fn() };
    const graph = {
      _nodes: [
        {
          id: 1,
          widgets: [
            { name: 'cfg', value: 7 },
            { name: 'seed', value: 12345 },
          ],
          properties: { sugarcubes_symbol: 'ksampler' },
        },
      ],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              instance_alias: 'Portrait Seed',
              cube_id: 'cube-1',
              nodes: ['1'],
              surface_signature: 'surface-a',
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'ksampler.cfg',
                    symbol: 'ksampler',
                    input_name: 'cfg',
                  },
                  {
                    control_id: 'ksampler.seed',
                    symbol: 'ksampler',
                    input_name: 'seed',
                  },
                ],
              },
              authored_flavors: [
                {
                  id: 'default',
                  name: 'Default',
                  values: { 'ksampler.cfg': 5, 'ksampler.seed': 22222 },
                },
              ],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    const dialogs = {
      promptText: jest.fn(async () => 'Portrait'),
    };
    const service = new FlavorService({ adapter, dialogs, storage, api, dirtyManager });

    const saved = await service.saveCurrentFaceValuesAsLocalFlavor(
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(saved).toBe(true);
    expect(dialogs.promptText).toHaveBeenCalledWith(
      expect.objectContaining({ initialValue: 'Portrait Seed' }),
    );
    const payload = api.states.get('cube-1');
    expect(payload!.surfaces['surface-a']!.flavors[0]!.name).toBe('Portrait');
    expect(payload!.surfaces['surface-a']!.flavors[0]!.values).toEqual({ 'ksampler.cfg': 7 });
    expect(payload!.surfaces['surface-a']!.flavors[0]!.values).not.toHaveProperty('ksampler.seed');
    expect(dirtyManager.requestRefresh).toHaveBeenCalled();
  });

  test('saveCurrentFaceValuesAsAuthoredFlavor seeds prompt from alias fallbacks', async () => {
    const dirtyManager = { requestRefresh: jest.fn() };
    const graph = {
      _nodes: [
        {
          id: 1,
          widgets: [
            { name: 'cfg', value: 7 },
            { name: 'seed', value: 12345 },
          ],
          properties: { sugarcubes_symbol: 'ksampler' },
        },
      ],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              instance_alias: '',
              default_alias: 'Demo Default',
              cube_id: 'cube-1',
              nodes: ['1'],
              surface_signature: 'surface-a',
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'ksampler.cfg',
                    symbol: 'ksampler',
                    input_name: 'cfg',
                  },
                  {
                    control_id: 'ksampler.seed',
                    symbol: 'ksampler',
                    input_name: 'seed',
                  },
                ],
              },
              authored_flavors: [
                {
                  id: 'default',
                  name: 'Default',
                  values: { 'ksampler.cfg': 5, 'ksampler.seed': 22222 },
                },
              ],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    const dialogs = {
      promptText: jest.fn(async () => 'Custom'),
    };
    const api = {
      saveAuthoredFlavor: jest.fn(async (_payload: string) => ({
        response: { ok: true },
        data: { saved: { flavor_id: 'custom' } },
      })),
    };
    const service = new FlavorService({ adapter, dialogs, api, dirtyManager });

    const saved = await service.saveCurrentFaceValuesAsAuthoredFlavor(
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(saved).toBe(true);
    expect(dialogs.promptText).toHaveBeenCalledWith(
      expect.objectContaining({ initialValue: 'Demo Default' }),
    );
    expect(JSON.parse(api.saveAuthoredFlavor.mock.calls[0][0]).values).toEqual({
      'ksampler.cfg': 7,
    });

    flavorMetadata(graph._groups[0].properties.sugarcubes).instance_alias = 'Instance Seed';
    await service.saveCurrentFaceValuesAsAuthoredFlavor(
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(dialogs.promptText).toHaveBeenLastCalledWith(
      expect.objectContaining({ initialValue: 'Instance Seed' }),
    );
  });

  test('saveCurrentFaceValuesAsDefault uses cube defaults copy and payload', async () => {
    const dirtyManager = { requestRefresh: jest.fn() };
    const graph = {
      _nodes: [
        {
          id: 1,
          widgets: [{ name: 'cfg', value: 7 }],
          properties: { sugarcubes_symbol: 'ksampler' },
        },
      ],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              nodes: ['1'],
              surface_signature: 'surface-a',
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'ksampler.cfg',
                    symbol: 'ksampler',
                    input_name: 'cfg',
                  },
                ],
              },
              authored_flavors: [{ id: 'default', name: 'Default', values: {} }],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    const api = {
      saveAuthoredFlavor: jest.fn(async (_payload: string) => ({
        response: { ok: true },
        data: { saved: { flavor_id: 'default' } },
      })),
    };
    const toast = { push: jest.fn() };
    const service = new FlavorService({ adapter, api, dirtyManager, toast });

    const saved = await service.saveCurrentFaceValuesAsDefault(
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(saved).toBe(true);
    expect(JSON.parse(api.saveAuthoredFlavor.mock.calls[0][0])).toEqual({
      cube_id: 'cube-1',
      flavor_id: 'default',
      flavor_name: 'Default',
      values: { 'ksampler.cfg': 7 },
    });
    expect(toast.push).toHaveBeenCalledWith(
      'success',
      'Cube defaults saved',
      'Current values saved as cube defaults.',
    );
    expect(flavorMetadata(graph._groups[0].properties.sugarcubes).flavor_options).toEqual([
      expect.objectContaining({ id: 'default', selected: true }),
    ]);
  });

  test('saveCurrentFaceValuesAsDefault reads unsymbolized managed surface nodes', async () => {
    const graph = {
      _nodes: [
        {
          id: 2147,
          type: 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion',
          properties: { 'Node name for S&R': 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion' },
          inputs: [
            { name: 'scale_factor', widget: { name: 'scale_factor' } },
            { name: 'cfg', widget: { name: 'cfg' } },
            { name: 'sampler_name', widget: { name: 'sampler_name' } },
          ],
          widgets: [
            { name: 'scale_factor', value: 1.5 },
            { name: 'cfg', value: 6 },
            { name: 'sampler_name', value: 'er_sde' },
          ],
        },
      ],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              nodes: ['2147'],
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'detailer.scale_factor',
                    symbol: 'detailer',
                    input_name: 'scale_factor',
                    class_type: 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion',
                  },
                  {
                    control_id: 'detailer.cfg',
                    symbol: 'detailer',
                    input_name: 'cfg',
                    class_type: 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion',
                  },
                  {
                    control_id: 'detailer.sampler_name',
                    symbol: 'detailer',
                    input_name: 'sampler_name',
                    class_type: 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion',
                  },
                ],
              },
              authored_flavors: [{ id: 'default', name: 'Default', values: {} }],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    const api = {
      saveAuthoredFlavor: jest.fn(async (_payload: string) => ({
        response: { ok: true },
        data: { saved: { flavor_id: 'default' } },
      })),
    };
    const service = new FlavorService({
      adapter,
      api,
      dirtyManager: { requestRefresh: jest.fn() },
    });

    const saved = await service.saveCurrentFaceValuesAsDefault(
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(saved).toBe(true);
    expect(JSON.parse(api.saveAuthoredFlavor.mock.calls[0][0]).values).toEqual({
      'detailer.scale_factor': 1.5,
      'detailer.cfg': 6,
      'detailer.sampler_name': 'er_sde',
    });
  });

  test('saveCurrentFaceValuesAsDefault uses defaults wording on failure', async () => {
    const graph = {
      _nodes: [],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              nodes: [],
              surface: { default_flavor_id: 'default', controls: [] },
              authored_flavors: [{ id: 'default', name: 'Default', values: {} }],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    const api = {
      saveAuthoredFlavor: jest.fn(async (_payload: string) => ({
        response: { ok: false, statusText: '' },
        data: {},
      })),
    };
    const toast = { push: jest.fn() };
    const service = new FlavorService({ adapter, api, toast });

    const saved = await service.saveCurrentFaceValuesAsDefault(
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(saved).toBe(false);
    expect(toast.push).toHaveBeenCalledWith(
      'error',
      'Default save failed',
      'Current values could not be saved as cube defaults.',
    );
  });

  test('selectFlavor ignores local flavor requests in defaults-only mode', async () => {
    const storage = createStorage();
    const api = createFlavorApi();
    const dirtyManager = { requestRefresh: jest.fn() };
    const graph = {
      _nodes: [
        {
          id: 1,
          widgets: [{ name: 'cfg', value: 6 }],
          properties: { sugarcubes_symbol: 'ksampler' },
        },
      ],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              nodes: ['1'],
              surface_signature: 'surface-a',
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'ksampler.cfg',
                    symbol: 'ksampler',
                    input_name: 'cfg',
                  },
                ],
              },
              flavor: 'portrait',
              flavor_scope: 'authored',
              authored_flavors: [
                { id: 'default', name: 'Default', values: { 'ksampler.cfg': 5 } },
                { id: 'portrait', name: 'Portrait', values: { 'ksampler.cfg': 7 } },
              ],
              local_flavors: [
                { id: 'portrait_local', name: 'Portrait', values: { 'ksampler.cfg': 9 } },
              ],
              flavor_options: [
                {
                  id: 'default',
                  name: 'Default',
                  scope: 'authored',
                  selected: false,
                  values: { 'ksampler.cfg': 5 },
                },
                {
                  id: 'portrait',
                  name: 'Portrait',
                  scope: 'authored',
                  selected: true,
                  values: { 'ksampler.cfg': 7 },
                },
                {
                  id: 'portrait_local',
                  name: 'Portrait',
                  scope: 'local',
                  selected: false,
                  values: { 'ksampler.cfg': 9 },
                },
              ],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    api.states.set('cube-1', {
      schema_version: 1,
      cube_id: 'cube-1',
      surfaces: {
        'surface-a': {
          selected_flavor_id: '',
          flavors: [{ id: 'portrait_local', name: 'Portrait', values: { 'ksampler.cfg': 9 } }],
        },
      },
    });
    const service = new FlavorService({ adapter, storage, api, dirtyManager });
    await service.loadLocalFlavorState('cube-1');

    await service.selectFlavor({
      metadata: flavorMetadata(graph._groups[0].properties.sugarcubes),
      flavor: {
        id: 'portrait_local',
        name: 'Portrait',
        scope: 'local',
        values: { 'ksampler.cfg': 9 },
      },
    });

    const metadata = flavorMetadata(graph._groups[0].properties.sugarcubes);
    expect(metadata.flavor).toBe('portrait');
    expect(metadata.flavor_scope).toBe('authored');
    expect(readWidgetValue(graph._nodes[0], 'cfg')).toBe(6);
    expect(api.selectLocalFlavor).not.toHaveBeenCalled();
    expect(dirtyManager.requestRefresh).not.toHaveBeenCalled();
  });

  test('refreshGroupMetadata clamps stale selected state to authored default', async () => {
    const storage = createStorage();
    const api = createFlavorApi();
    const dirtyManager = { requestRefresh: jest.fn() };
    const graph = {
      _nodes: [
        {
          id: 1,
          widgets: [{ name: 'cfg', value: 9 }],
          properties: { sugarcubes_symbol: 'ksampler' },
        },
      ],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              nodes: ['1'],
              surface_signature: 'surface-a',
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'ksampler.cfg',
                    symbol: 'ksampler',
                    input_name: 'cfg',
                  },
                ],
              },
              flavor: 'portrait_local',
              flavor_scope: 'local',
              authored_flavors: [
                { id: 'default', name: 'Default', values: { 'ksampler.cfg': 5 } },
                { id: 'portrait', name: 'Portrait', values: { 'ksampler.cfg': 7 } },
              ],
              local_flavors: [
                { id: 'portrait_local', name: 'Portrait Local', values: { 'ksampler.cfg': 9 } },
              ],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    api.states.set('cube-1', {
      schema_version: 1,
      cube_id: 'cube-1',
      surfaces: {
        'surface-a': {
          selected_flavor_id: 'portrait_local',
          flavors: [
            { id: 'portrait_local', name: 'Portrait Local', values: { 'ksampler.cfg': 9 } },
          ],
        },
      },
    });
    const service = new FlavorService({ adapter, storage, api, dirtyManager });
    await service.loadLocalFlavorState('cube-1');

    service.refreshGroupMetadata(
      graph,
      graph._groups[0],
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    const metadata = flavorMetadata(graph._groups[0].properties.sugarcubes);
    expect(metadata.flavor).toBe('default');
    expect(metadata.flavor_scope).toBe('authored');
    expect(metadata.active_flavor_values).toEqual({ 'ksampler.cfg': 5 });
    expect(metadata.local_flavors).toEqual([]);
    expect(
      metadata.flavor_options!.map((entry) => [entry.id, entry.scope, entry.selected]),
    ).toEqual([['default', 'authored', true]]);
    expect(readWidgetValue(graph._nodes[0], 'cfg')).toBe(5);
  });

  test('hydrateFromDefinition upgrades existing group metadata from loaded cube definition', async () => {
    const graph = {
      _nodes: [],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              default_alias: 'Demo',
              nodes: [],
              flavor: 'portrait_local',
              flavor_scope: 'local',
              authored_flavors: [{ id: 'default', name: 'Default', values: {} }],
              surface: null,
              surface_signature: '',
            },
          },
        },
      ],
    };
    const service = new FlavorService({ api: createFlavorApi() });

    await service.hydrateFromDefinition({
      cubeId: 'cube-1',
      graph,
      entry: {
        status: 'ready',
        payload: {
          cube: {
            surface_signature: 'surface-a',
            surface: {
              default_flavor_id: 'default',
              controls: [{ control_id: 'ksampler.cfg', symbol: 'ksampler', input_name: 'cfg' }],
            },
            flavors: {
              authored: [
                { id: 'default', name: 'Default', values: { 'ksampler.cfg': 7 } },
                { id: 'portrait', name: 'Portrait', values: { 'ksampler.cfg': 9 } },
              ],
            },
          },
        },
      },
    });

    const metadata = flavorMetadata(graph._groups[0].properties.sugarcubes);
    expect(metadata.surface_signature).toBe('surface-a');
    expect(metadata.surface!.controls).toHaveLength(1);
    expect(metadata.authored_flavors![0]!.values).toEqual({ 'ksampler.cfg': 7 });
    expect(metadata.active_flavor_values).toEqual({ 'ksampler.cfg': 7 });
    expect(metadata.flavor).toBe('default');
    expect(metadata.flavor_scope).toBe('authored');
    expect(metadata.flavor_options!.map((entry) => entry.id)).toEqual(['default']);
  });

  test('finalized-save hydration reapplies the authoritative authored value', async () => {
    const node = {
      id: 1,
      type: 'KSampler',
      properties: { sugarcubes_symbol: 'ksampler' },
      widgets: [{ name: 'cfg', value: 99 }],
    };
    const graph = {
      _nodes: [node],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              nodes: ['1'],
              flavor: 'default',
              flavor_scope: 'authored',
              active_flavor_values: { 'ksampler.cfg': 7 },
            },
          },
        },
      ],
    };
    const service = new FlavorService({ api: createFlavorApi() });

    await service.hydrateFromDefinition({
      cubeId: 'cube-1',
      graph,
      forceApply: true,
      entry: {
        status: 'ready',
        payload: {
          cube: {
            surface_signature: 'surface-a',
            surface: {
              default_flavor_id: 'default',
              controls: [
                {
                  control_id: 'ksampler.cfg',
                  symbol: 'ksampler',
                  input_name: 'cfg',
                  class_type: 'KSampler',
                },
              ],
            },
            flavors: {
              authored: [{ id: 'default', name: 'Default', values: { 'ksampler.cfg': 7 } }],
            },
          },
        },
      },
    });

    expect(readWidgetValue(node, 'cfg')).toBe(7);
  });

  test('hydrateFromDefinition only updates groups with the matching definition key', async () => {
    const graph = {
      _nodes: [],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-old',
              cube_id: 'cube-1',
              cube_version: '1.0.0',
              cube_definition_key: 'cube-1@1.0.0',
              surface_signature: 'surface-old',
              authored_flavors: [{ id: 'default', name: 'Default', values: {} }],
            },
          },
        },
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-new',
              cube_id: 'cube-1',
              cube_version: '1.2.0',
              cube_definition_key: 'cube-1@1.2.0',
              surface_signature: 'surface-stale',
              authored_flavors: [{ id: 'default', name: 'Default', values: {} }],
            },
          },
        },
      ],
    };
    const service = new FlavorService({ api: createFlavorApi() });

    await service.hydrateFromDefinition({
      cubeId: 'cube-1',
      definitionKey: 'cube-1@1.2.0',
      graph,
      entry: {
        status: 'ready',
        payload: {
          cube: {
            version: '1.2.0',
            surface_signature: 'surface-new',
            surface: {
              default_flavor_id: 'default',
              controls: [{ control_id: 'ksampler.cfg', symbol: 'ksampler', input_name: 'cfg' }],
            },
            flavors: {
              authored: [{ id: 'default', name: 'Default', values: { 'ksampler.cfg': 9 } }],
            },
          },
        },
      },
    });

    expect(flavorMetadata(graph._groups[0].properties.sugarcubes).surface_signature).toBe(
      'surface-old',
    );
    expect(graph._groups[1].properties.sugarcubes.surface_signature).toBe('surface-new');
    expect(flavorMetadata(graph._groups[1].properties.sugarcubes).active_flavor_values).toEqual({
      'ksampler.cfg': 9,
    });
  });

  test('reconcileGroupLocalFlavors prompts before renaming authored collisions', async () => {
    const graph = {
      _nodes: [],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              surface_signature: 'surface-a',
              authored_flavors: [
                { id: 'default', name: 'Default', values: {} },
                { id: 'portrait', name: 'Portrait', values: {} },
              ],
            },
          },
        },
      ],
    };
    const api = createFlavorApi();
    api.states.set('cube-1', {
      schema_version: 1,
      cube_id: 'cube-1',
      surfaces: {
        'surface-a': {
          selected_flavor_id: 'portrait',
          flavors: [{ id: 'portrait', name: 'Portrait', values: {} }],
        },
      },
    });
    const dialogs = { promptText: jest.fn(async () => 'Portrait Local') };
    const service = new FlavorService({ api, dialogs });
    await service.loadLocalFlavorState('cube-1');

    await service.reconcileGroupLocalFlavors(
      graph,
      graph._groups[0],
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(dialogs.promptText).toHaveBeenCalledWith(
      expect.objectContaining({ initialValue: 'Portrait_local' }),
    );
    const payload = JSON.parse(api.reconcileLocalFlavors.mock.calls[0][0]);
    expect(payload.rename_map).toEqual({ portrait: 'Portrait Local' });
  });

  test('manageFlavors deletes the selected local flavor through dialog selection', async () => {
    const storage = createStorage();
    const dirtyManager = { requestRefresh: jest.fn() };
    const graph = {
      _nodes: [
        {
          id: 1,
          widgets: [{ name: 'cfg', value: 7 }],
          properties: { sugarcubes_symbol: 'ksampler' },
        },
      ],
      _groups: [
        {
          properties: {
            sugarcubes: {
              managed: true,
              instance_id: 'inst-1',
              cube_id: 'cube-1',
              nodes: ['1'],
              surface_signature: 'surface-a',
              surface: {
                default_flavor_id: 'default',
                controls: [
                  {
                    control_id: 'ksampler.cfg',
                    symbol: 'ksampler',
                    input_name: 'cfg',
                  },
                ],
              },
              authored_flavors: [{ id: 'default', name: 'Default', values: { 'ksampler.cfg': 5 } }],
              local_flavors: [{ id: 'portrait', name: 'Portrait', values: { 'ksampler.cfg': 7 } }],
            },
          },
        },
      ],
      setDirtyCanvas: jest.fn(),
    };
    const adapter = {
      getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }),
    };
    const dialogs = {
      selectItem: jest.fn(async () => 'portrait'),
    };
    const toast = { push: jest.fn() };
    const api = createFlavorApi();
    api.states.set('cube-1', {
      schema_version: 1,
      cube_id: 'cube-1',
      surfaces: {
        'surface-a': {
          selected_flavor_id: 'portrait',
          flavors: [{ id: 'portrait', name: 'Portrait', values: { 'ksampler.cfg': 7 } }],
        },
      },
    });
    const service = new FlavorService({ adapter, dialogs, storage, api, dirtyManager, toast });
    await service.loadLocalFlavorState('cube-1');

    const deleted = await service.manageFlavors(
      flavorMetadata(graph._groups[0].properties.sugarcubes),
    );

    expect(deleted).toBe(true);
    expect(dialogs.selectItem).toHaveBeenCalled();
    expect(dirtyManager.requestRefresh).toHaveBeenCalledWith({
      graph,
      reason: 'local-flavor-delete',
    });
    expect(toast.push).toHaveBeenCalledWith('success', 'Local flavor deleted', 'portrait');
  });
});
