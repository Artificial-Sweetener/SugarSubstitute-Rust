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
/** Present a persisted accent color picker with validated hexadecimal editing. */
import { DEFAULT_ACCENT_COLOR, normalizeAccentColor } from './AccentPalette.js';

/** Build a settings row without retaining detached inputs or accepting invalid color text. */
export function createAccentColorSetting(
  documentRef: Document,
  color: string,
  save: (color: string) => void,
): HTMLElement {
  const row = documentRef.createElement('div');
  row.className = 'sugarcubes-settings-row sc-accent-setting';
  row.style.cssText = 'display:flex;align-items:center;gap:12px';
  const picker = documentRef.createElement('input');
  picker.type = 'color';
  picker.value = color;
  picker.setAttribute('aria-label', 'SugarCubes accent color');
  const text = documentRef.createElement('input');
  text.type = 'text';
  text.value = color;
  text.placeholder = DEFAULT_ACCENT_COLOR;
  text.setAttribute('aria-label', 'SugarCubes accent hex');
  text.maxLength = 7;
  const reset = documentRef.createElement('button');
  reset.type = 'button';
  reset.textContent = 'Reset';
  reset.className = 'p-button p-button-secondary';
  /** Publish only complete colors, keeping transient text edits local to the control. */
  const accept = (value: string): void => {
    const normalized = normalizeAccentColor(value);
    text.setCustomValidity(normalized ? '' : 'Enter a hexadecimal color such as #b58cff.');
    text.setAttribute('aria-invalid', normalized ? 'false' : 'true');
    if (!normalized) return;
    picker.value = text.value = normalized;
    save(normalized);
  };
  picker.addEventListener('input', () => accept(picker.value));
  text.addEventListener('input', () => {
    if (text.value.trim().length === 7 && normalizeAccentColor(text.value)) accept(text.value);
  });
  text.addEventListener('change', () => accept(text.value));
  reset.addEventListener('click', () => accept(DEFAULT_ACCENT_COLOR));
  row.append(picker, text, reset);
  return row;
}
