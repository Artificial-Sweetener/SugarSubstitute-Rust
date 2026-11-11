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
/** Verify root placement depends only on authoritative graph object identity. */

import { describe, expect, test } from '@jest/globals';
import { ComfyCubeGraphScope } from '../../../frontend/comfyui/ui/cube/placement/ComfyCubeGraphScope.js';
import {
  CubeRootPlacementError,
  CubeRootPlacementPolicy,
} from '../../../frontend/comfyui/ui/cube/placement/CubeRootPlacementPolicy.js';

describe('CubeRootPlacementPolicy', () => {
  test('allows only the exact root graph object', () => {
    const root = { id: 'same-name' };
    const nested = { id: 'same-name', rootGraph: root };
    const policy = new CubeRootPlacementPolicy(root);

    expect(policy.allows(root)).toBe(true);
    expect(policy.allows(nested)).toBe(false);
    expect(() => policy.assertAllowed(nested, 'created')).toThrow(CubeRootPlacementError);
    expect(() => policy.assertAllowed(null)).toThrow('top-level workflow');
  });

  test('resolves the current canvas graph at command time', () => {
    const root = {};
    const nested = {};
    let current: object = root;
    const scope = new ComfyCubeGraphScope(root, () => current);

    expect(scope.isCurrentRoot()).toBe(true);
    scope.assertCurrentRoot('imported');
    current = nested;
    expect(scope.isCurrentRoot()).toBe(false);
    expect(() => scope.assertCurrentRoot('imported')).toThrow('top-level workflow');
  });
});
