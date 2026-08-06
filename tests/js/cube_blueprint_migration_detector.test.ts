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
/** Verify legacy Blueprint detection is explicit, one-shot, and non-destructive. */

import { jest } from '@jest/globals';
import { CubeBlueprintMigrationDetector } from '../../frontend/comfyui/ui/affordance/CubeBlueprintMigrationDetector.js';
import { isCubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

test('reports a Sugar-marked Blueprint once and leaves it ordinary graph data', () => {
  class LegacyBlueprintNode {
    static comfyClass = 'SubgraphBlueprint.LegacyCube';
    properties = {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'legacy-instance' },
    };
  }
  const legacy = new LegacyBlueprintNode();
  const graph = { _nodes: [legacy] };
  const feedback = { push: jest.fn() };
  const logger = { warn: jest.fn() };
  const detector = new CubeBlueprintMigrationDetector({ feedback, logger });

  expect(detector.scan(graph)).toBe(1);
  expect(detector.scan(graph)).toBe(0);
  expect(graph._nodes).toEqual([legacy]);
  expect(isCubeNode(legacy)).toBe(false);
  expect(logger.warn).toHaveBeenCalledTimes(1);
  expect(feedback.push).toHaveBeenCalledWith(
    'warn',
    'Legacy SugarCube Blueprint detected',
    expect.stringContaining('ordinary Subgraph data'),
  );
});
