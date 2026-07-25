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
/** Verify graph-independent Cube workflow pre-configuration. */

import { jest } from '@jest/globals';

import { CubeWorkflowPreconfiguration } from '../../frontend/comfyui/ui/cube/CubeWorkflowPreconfiguration.js';
import type { LegacyCubeWorkflowExtractor } from '../../frontend/comfyui/ui/cube/migration/LegacyCubeWorkflowExtractor.js';

describe('CubeWorkflowPreconfiguration', () => {
  test('transfers legacy extraction exactly once without registering node types', () => {
    const batch = { plans: [{ key: 'legacy' }], connections: [], warnings: [] };
    const extractInPlace = jest.fn(() => batch);
    const service = new CubeWorkflowPreconfiguration({
      extractInPlace,
    } as unknown as LegacyCubeWorkflowExtractor);
    const workflow = {};

    expect(service.prepare(workflow)).toBe(1);
    expect(extractInPlace).toHaveBeenCalledWith(workflow);
    expect(service.takeLegacyBatch()).toBe(batch);
    expect(service.takeLegacyBatch()).toBeNull();
  });
});
