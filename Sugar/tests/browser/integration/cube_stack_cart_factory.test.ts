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
/** Prove cart capture uses opening catalog/import geometry and rejects changing graph lifetimes. */
import { expect, jest, test } from '@jest/globals';
import { CubeStackCartFactory } from '../../../frontend/comfyui/ui/picker/infrastructure/CubeStackCartFactory.js';
import type { CubePickerDescriptor } from '../../../frontend/comfyui/ui/picker/CubePickerDescriptor.js';
import type { CubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphModels.js';
import type { CubeGraphStackEntry } from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphStackModels.js';
import type { ImportPayload } from '../../../frontend/comfyui/ui/import/PlacementPayload.js';

test('captures exact opening payloads and renderer-planned sizes without constructing native nodes', async () => {
  const f = fixture();
  const capture = await f.factory.open();
  expect(f.events).toEqual([
    'ready',
    'scope',
    'snapshot',
    'analysis',
    'snapshot',
    'size',
    'snapshot',
  ]);
  expect(capture.library[0]?.role).toBe('Start cubes');
  expect(capture.library[0]?.models).toEqual(['SDXL']);
  expect(capture.library[0]?.targetModel).toBe('SDXL');
  expect(capture.library[0]?.secondary).toContain('v1');
  expect(f.refreshCatalog).not.toHaveBeenCalled();
  expect(f.events).not.toContain('prepared');
  f.descriptor.displayName = 'Later name';
  f.payload.document = { cube_id: 'cube', version: 'later' };
  capture.session.add(f.descriptor.key);
  capture.session.add(f.descriptor.key);
  await expect(capture.session.apply()).resolves.toBe(true);
  expect(f.commit).toHaveBeenCalledTimes(1);
  const creations = f.commit.mock.calls[0]![2];
  expect(creations.map((item) => item.instanceId)).toEqual(['new-1', 'new-2']);
  expect(creations.map((item) => item.payload.document)).toEqual([
    { cube_id: 'cube', version: '1' },
    { cube_id: 'cube', version: '1' },
  ]);
  expect(creations.every((item) => item.size[0] === 920 && item.size[1] === 600)).toBe(true);
});

test('refresh preserves staged definitions and captures new catalog generations independently', async () => {
  const f = fixture();
  const capture = await f.factory.open();
  const originalKey = capture.library[0]!.catalogKey;
  capture.session.add(originalKey);
  f.descriptor.displayName = 'Refreshed Cube';
  f.descriptor.contentHash = 'changed';
  f.payload.document = { cube_id: 'cube', version: '1', updated: true };
  await capture.refreshLibrary();
  const updatedKey = capture.library[0]!.catalogKey;
  expect(updatedKey).not.toBe(originalKey);
  expect(capture.artifactRows.get(originalKey)?.label).toBe('Cube');
  expect(capture.artifactRows.get(updatedKey)?.label).toBe('Refreshed Cube');
  capture.session.add(updatedKey);
  await capture.session.apply();
  expect(f.commit.mock.calls[0]![2].map((item) => item.payload.document)).toEqual([
    { cube_id: 'cube', version: '1' },
    { cube_id: 'cube', version: '1', updated: true },
  ]);
});

test('failed or retired refresh leaves the last accepted library and staged draft intact', async () => {
  const f = fixture();
  const capture = await f.factory.open();
  capture.session.add(capture.library[0]!.catalogKey);
  const library = capture.library;
  const entries = capture.session.entries;
  f.refreshCatalog.mockRejectedValueOnce(new Error('Unavailable'));
  await expect(capture.refreshLibrary()).rejects.toThrow('Unavailable');
  expect(capture.library).toBe(library);
  expect(capture.session.entries).toBe(entries);
  capture.session.dispose();
  await expect(capture.refreshLibrary()).rejects.toThrow('closed');
  expect(capture.library).toBe(library);
  expect(f.commit).not.toHaveBeenCalled();
});

test('unchanged refresh preserves row identity and skips frame and placement preparation', async () => {
  const f = fixture();
  const capture = await f.factory.open();
  const row = capture.library[0]!;
  capture.session.add(row.catalogKey);
  f.events.length = 0;
  await capture.refreshLibrary();
  expect(capture.library[0]).toBe(row);
  expect(f.events).not.toContain('size');
  expect(f.events).not.toContain('prepared');
  capture.session.add(row.catalogKey);
  await capture.session.apply();
  expect(f.events.filter((event) => event === 'prepared')).toHaveLength(2);
});

test.each(['reorder', 'stale', 'disposed', 'portable'] as const)(
  'rejects %s capture before a cart can edit or commit',
  async (reason) => {
    const f = fixture();
    if (reason === 'reorder') f.runtime.reorder.pending = true;
    if (reason === 'stale')
      f.runtime.reorder.refresh = async () => {
        f.revision = 'changed';
        return analysis([]);
      };
    if (reason === 'disposed')
      f.runtime.reorder.refresh = async () => {
        f.runtime.graphSnapshots.available = false;
        return analysis([]);
      };
    if (reason === 'portable') delete f.payload.document;
    await expect(f.factory.open()).rejects.toThrow(
      reason === 'reorder'
        ? 'reordering'
        : reason === 'portable'
          ? 'portable document'
          : 'Workflow changed',
    );
    expect(f.commit).not.toHaveBeenCalled();
  },
);

/** Bind narrow external ports and keep every captured artifact isolated as the real registry does. */
function fixture() {
  const events: string[] = [];
  let id = 0;
  const descriptor: CubePickerDescriptor = {
    key: 'a'.repeat(64),
    cubeId: 'cube',
    version: '1',
    contentHash: 'original',
    displayName: 'Cube',
    description: 'Test cube',
    searchTerms: [],
    targetModel: 'SDXL',
    supportedModels: [],
    requiredCustomNodes: [],
    source: { kind: 'personal' },
    inputs: [],
    outputs: [{ id: 'image', name: 'Image', label: 'Image', type: 'IMAGE' }],
  };
  const payload: ImportPayload = {
    cube: { cube_id: 'cube', cube_version: '1', default_alias: 'Cube' },
    document: { cube_id: 'cube', version: '1' },
    nodes: [],
  };
  const commit = jest.fn<
    (
      opening: typeof snapshot,
      result: CubeGraphAnalysis,
      creations: readonly {
        instanceId: string;
        payload: ImportPayload;
        size: readonly [number, number];
      }[],
    ) => void
  >();
  const snapshot = { revision: 'opening', serializedWorkflow: '{}' };
  const state = { revision: 'opening' };
  const runtime = {
    graphScope: {
      assertCurrentRoot: () => {
        events.push('scope');
      },
    },
    reorder: {
      pending: false,
      refresh: async () => {
        events.push('analysis');
        return analysis([]);
      },
    },
    graphSnapshots: {
      available: true,
      snapshot: () => {
        events.push('snapshot');
        return { ...snapshot, revision: state.revision };
      },
    },
    construction: {
      previewSize: () => {
        events.push('size');
        return [920, 600] as [number, number];
      },
    },
    nodes: { list: () => [] },
    stackCommit: { commit },
  };
  const refreshCatalog = jest.fn(async () => {
    events.push('catalog');
  });
  const factory = new CubeStackCartFactory({
    registry: {
      entries: () => [{ type: 'CubeType', descriptor: { ...descriptor } }],
      capturedPayload: () => JSON.parse(JSON.stringify(payload)) as ImportPayload,
    },
    placement: {
      prepareCaptured: (captured, payload) => {
        events.push('prepared');
        return { descriptor: captured, payload, options: {} };
      },
    },
    gateway: {
      applyStack: async (_snapshot, entries: readonly CubeGraphStackEntry[]) =>
        analysis(entries.map((entry) => entry.instanceId)),
    },
    getRuntime: () => runtime,
    ensureCatalog: async () => {
      events.push('ready');
    },
    refreshCatalog,
    createId: () => `new-${++id}`,
  });
  return {
    factory,
    runtime,
    descriptor,
    payload,
    commit,
    events,
    refreshCatalog,
    get revision() {
      return state.revision;
    },
    set revision(value: string) {
      state.revision = value;
    },
  };
}
/** Return exact canonical opening/new identities while external planning remains a replaceable boundary. */
function analysis(ids: readonly string[]): CubeGraphAnalysis {
  return {
    semanticHash: 'hash',
    instances: ids.map((instanceId) => ({
      instanceId,
      nodeId: instanceId,
      definitionId: instanceId,
      cubeId: 'cube',
      version: '1',
      alias: 'Cube',
      executionMode: 0,
    })),
    segments: [{ instanceIds: ids, reorderable: true, boundaryNodeIds: [] }],
    placements: ids.map((instanceId, index) => ({
      instanceId,
      nodeId: instanceId,
      position: [index * 944, 0],
    })),
  };
}
