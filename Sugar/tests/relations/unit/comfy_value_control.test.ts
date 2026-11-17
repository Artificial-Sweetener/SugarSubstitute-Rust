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
import { expect, jest, test } from '@jest/globals';
import { comfyValueControl } from '../../../frontend/comfyui/ui/relations/infrastructure/ComfyValueControl.js';

const domain = {
  kind: 'number',
  minimum: 0,
  maximum: 2 ** 64,
  integer: true,
  step: 1,
} as const;

test('a shared seed is stepped by the function ComfyUI publishes, with the shared range', () => {
  const compute = jest.fn(() => 7);
  const step = comfyValueControl({
    comfyAPI: { valueControl: { computeNextControlledValue: compute } },
  });
  expect(step(domain, 42, 'randomize')).toBe(7);
  expect(compute).toHaveBeenCalledWith(
    { type: 'number', value: 42, options: { min: 0, max: 2 ** 64, step2: 1 } },
    'randomize',
  );
});

test('a seed keeps its value when ComfyUI returns none, and a missing function is reported', () => {
  const none = comfyValueControl({
    comfyAPI: { valueControl: { computeNextControlledValue: () => undefined } },
  });
  expect(none(domain, 42, 'increment')).toBe(42);
  expect(() => comfyValueControl({})(domain, 42, 'randomize')).toThrow(
    'ComfyUI value control is unavailable.',
  );
});
