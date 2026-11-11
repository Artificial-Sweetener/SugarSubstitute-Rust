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
/** Render an editable menu value while preserving native input and caret identity during refresh. */
export interface ContextMenuInput {
  readonly value: string;
  readonly minimum: number;
  readonly maximum: number;
  readonly step: string;
  change(value: string): void;
}

/** Keep reusable menu input presentation independent of feature values and calculations. */
export function createContextMenuField(
  document: Document,
  entry: { readonly label: string; readonly disabled?: boolean; readonly input: ContextMenuInput },
  changed: () => void,
): { readonly root: HTMLElement; readonly input: HTMLInputElement; refresh(): void } {
  const root = document.createElement('label');
  root.dataset.scMenuField = '';
  root.style.cssText = 'display:flex;align-items:center;gap:12px;padding:6px 12px;';
  const label = document.createElement('span');
  const input = document.createElement('input');
  input.type = 'number';
  input.min = String(entry.input.minimum);
  input.max = String(entry.input.maximum);
  input.step = entry.input.step;
  input.style.cssText =
    'width:96px;margin-left:auto;color:inherit;background:transparent;border:1px solid var(--sc-border-subtle);font:inherit;';
  root.append(label, input);
  input.addEventListener('input', () => {
    entry.input.change(input.value);
    changed();
  });
  /** Update values without replacing the focused control or normalizing incomplete decimal input. */
  const refresh = (): void => {
    label.textContent = entry.label;
    input.setAttribute('aria-label', entry.label);
    input.disabled = entry.disabled ?? false;
    if (input.value !== entry.input.value) input.value = entry.input.value;
  };
  refresh();
  return { root, input, refresh };
}
