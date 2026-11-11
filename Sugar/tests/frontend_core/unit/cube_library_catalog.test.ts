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
/** Safeguard library metadata used by saving, authoring, and source presentation. */
import { describe, expect, jest, test } from '@jest/globals';
import { CubeLibraryCatalog } from '../../../frontend/comfyui/ui/core/CubeLibraryCatalog.js';
import type { ApiJsonResult } from '../../../frontend/comfyui/ui/core/CubeLibraryApi.js';

/** Build a data-only harness without mounting the obsolete browser. */
function harness(list: (options?: RequestInit) => Promise<ApiJsonResult>) {
  const catalog = new CubeLibraryCatalog({
    api: { list },
    logger: { warn: jest.fn() },
    onUpdated: () => {},
  });
  return {
    catalog: {
      refresh: (force: boolean) => catalog.refresh({ force }),
      getCubeById: (id: string) => catalog.getCubeById(id),
    },
    entries: () => catalog.getCubes(),
  };
}

describe('library metadata catalog', () => {
  test('normalizes model targets while retaining writable source and fork lineage metadata', async () => {
    const source = { type: 'local', namespace: 'personal' };
    const lineage = { parent_cube_id: 'github/example/pack/Anima/base.cube' };
    const { catalog, entries } = harness(async () => ({
      response: { ok: true },
      data: {
        cubes: [
          {
            cube_id: 'local/personal/Anima/demo.cube',
            target_model: ' Anima ',
            supported_models: [' Anima ', 'SDXL'],
            is_writable: true,
            source,
            lineage,
          },
          null,
        ],
      },
    }));
    await catalog.refresh(true);
    expect(entries()).toHaveLength(1);
    expect(catalog.getCubeById('local/personal/Anima/demo.cube')).toMatchObject({
      target_model: 'Anima',
      supported_models: ['Anima', 'SDXL'],
      is_writable: true,
      source,
      lineage,
    });
    expect(catalog.getCubeById('missing')).toBeNull();
  });

  test('coalesces concurrent refreshes and retains the previous catalog after failure', async () => {
    const list = jest.fn<(options?: RequestInit) => Promise<ApiJsonResult>>();
    list.mockResolvedValueOnce({
      response: { ok: true },
      data: { cubes: [{ cube_id: 'local/personal/demo.cube' }] },
    });
    list.mockRejectedValueOnce(new Error('offline'));
    const { catalog, entries } = harness(list);
    await Promise.all([catalog.refresh(true), catalog.refresh(true)]);
    expect(list).toHaveBeenCalledTimes(1);
    await catalog.refresh(true);
    expect(entries().map((entry) => entry.cube_id)).toEqual(['local/personal/demo.cube']);
  });

  test('caches an empty successful catalog until explicitly invalidated', async () => {
    const list = jest.fn(async () => ({ response: { ok: true }, data: { cubes: [] } }));
    const { catalog } = harness(list);
    await catalog.refresh(false);
    await catalog.refresh(false);
    expect(list).toHaveBeenCalledTimes(1);
    await catalog.refresh(true);
    expect(list).toHaveBeenCalledTimes(2);
  });

  test('rejects duplicate identities without publishing a partial replacement', async () => {
    const onUpdated = jest.fn();
    const logger = { warn: jest.fn() };
    const list = jest.fn<() => Promise<ApiJsonResult>>();
    list
      .mockResolvedValueOnce({
        response: { ok: true },
        data: { cubes: [{ cube_id: 'local/personal/first.cube' }] },
      })
      .mockResolvedValueOnce({
        response: { ok: true },
        data: { cubes: [{ cube_id: 'duplicate' }, { cube_id: 'duplicate' }] },
      });
    const catalog = new CubeLibraryCatalog({ api: { list }, logger, onUpdated });
    await catalog.refresh();
    await catalog.refresh({ force: true });
    expect(catalog.getCubes().map((entry) => entry.cube_id)).toEqual(['local/personal/first.cube']);
    expect(onUpdated).toHaveBeenCalledTimes(1);
    expect(logger.warn).toHaveBeenCalledTimes(1);
  });

  test('does not publish a late transport result after disposal', async () => {
    let resolve: ((result: ApiJsonResult) => void) | undefined;
    let signal: AbortSignal | null | undefined;
    const onUpdated = jest.fn();
    const catalog = new CubeLibraryCatalog({
      api: {
        list: (options) => {
          signal = options?.signal;
          return new Promise<ApiJsonResult>((complete) => {
            resolve = complete;
          });
        },
      },
      logger: { warn: jest.fn() },
      onUpdated,
    });
    const refresh = catalog.refresh();
    catalog.dispose();
    expect(signal?.aborted).toBe(true);
    if (!resolve) throw new Error('Catalog transport was not started');
    resolve({ response: { ok: true }, data: { cubes: [{ cube_id: 'late' }] } });
    await refresh;
    expect(catalog.getCubes()).toEqual([]);
    expect(onUpdated).not.toHaveBeenCalled();
  });
});
