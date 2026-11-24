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
/** Prove the Accent color row's controls show, choose and restore colours as Python's row does. */
import { expect, jest, test } from '@jest/globals';
import { createAccentColorControls } from '../../../frontend/comfyui/ui/surface/appearance/AccentColorSetting.js';
import { DEFAULT_ACCENT_COLOR } from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

function buttons(element: HTMLElement): string[] {
  return [...element.querySelectorAll('button')].map((button) => button.textContent ?? '');
}

test('shows the swatch and hex value, saves a chosen colour, and restores the default', () => {
  const save = jest.fn<(color: string) => void>();
  const controls = createAccentColorControls(document, '#b58cff', save, true);
  const swatch = controls.preview.querySelector<HTMLElement>('.sc-accent-swatch')!;
  const hex = controls.preview.querySelector('.sc-accent-hex')!;
  expect(hex.textContent).toBe('#b58cff');
  expect(swatch.style.background).not.toBe('');
  expect(controls.preview.querySelector('input')).toBeNull();
  expect(buttons(controls.actions)).toEqual(['Choose', 'Default']);
  const picker = controls.actions.querySelector<HTMLInputElement>('input[type="color"]')!;
  picker.value = '#ffe066';
  picker.dispatchEvent(new Event('input'));
  expect(save).toHaveBeenLastCalledWith('#ffe066');
  expect(hex.textContent).toBe('#ffe066');
  [...controls.actions.querySelectorAll('button')][1]!.click();
  expect(save).toHaveBeenLastCalledWith(DEFAULT_ACCENT_COLOR);
  expect(hex.textContent).toBe(DEFAULT_ACCENT_COLOR);
});

test('Choose opens the colour picker; a host with a system accent offers no Default button', () => {
  const controls = createAccentColorControls(document, '#b58cff', jest.fn(), false);
  expect(buttons(controls.actions)).toEqual(['Choose']);
  const picker = controls.actions.querySelector<HTMLInputElement>('input[type="color"]')!;
  const opened = jest.fn();
  picker.showPicker = opened;
  controls.actions.querySelector('button')!.click();
  expect(opened).toHaveBeenCalledTimes(1);
  controls.update('#107c10', true);
  expect(controls.preview.querySelector('.sc-accent-hex')?.textContent).toBe('#107c10');
  expect(controls.actions.querySelector('button')!.disabled).toBe(true);
});
