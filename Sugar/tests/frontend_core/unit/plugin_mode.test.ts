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
/** Prove Plugin Mode transitions are idempotent and scoped to their named browser boundary. */
import { expect, jest, test } from '@jest/globals';
import {
  publishPluginMode,
  readPluginMode,
  subscribePluginMode,
} from '../../../frontend/comfyui/ui/core/PluginMode.js';

test('publishes accepted mode changes once and releases subscribers', () => {
  publishPluginMode(document, false);
  const changed = jest.fn();
  const stop = subscribePluginMode(document, changed);
  publishPluginMode(document, true);
  publishPluginMode(document, true);
  expect(readPluginMode(document)).toBe(true);
  expect(changed).toHaveBeenCalledTimes(1);
  stop();
  publishPluginMode(document, false);
  expect(changed).toHaveBeenCalledTimes(1);
});
