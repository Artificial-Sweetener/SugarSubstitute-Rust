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
/** Verify palette reads precede writes and remain shared across mounted model fields. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ModelPickerAppearance } from '../../../frontend/comfyui/ui/model_library/ModelPickerAppearance.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
  document.documentElement.removeAttribute('style');
});

test('shares the active palette across new fields and refreshes it after a host theme change', async () => {
  const readStyle = jest.spyOn(window, 'getComputedStyle');
  const appearance = new ModelPickerAppearance(document, {
    ui: { settings: { getSettingValue: () => true } },
  });
  appearance.activate();
  const paletteReads = () =>
    readStyle.mock.calls.filter(([root]) => root === document.documentElement).length;
  expect(paletteReads()).toBe(1);
  const fields = [document.createElement('div'), document.createElement('div')];
  document.body.append(...fields);
  for (const field of fields) appearance.track(field);
  expect(paletteReads()).toBe(1);
  expect(fields.every((field) => field.dataset.scModelAppearance === 'fluent')).toBe(true);
  document.documentElement.style.colorScheme = 'light';
  await Promise.resolve();
  expect(paletteReads()).toBe(2);
  appearance.release(fields[0]!);
  expect(fields[0]!.dataset.scModelAppearance).toBeUndefined();
  appearance.dispose();
  expect(fields[1]!.dataset.scModelAppearance).toBeUndefined();
});
