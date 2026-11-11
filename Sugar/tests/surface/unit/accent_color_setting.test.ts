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
/** Prove the menu accepts complete colors while preserving transient and invalid edits. */
import { expect, jest, test } from '@jest/globals';
import { createAccentColorSetting } from '../../../frontend/comfyui/ui/surface/appearance/AccentColorSetting.js';
import { DEFAULT_ACCENT_COLOR } from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

test('normalizes valid edits, synchronizes the picker, and resets through the same persistence path', () => {
  const save = jest.fn<(color: string) => void>();
  const row = createAccentColorSetting(document, '#b58cff', save);
  const text = row.querySelector<HTMLInputElement>('input[type="text"]')!;
  const picker = row.querySelector<HTMLInputElement>('input[type="color"]')!;
  expect(text.value).toBe('#b58cff');
  expect(save).not.toHaveBeenCalled();
  text.value = '#ABC';
  text.dispatchEvent(new Event('change'));
  expect(save).toHaveBeenLastCalledWith('#aabbcc');
  expect(picker.value).toBe('#aabbcc');
  picker.value = '#ffe066';
  picker.dispatchEvent(new Event('input'));
  expect(text.value).toBe('#ffe066');
  expect(save).toHaveBeenLastCalledWith('#ffe066');
  text.value = '#123456';
  text.dispatchEvent(new Event('input'));
  expect(save).toHaveBeenLastCalledWith('#123456');
  expect(picker.value).toBe('#123456');
  row.querySelector<HTMLButtonElement>('button')!.click();
  expect(save).toHaveBeenLastCalledWith(DEFAULT_ACCENT_COLOR);
  expect(text.value).toBe(DEFAULT_ACCENT_COLOR);
});

test('rejects invalid edits without changing the selected color or persisting incomplete text', () => {
  const save = jest.fn<(color: string) => void>();
  const row = createAccentColorSetting(document, '#b58cff', save);
  const text = row.querySelector<HTMLInputElement>('input[type="text"]')!;
  const picker = row.querySelector<HTMLInputElement>('input[type="color"]')!;
  text.value = '#f';
  text.dispatchEvent(new Event('input'));
  expect(save).not.toHaveBeenCalled();
  text.value = '#123';
  text.dispatchEvent(new Event('input'));
  expect(text.value).toBe('#123');
  expect(save).not.toHaveBeenCalled();
  text.value = '#f';
  text.dispatchEvent(new Event('change'));
  expect(text.getAttribute('aria-invalid')).toBe('true');
  expect(text.validationMessage).toContain('hexadecimal');
  expect(picker.value).toBe('#b58cff');
  text.value = '#123456';
  text.dispatchEvent(new Event('change'));
  expect(text.getAttribute('aria-invalid')).toBe('false');
  expect(text.validationMessage).toBe('');
  expect(save).toHaveBeenCalledTimes(1);
});
