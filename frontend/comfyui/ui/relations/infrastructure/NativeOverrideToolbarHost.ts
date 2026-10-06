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
/** Reserve a workflow layout row after native Run controls without moving Comfy-owned DOM. */
export function attachNativeOverrideToolbar(document: Document, element: HTMLElement): boolean {
  const canvas = document.getElementById('graph-canvas-container');
  const actionbar = canvas?.querySelector('.actionbar-container');
  const panel = actionbar?.closest('.p-splitterpanel');
  if (!panel || !panel.querySelector('.graph-canvas-panel')) {
    element.remove();
    return false;
  }
  let runRow = actionbar;
  while (runRow?.parentElement && runRow.parentElement !== panel) runRow = runRow.parentElement;
  if (!runRow) return false;
  if (element.parentElement !== panel || element.previousElementSibling !== runRow)
    panel.insertBefore(element, runRow.nextSibling);
  return true;
}
