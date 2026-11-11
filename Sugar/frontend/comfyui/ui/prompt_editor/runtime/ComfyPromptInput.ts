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
/** Declare the native editing surface through Comfy's host-facing textual-input contract. */

/** Keep Comfy's capture-phase ChangeTracker from treating native editor history as workflow history. */
export function markComfyPromptInput(root: HTMLElement): void {
  const content = root.querySelector<HTMLElement>('.cm-content');
  if (!content || Reflect.get(content, 'type') === 'textarea') return;
  // Comfy's ChangeTracker explicitly duck-types type=textarea before queued keyboard history.
  Object.defineProperty(content, 'type', { configurable: true, value: 'textarea' });
}
