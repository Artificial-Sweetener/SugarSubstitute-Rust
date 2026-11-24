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
/** Reflect a menu row's check into the selected library's checkmark presentation. */
import { readOwnedVisualStyle } from '../surface/appearance/VisualStyleBoundary.js';

/**
 * Show a row checked or unchecked. Open menus are refreshed as often as every frame, so only a
 * change is written.
 */
export function setMenuItemChecked(row: HTMLElement, checked: boolean): void {
  if (row.getAttribute('aria-checked') !== String(checked))
    row.setAttribute('aria-checked', String(checked));
  const presentation =
    readOwnedVisualStyle(row.ownerDocument) === 'spectrum' ? 'selected' : 'checked';
  if (row.hasAttribute(presentation) !== checked) row.toggleAttribute(presentation, checked);
}
