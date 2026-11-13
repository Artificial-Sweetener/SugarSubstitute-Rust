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
/** Render a restore action using the native seed popup's row presentation. */
import { createSeedActionIcon } from './SeedActionIcons.js';

/** Retain native popup spacing and theme colors while rendering only literal action text. */
export function createNativeSeedReuseRow(
  document: Document,
  reference: HTMLLabelElement,
  disabled: boolean,
  reuse: () => void,
  dismiss: () => void,
): HTMLButtonElement {
  const row = document.createElement('button');
  row.type = 'button';
  row.className = reference.className;
  row.dataset.scSeedOption = 'reuse-last';
  row.setAttribute('aria-label', 'Reuse last');
  row.disabled = disabled;
  const content = document.createElement('div');
  content.className = 'flex min-w-0 flex-1 items-center gap-2 text-wrap';
  const frame = document.createElement('div');
  frame.className =
    'flex size-8 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-secondary-background';
  frame.append(createSeedActionIcon(document, 'reuse-last', false));
  const label = document.createElement('span');
  label.className = 'text-sm/tight font-normal text-base-foreground';
  label.textContent = 'Reuse last';
  content.append(frame, label);
  row.append(content);
  row.addEventListener('click', reuse);
  row.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    dismiss();
  });
  return row;
}
