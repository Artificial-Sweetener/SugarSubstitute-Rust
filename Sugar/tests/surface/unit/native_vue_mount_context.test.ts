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
/** Prove isolated native Vue roots cannot share portal IDs or alter the host context. */
import { expect, test } from '@jest/globals';
import { scopeNativeVueMountContext } from '../../../frontend/comfyui/ui/surface/NativeVueMountContext.js';

test('retains host services and gives independently rendered roots distinct ID namespaces', () => {
  const context = {
    config: { idPrefix: 'comfy', globalProperties: {} },
    provides: { service: {} },
  };
  const first = scopeNativeVueMountContext(context),
    second = scopeNativeVueMountContext(context);
  expect(first.provides).toBe(context.provides);
  expect(second.provides).toBe(context.provides);
  expect(first.config).not.toEqual(second.config);
  expect(context.config.idPrefix).toBe('comfy');
});
