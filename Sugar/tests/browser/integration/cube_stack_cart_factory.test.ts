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
/** Prove cart capture uses opening catalog/import geometry and applies through the Cube chain. */
import { expect, jest, test } from '@jest/globals';
import { CubeStackCartFactory } from '../../../frontend/comfyui/ui/picker/infrastructure/CubeStackCartFactory.js';
import type { CubePickerDescriptor } from '../../../frontend/comfyui/ui/picker/CubePickerDescriptor.js';
import type { ImportPayload } from '../../../frontend/comfyui/ui/import/PlacementPayload.js';
import type { CubeStackRequest } from '../../../frontend/comfyui/ui/picker/application/CubeStackCartSession.js';

test('captures exact opening payloads and renderer-planned sizes without constructing native nodes', async () => {
  const f = fixture();
  const capture = await f.factory.open();
  expect(f.events).toEqual(['ready', 'scope', 'size', 'order']);
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
  expect(capture.session.apply()).toBe(true);
  expect(f.commit).toHaveBeenCalledTimes(1);
  const creations = created(f.commit.mock.calls[0]![0]);
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
  capture.session.apply();
  expect(created(f.commit.mock.calls[0]![0]).map((item) => item.payload.document)).toEqual([
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
  capture.session.apply();
  expect(f.events.filter((event) => event === 'prepared')).toHaveLength(2);
});

test('rejects a Cube without a portable document before a cart can edit or commit', async () => {
  const f = fixture();
  delete f.payload.document;
  await expect(f.factory.open()).rejects.toThrow('portable document');
  expect(f.commit).not.toHaveBeenCalled();
});

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
  const commit = jest.fn((_requested: readonly CubeStackRequest<ImportPayload>[]) => []);
  const runtime = {
    graphScope: {
      assertCurrentRoot: () => {
        events.push('scope');
      },
    },
    chain: {
      order: () => {
        events.push('order');
        return [];
      },
      apply: commit,
    },
    construction: {
      previewSize: () => {
        events.push('size');
        return [920, 600] as [number, number];
      },
    },
    nodes: { list: () => [] },
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
  };
}
/** The new Cubes a cart asked the chain to create. */
function created(requested: readonly CubeStackRequest<ImportPayload>[]) {
  return requested.flatMap((entry) => (entry.source === 'new' ? [entry] : []));
}
