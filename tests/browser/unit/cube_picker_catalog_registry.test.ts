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
import { describe, expect, jest, test } from '@jest/globals';
import type { ApiJsonResult } from '../../../frontend/comfyui/ui/core/CubeLibraryApi.js';
import { CubePickerCatalogRegistry } from '../../../frontend/comfyui/ui/picker/CubePickerCatalogRegistry.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

const keyA = 'a'.repeat(64);
const keyB = 'b'.repeat(64);

function response(data: UnknownRecord, ok = true): ApiJsonResult {
  return { response: { ok, status: ok ? 200 : 500 }, data };
}

function descriptor(
  key: string,
  cubeId: string,
  version = '1.0.0',
  targetModel = '',
  source: UnknownRecord = { kind: 'local' },
): UnknownRecord {
  return {
    key,
    cubeId,
    version,
    displayName: cubeId,
    description: '',
    searchTerms: [cubeId],
    targetModel,
    supportedModels: [],
    requiredCustomNodes: [],
    source,
    inputs: [],
    outputs: [],
  };
}

function catalog(revision: string, entries: UnknownRecord[]): UnknownRecord {
  return { schemaVersion: 1, catalogRevision: revision, entries, errors: [] };
}

function payload(cubeId: string, version = '1.0.0'): UnknownRecord {
  return {
    cube: { cube_id: cubeId, version, default_alias: cubeId },
    nodes: [],
    markers: [],
    connections: [],
    subgraphs: [],
    layout: { origin: [0, 0], groups: [] },
    boundaries: { inputs: [], outputs: [] },
  };
}

function setup(catalogResults: ApiJsonResult[], payloads: Record<string, ApiJsonResult>) {
  const listPickerCatalog = jest.fn(async () => {
    const result = catalogResults.shift();
    if (!result) throw new Error('No catalog result queued.');
    return result;
  });
  const load = jest.fn(async (body: BodyInit | null) => {
    const decoded: unknown = JSON.parse(String(body));
    const cubeId =
      typeof decoded === 'object' && decoded !== null
        ? String(Reflect.get(decoded, 'cube_id') || '')
        : '';
    const result = payloads[cubeId];
    if (!result) throw new Error(`Missing ${cubeId}`);
    return result;
  });
  const logger = { debug: jest.fn(), error: jest.fn(), warn: jest.fn() };
  return {
    registry: new CubePickerCatalogRegistry({ api: { listPickerCatalog, load }, logger }),
    listPickerCatalog,
    load,
    logger,
  };
}

