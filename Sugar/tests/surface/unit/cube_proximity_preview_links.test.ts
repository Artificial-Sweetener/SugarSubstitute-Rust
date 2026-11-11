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
/** Characterize the preview-link projection extracted from host registration. */
import { expect, test } from '@jest/globals';
import { projectCubeProximityPreviewLinks } from '../../../frontend/comfyui/ui/surface/CubeProximityPreviewLinks.js';

test('preserves accepted slot and endpoint identity and excludes disabled or incomplete prompt links', () => {
  const valid = { outputId: 'source', outputSlot: 2, inputId: 'target' };
  expect(
    projectCubeProximityPreviewLinks(true, [
      valid,
      { ...valid, outputId: undefined },
      { ...valid, inputId: undefined },
    ]),
  ).toEqual([{ originId: 'source', originSlot: 2, targetId: 'target' }]);
  expect(projectCubeProximityPreviewLinks(false, [valid])).toEqual([]);
  expect(valid).toEqual({ outputId: 'source', outputSlot: 2, inputId: 'target' });
});
