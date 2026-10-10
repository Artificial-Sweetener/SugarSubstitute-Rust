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
/** Verify pack changes invalidate independent Cube discovery catalogs. */

import { describe, expect, jest, test } from '@jest/globals';
import { CubeCatalogInvalidationCoordinator } from '../../../frontend/comfyui/ui/core/CubeCatalogInvalidationCoordinator.js';

describe('CubeCatalogInvalidationCoordinator', () => {
  test('refreshes the native picker and library metadata together', async () => {
    const refreshPicker = jest.fn(async () => undefined);
    const refreshLibrary = jest.fn(async () => undefined);
    const logger = { warn: jest.fn() };
    const coordinator = new CubeCatalogInvalidationCoordinator({
      refreshPicker,
      refreshLibrary,
      logger,
    });

    await coordinator.invalidate();

    expect(refreshPicker).toHaveBeenCalledTimes(1);
    expect(refreshLibrary).toHaveBeenCalledTimes(1);
    expect(logger.warn).not.toHaveBeenCalled();
  });

  test('reports each failure without suppressing the other refresh', async () => {
    const pickerError = new Error('picker failed');
    const libraryError = new Error('library failed');
    const logger = { warn: jest.fn() };
    const coordinator = new CubeCatalogInvalidationCoordinator({
      refreshPicker: async () => Promise.reject(pickerError),
      refreshLibrary: async () => Promise.reject(libraryError),
      logger,
    });

    await coordinator.invalidate();

    expect(logger.warn).toHaveBeenNthCalledWith(
      1,
      'SugarCubes: failed to refresh the native picker catalog.',
      { error: pickerError },
    );
    expect(logger.warn).toHaveBeenNthCalledWith(
      2,
      'SugarCubes: failed to refresh the library metadata catalog.',
      { error: libraryError },
    );
  });
});