describe('Cube picker catalog registry', () => {
  test('publishes an atomic snapshot only after every advertised payload is valid', async () => {
    const cubeA = 'local/demo/a.cube';
    const cubeB = 'local/demo/b.cube';
    const { registry, logger } = setup(
      [response(catalog('revision-1', [descriptor(keyA, cubeA), descriptor(keyB, cubeB)]))],
      { [cubeA]: response(payload(cubeA)), [cubeB]: response({}, false) },
    );

    const result = await registry.refresh();

    expect(result.changed).toBe(true);
    expect(result.unavailableCubeIds).toEqual([cubeB]);
    expect(result.definitions.map((definition) => definition.name)).toEqual([
      `SugarCubes.Cube.${keyA}`,
    ]);
    expect(registry.descriptor(`SugarCubes.Cube.${keyB}`)).toBeNull();
    expect(logger.error).toHaveBeenCalledWith(
      'SugarCubes picker payload is unavailable.',
      expect.objectContaining({ cubeId: cubeB }),
    );
  });

  test('projects native model categories and pack provenance from catalog metadata', async () => {
    const cubeA = 'Artificial-Sweetener/Base-Cubes/Anima/demo.cube';
    const { registry } = setup(
      [
        response(
          catalog('revision-models', [
            descriptor(keyA, cubeA, '1.0.0', 'Anima', {
              kind: 'github',
              repoRef: 'Artificial-Sweetener/Base-Cubes',
            }),
          ]),
        ),
      ],
      { [cubeA]: response(payload(cubeA)) },
    );

    const result = await registry.refresh();

    expect(result.definitions[0]).toMatchObject({
      category: 'SugarCubes/Anima',
      python_module: 'custom_nodes.Base-Cubes',
      sugarcubes_pack_name: 'Base-Cubes',
      sugarcubes_target_model: 'Anima',
    });
  });

  test('coalesces concurrent refresh and skips unchanged revisions', async () => {
    const cubeA = 'local/demo/a.cube';
    const first = response(catalog('revision-1', [descriptor(keyA, cubeA)]));
    const { registry, listPickerCatalog, load } = setup([first, first], {
      [cubeA]: response(payload(cubeA)),
    });

    const [left, right] = await Promise.all([registry.refresh(), registry.refresh()]);
    const unchanged = await registry.refresh();

    expect(left).toEqual(right);
    expect(unchanged.changed).toBe(false);
    expect(listPickerCatalog).toHaveBeenCalledTimes(2);
    expect(load).toHaveBeenCalledTimes(1);
  });

  test('queues a forced invalidation that arrives during an active refresh', async () => {
    const cubeA = 'local/demo/a.cube';
    let releaseFirst!: (value: ApiJsonResult) => void;
    const firstResult = new Promise<ApiJsonResult>((resolve) => {
      releaseFirst = resolve;
    });
    const listPickerCatalog = jest
      .fn<() => Promise<ApiJsonResult>>()
      .mockReturnValueOnce(firstResult)
      .mockResolvedValueOnce(response(catalog('revision-2', [descriptor(keyA, cubeA, '2.0.0')])));
    const load = jest
      .fn<(body: BodyInit | null) => Promise<ApiJsonResult>>()
      .mockResolvedValueOnce(response(payload(cubeA)))
      .mockResolvedValueOnce(response(payload(cubeA, '2.0.0')));
    const registry = new CubePickerCatalogRegistry({
      api: { listPickerCatalog, load },
      logger: { debug: jest.fn(), error: jest.fn(), warn: jest.fn() },
    });

    const initial = registry.refresh();
    const forced = registry.refresh({ force: true });
    releaseFirst(response(catalog('revision-1', [descriptor(keyA, cubeA)])));

    await initial;
    await forced;
    expect(listPickerCatalog).toHaveBeenCalledTimes(2);
    expect(load).toHaveBeenCalledTimes(2);
    expect(registry.revision).toBe('revision-2');
  });

  test('reconciles removals and returns isolated placement payload copies', async () => {
    const cubeA = 'local/demo/a.cube';
    const type = `SugarCubes.Cube.${keyA}`;
    const { registry } = setup(
      [
        response(catalog('revision-1', [descriptor(keyA, cubeA)])),
        response(catalog('revision-2', [])),
      ],
      { [cubeA]: response(payload(cubeA)) },
    );
    await registry.refresh();
    const first = registry.preparedPayload(type);
    const second = registry.preparedPayload(type);

    expect(first).not.toBe(second);
    expect(first).toEqual(second);
    if (first?.cube) first.cube.cube_id = 'mutated';
    expect(registry.preparedPayload(type)?.cube?.cube_id).toBe(cubeA);

    const removed = await registry.refresh();
    expect(removed.changed).toBe(true);
    expect(removed.definitions).toEqual([]);
    expect(registry.preparedPayload(type)).toBeNull();
  });

  test('preserves the last known-good snapshot when catalog refresh fails', async () => {
    const cubeA = 'local/demo/a.cube';
    const type = `SugarCubes.Cube.${keyA}`;
    const { registry } = setup(
      [response(catalog('revision-1', [descriptor(keyA, cubeA)])), response({}, false)],
      { [cubeA]: response(payload(cubeA)) },
    );
    await registry.refresh();

    await expect(registry.refresh({ force: true })).rejects.toThrow(
      'Cube picker catalog request failed',
    );

    expect(registry.revision).toBe('revision-1');
    expect(registry.preparedPayload(type)?.cube?.cube_id).toBe(cubeA);
  });

  test('rejects identity and version mismatches before advertising a definition', async () => {
    const cubeA = 'local/demo/a.cube';
    const { registry } = setup(
      [response(catalog('revision-1', [descriptor(keyA, cubeA, '2.0.0')]))],
      { [cubeA]: response(payload(cubeA, '1.0.0')) },
    );

    const result = await registry.refresh();

    expect(result.definitions).toEqual([]);
    expect(result.unavailableCubeIds).toEqual([cubeA]);
  });
});
