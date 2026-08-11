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
import { CubeVersionAvailabilityService } from '../../../frontend/comfyui/ui/cube/version/CubeVersionAvailabilityService.js';
import { CubeVersionRepository } from '../../../frontend/comfyui/ui/cube/version/CubeVersionRepository.js';
import type { ApiJsonResult } from '../../../frontend/comfyui/ui/core/CubeLibraryApi.js';

const result = (data: Record<string, unknown>, ok = true): ApiJsonResult => ({
  response: { ok },
  data,
});

describe('CubeVersionRepository', () => {
  test('projects backend-owned unique versions and loads exact artifacts', async () => {
    const listRevisions = jest.fn(async () =>
      result({ version_revisions: [{ version: '2.0.0', revision_ref: 'WORKTREE' }] }),
    );
    const load = jest.fn(async () => result({ cube: { cube_id: 'cube.test', version: '2.0.0' } }));
    const loadRevision = jest.fn(async () => result({}));
    const repository = new CubeVersionRepository({ listRevisions, load, loadRevision });

    const options = await repository.listOptions('cube.test', '1.0.0');
    const payload = await repository.loadArtifact('cube.test', options[0]!);

    expect(options.map((option) => option.value)).toEqual(['2.0.0']);
    expect(payload.cube?.version).toBe('2.0.0');
    expect(load).toHaveBeenCalledWith(
      JSON.stringify({ cube_id: 'cube.test', origin: { x: 0, y: 0 } }),
      { headers: { 'Content-Type': 'application/json' } },
    );
    expect(loadRevision).not.toHaveBeenCalled();
  });

  test('coalesces concurrent option discovery and retries failures', async () => {
    const listOptions = jest
      .fn<(cubeId: string, fallbackVersion: string) => Promise<never[]>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue([]);
    const availability = new CubeVersionAvailabilityService({ listOptions });

    await expect(availability.list('cube.test', '1.0.0')).rejects.toThrow('offline');
    await expect(availability.list('cube.test', '1.0.0')).resolves.toEqual([]);
    expect(listOptions).toHaveBeenCalledTimes(2);
  });

  test('omits loader-rejected revisions and preserves one checked-safe option set', async () => {
    const listRevisions = jest.fn(async () =>
      result({
        version_revisions: [
          { version: '2.0.0', revision_ref: 'WORKTREE' },
          { version: '1.0.0', revision_ref: 'legacy-ref' },
        ],
      }),
    );
    const load = jest.fn(async () => result({ cube: { cube_id: 'cube.test', version: '2.0.0' } }));
    const loadRevision = jest.fn(async () =>
      result(
        {
          error: {
            message: 'Unsafe subgraph widget snapshot',
            detail: 'Positional values have no stable names.',
          },
        },
        false,
      ),
    );
    const logger = { warn: jest.fn() };
    const repository = new CubeVersionRepository({ listRevisions, load, loadRevision }, logger);

    const options = await repository.listOptions('cube.test', '2.0.0');
    await repository.loadArtifact('cube.test', options[0]!);

    expect(options.map((option) => option.value)).toEqual(['2.0.0']);
    expect(load).toHaveBeenCalledTimes(1);
    expect(loadRevision).toHaveBeenCalledTimes(1);
    expect(logger.warn).toHaveBeenCalledWith(
      expect.stringContaining("omitted unavailable Cube version 'v1.0.0'"),
      expect.objectContaining({
        message: 'Unsafe subgraph widget snapshot Positional values have no stable names.',
      }),
    );
  });
});
