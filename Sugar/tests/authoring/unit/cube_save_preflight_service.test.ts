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
/** Verify invalid nested state is rejected before the broad save workflow starts. */

import { describe, expect, jest, test } from '@jest/globals';
import { CubeSavePreflightService } from '../../../frontend/comfyui/ui/save/CubeSavePreflightService.js';

describe('CubeSavePreflightService', () => {
  test('delegates a valid save unchanged', async () => {
    const outcome = { status: 'saved' as const, savedCubeIds: ['demo.cube'] };
    const save = jest.fn(async () => outcome);
    const service = new CubeSavePreflightService({
      workflow: { save, saveImplementation: save },
      validate: () => undefined,
    });

    await expect(service.save({ cubeIds: ['demo.cube'] })).resolves.toBe(outcome);
    expect(save).toHaveBeenCalledWith({ cubeIds: ['demo.cube'] });
  });

  test('returns a failed outcome and feedback before any save mutation', async () => {
    const save = jest.fn(async () => ({ status: 'saved' as const, savedCubeIds: [] }));
    const feedback = { push: jest.fn() };
    const service = new CubeSavePreflightService({
      workflow: { save, saveImplementation: save },
      validate: () => {
        throw new Error('Remove the nested SugarCube wrapper.');
      },
      feedback,
    });

    await expect(service.save()).resolves.toEqual({
      status: 'failed',
      savedCubeIds: [],
      message: 'Remove the nested SugarCube wrapper.',
    });
    expect(save).not.toHaveBeenCalled();
    expect(feedback.push).toHaveBeenCalledWith(
      'error',
      'SugarCube save blocked',
      'Remove the nested SugarCube wrapper.',
    );
  });
});
