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
import { DirtyManager } from '../../frontend/comfyui/ui/graph/DirtyManager.js';
import type { CubeGroupMetadataRecord } from '../../frontend/comfyui/ui/graph/GroupMetadata.js';
import type { CubeDefinitionEntry } from '../../frontend/comfyui/ui/graph/CubeDefinitionStore.js';
import type { UnknownRecord } from '../../frontend/comfyui/ui/types/common.js';

function makeGraph(metadata: CubeGroupMetadataRecord) {
  return {
    _nodes: [
      {
        id: 1,
        type: 'A',
        pos: [0, 0],
        size: [100, 50],
        widgets: [],
        properties: { sugarcubes_symbol: 'node_a' },
      },
    ],
    links: [],
    _groups: [
      {
        title: 'Cube Group',
        pos: [0, 0],
        size: [300, 200],
        properties: { sugarcubes: metadata },
      },
    ],
  };
}

function buildManager({ cubes = [] }: { cubes?: UnknownRecord[] } = {}) {
  const adapter = { getConsole: () => ({ warn: jest.fn() }) };
  const events = {
    on: jest.fn((_event: string, _handler: (payload: UnknownRecord) => void) => undefined),
    emit: jest.fn((_event: string, _payload: unknown) => undefined),
  };
  const scheduler = { raf: (callback: FrameRequestCallback) => (callback(0), 1) };
  const cubeBrowser = { getCubes: () => cubes, setDirtyCubeIds: jest.fn() };
  const ensureSpy = jest.fn((_request: UnknownRecord): CubeDefinitionEntry | null => null);
  const definitionStore = {
    ensure: ensureSpy,
    getEntry: (_request: UnknownRecord): CubeDefinitionEntry | null => null,
  };
  const manager = new DirtyManager({
    adapter,
    events,
    scheduler,
    cubeBrowser,
    definitionStore,
  });

  return { manager, ensureSpy, cubeBrowser };
}

function expectCurrentDefinitionRequest(
  ensureSpy: jest.Mock<(request: UnknownRecord) => CubeDefinitionEntry | null>,
  cubeId: string,
) {
  expect(ensureSpy).toHaveBeenCalledWith({
    cubeId,
    cubeVersion: '',
    revisionRef: 'WORKTREE',
    definitionKey: cubeId,
  });
}

describe('dirty manager definition loading', () => {
  test('prefers relative_path when available', () => {
    const cubeId = 'local/example-user/demo.cube';
    const graph = makeGraph({
      managed: true,
      instance_id: 'inst-1',
      cube_id: cubeId,
      default_alias: 'Demo Cube',
      nodes: ['1'],
      markers: { inputs: [] },
    });
    const { manager, ensureSpy } = buildManager({
      cubes: [{ cube_id: cubeId, relative_path: '_forks/me/demo.cube', name: 'Demo Cube' }],
    });

    manager.refresh({ graph });

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    expectCurrentDefinitionRequest(ensureSpy, cubeId);
  });

  test('falls back to path when relative_path is missing', () => {
    const cubeId = 'local/example-user/demo_path.cube';
    const graph = makeGraph({
      managed: true,
      instance_id: 'inst-2',
      cube_id: cubeId,
      default_alias: 'Demo Path',
      nodes: ['1'],
      markers: { inputs: [] },
    });
    const { manager, ensureSpy } = buildManager({
      cubes: [{ cube_id: cubeId, path: 'E:/cubes/demo.cube', name: 'Demo Path' }],
    });

    manager.refresh({ graph });

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    expectCurrentDefinitionRequest(ensureSpy, cubeId);
  });

  test('skips definition loads without identifiers', () => {
    const cubeId = 'local/example-user/demo_missing_name.cube';
    const graph = makeGraph({
      managed: true,
      instance_id: 'inst-3',
      cube_id: cubeId,
      default_alias: '',
      nodes: ['1'],
      markers: { inputs: [] },
    });
    const { manager, ensureSpy } = buildManager({
      cubes: [{ cube_id: cubeId }],
    });

    manager.refresh({ graph });

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    expectCurrentDefinitionRequest(ensureSpy, cubeId);
  });

  test('uses default_alias when no library entry exists', () => {
    const cubeId = 'local/example-user/loose_cube.cube';
    const graph = makeGraph({
      managed: true,
      instance_id: 'inst-4',
      cube_id: cubeId,
      default_alias: 'Loose Cube',
      nodes: ['1'],
      markers: { inputs: [] },
    });
    const { manager, ensureSpy } = buildManager();

    manager.refresh({ graph });

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    expectCurrentDefinitionRequest(ensureSpy, cubeId);
  });

  test('loads version-aware definitions for historical cube instances', () => {
    const cubeId = 'local/example-user/demo_versioned.cube';
    const graph = makeGraph({
      managed: true,
      instance_id: 'inst-versioned',
      cube_id: cubeId,
      default_alias: 'Demo Versioned',
      cube_version: '1.0.0',
      cube_revision_ref: 'abc123456789',
      cube_definition_key: `${cubeId}@1.0.0`,
      nodes: ['1'],
      markers: { inputs: [] },
    });
    const { manager, ensureSpy } = buildManager({
      cubes: [{ cube_id: cubeId, name: 'Demo Versioned' }],
    });

    manager.refresh({ graph });

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    expect(ensureSpy).toHaveBeenCalledWith({
      cubeId,
      cubeVersion: '1.0.0',
      revisionRef: 'abc123456789',
      definitionKey: `${cubeId}@1.0.0`,
    });
  });

  test('does not mark cubes dirty when library list is empty', () => {
    const cubeId = 'local/example-user/demo_clean.cube';
    const graph = makeGraph({
      managed: true,
      instance_id: 'inst-5',
      cube_id: cubeId,
      default_alias: 'Demo Cube',
      nodes: ['1'],
      markers: { inputs: [] },
    });
    const { manager } = buildManager();

    const result = manager.refresh({ graph });

    expect(Array.from(result.dirtyCubeIds)).toEqual([]);
  });
});
