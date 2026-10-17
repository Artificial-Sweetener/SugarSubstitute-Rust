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
/** Isolate FAST custom-element rendering at the JSDOM boundary; browser proofs use Microsoft's implementation. */

/** Supply a menu container while preserving the shared owner's real DOM and geometry policy. */
export function createMenuList(document: Document): HTMLElement {
  return document.createElement('div');
}

/** Supply a focusable row while shared interaction tests exercise real events and feature callbacks. */
export function createMenuItem(document: Document): HTMLElement {
  return document.createElement('button');
}

/** Reflect checked presentation at the mocked third-party component boundary. */
export function setMenuItemChecked(row: HTMLElement, checked: boolean): void {
  row.setAttribute('aria-checked', String(checked));
  if (checked) row.setAttribute('checked', '');
}

/** Preserve the known slots while testing the real shared menu owner. */
export function menuItemSlot(_document: Document, position: 'start' | 'end'): string {
  return position;
}
