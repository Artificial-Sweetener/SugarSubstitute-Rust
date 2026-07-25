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
/** Verify all Cube import paths share one reporting policy. */

import { jest } from '@jest/globals';
import { CubeImportOutcomeReporter } from '../../frontend/comfyui/ui/import/CubeImportOutcomeReporter.js';
import type { ImportResult } from '../../frontend/comfyui/ui/import/CubeImportTypes.js';

test('reports warnings, success, and native Cube-node focus exactly once', () => {
  const pushToast = jest.fn();
  const focusImportedCube = jest.fn();
  const reporter = new CubeImportOutcomeReporter({ pushToast, focusImportedCube });
  const result: ImportResult = {
    success: true,
    summary: 'cube 1, internal nodes 9',
    message: '',
    warnings: ['native definition normalized'],
    missingTypes: [],
    nodesAdded: 0,
    markersAdded: 0,
    connectionsMade: 0,
    primaryNodeId: 'surface-instance',
    bounds: { minX: 10, minY: 20, maxX: 910, maxY: 660 },
  };

  const outcome = reporter.report('SDXL/Text to Image', ['backend warning'], result, {
    cube: { cube_id: 'sdxl.cube' },
  });

  expect(pushToast.mock.calls).toEqual([
    ['warn', 'SugarCube import warnings', 'backend warning'],
    ['warn', 'SugarCube import notes', 'native definition normalized'],
    ['success', 'Imported SDXL/Text to Image', 'cube 1, internal nodes 9'],
  ]);
  expect(focusImportedCube).toHaveBeenCalledWith(result);
  expect(outcome).toEqual({
    summary: 'cube 1, internal nodes 9',
    frontendWarnings: ['native definition normalized'],
  });
});

test('reports incomplete placement without focusing', () => {
  const pushToast = jest.fn();
  const focusImportedCube = jest.fn();
  const reporter = new CubeImportOutcomeReporter({ pushToast, focusImportedCube });

  reporter.report(
    'Broken',
    [],
    {
      success: false,
      message: 'Graph unavailable',
      summary: '',
      warnings: [],
    },
    {},
  );

  expect(pushToast).toHaveBeenCalledWith(
    'warn',
    'SugarCube Broken import incomplete',
    'Graph unavailable',
  );
  expect(focusImportedCube).not.toHaveBeenCalled();
});
