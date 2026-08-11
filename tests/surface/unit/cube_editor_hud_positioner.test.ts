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
/** Verify the Cube editor HUD observer cannot wake itself indefinitely. */

import { jest } from '@jest/globals';
import { CubeEditorHudPositioner } from '../../../frontend/comfyui/ui/surface/CubeEditorHudPositioner.js';

test('does not rewrite unchanged owned style properties after observing itself', async () => {
  document.body.replaceChildren();
  const root = document.createElement('aside');
  document.body.append(root);
  const setProperty = jest.spyOn(root.style, 'setProperty');
  const positioner = new CubeEditorHudPositioner(document, root);

  await Promise.resolve();
  await Promise.resolve();
  expect(setProperty).toHaveBeenCalledTimes(2);

  document.body.append(document.createElement('div'));
  await Promise.resolve();
  await Promise.resolve();
  expect(setProperty).toHaveBeenCalledTimes(2);
  positioner.dispose();
});
