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
import { CubeDefinitionStore } from '../../web/comfyui/ui/graph/CubeDefinitionStore.js';
import type { UnknownRecord } from '../../web/comfyui/ui/types/common.js';

interface TestDefinition extends UnknownRecord {
  cube: {
    cube_id: string;
    version: string;
    surface?: { controls: Array<{ name: string; default: string }> };
  };
  nodes: UnknownRecord[];
  markers: UnknownRecord[];
  connections: UnknownRecord[];
  layout: { groups: UnknownRecord[] };
}

function makeDefinition(version: string): TestDefinition {
  return {
    cube: {
      cube_id: 'cube-1',
      version,
    },
    nodes: [{ symbol: 'node_a', class_type: 'KSampler', inputs: {} }],
    markers: [],
    connections: [],
    layout: { groups: [] },
  };
}

describe('CubeDefinitionStore', () => {
  test('replaces a ready same-key definition with the finalized save result', () => {
    const onUpdate = jest.fn();
    const store = new CubeDefinitionStore({ onUpdate });
    const request = {
      cubeId: 'cube-1',
      cubeVersion: '1.0.0',
      definitionKey: 'cube-1@1.0.0',
    };
    const first = makeDefinition('1.0.0');
    first.cube.surface = { controls: [{ name: 'prompt', default: 'first' }] };
    const finalized = makeDefinition('1.0.0');
    finalized.cube.surface = { controls: [{ name: 'prompt', default: 'persisted' }] };

    store.publishFinalized(request, first);
    const entry = store.publishFinalized(request, finalized);

    expect(store.getEntry(request)).toBe(entry);
    expect(entry.payload.cube.surface!.controls[0]!.default).toBe('persisted');
    expect(onUpdate).toHaveBeenLastCalledWith('cube-1@1.0.0', entry);
  });

  test('evicts the stale unversioned worktree alias after a finalized save', () => {
    const store = new CubeDefinitionStore();
    store.publishFinalized({ cubeId: 'cube-1' }, makeDefinition(''));

    store.publishFinalized({ cubeId: 'cube-1', cubeVersion: '1.0.0' }, makeDefinition('1.0.0'));

    expect(store.getEntry('cube-1')).toBeNull();
    expect(store.getEntry('cube-1@1.0.0')).toMatchObject({ status: 'ready' });
  });

  test('loads and caches historical definitions by version-aware key', async () => {
    const calls: Array<{ body: UnknownRecord; options?: RequestInit }> = [];
    const onUpdate = jest.fn();
    const api = {
      load: jest.fn(async (_body: BodyInit | null, _options?: RequestInit) => ({
        response: { ok: true },
        data: {},
      })),
      loadRevision: jest.fn(async (body: BodyInit | null, options?: RequestInit) => {
        const serializedBody = typeof body === 'string' ? body : '{}';
        calls.push({ body: JSON.parse(serializedBody) as UnknownRecord, options });
        return {
          response: { ok: true },
          data: makeDefinition('1.0.0'),
        };
      }),
    };
    const store = new CubeDefinitionStore({ api, onUpdate });

    await store.loadDefinition({
      cubeId: 'cube-1',
      cubeVersion: '1.0.0',
      revisionRef: 'abc123456789',
      definitionKey: 'cube-1@1.0.0',
    });

    expect(api.load).not.toHaveBeenCalled();
    expect(api.loadRevision).toHaveBeenCalledTimes(1);
    expect(calls[0]!.body).toMatchObject({
      cube_id: 'cube-1',
      revision_ref: 'abc123456789',
      version_pin: '1.0.0',
    });
    expect(store.getStatus('cube-1@1.0.0')).toBe('ready');
    expect(store.getEntry('cube-1@1.0.0')).toMatchObject({
      cubeId: 'cube-1',
      cubeVersion: '1.0.0',
      revisionRef: 'abc123456789',
      definitionKey: 'cube-1@1.0.0',
    });
    expect(onUpdate).toHaveBeenCalledWith(
      'cube-1@1.0.0',
      expect.objectContaining({ status: 'ready' }),
    );
  });
});
