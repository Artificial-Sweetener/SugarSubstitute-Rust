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
/** Place owned preview accessories around an identified native body without moving its fields. */
import { relocateControl } from '../core/ControlPlacement.js';

/** Keep previews inside native disclosure geometry and controls after the native field stack. */
export function reconcileNativeNodeBodyAccessories(
  root: HTMLElement,
  before: readonly HTMLElement[],
  after: readonly HTMLElement[],
): void {
  const native = root.querySelector<HTMLElement>('.lg-node');
  const body = native?.querySelector<HTMLElement>('[data-testid^="node-body-"]');
  const parent = body?.parentElement;
  if (!body || !parent) {
    for (const element of before) if (!root.contains(element)) root.append(element);
    return;
  }
  let next: Node = body;
  for (const element of [...before].reverse()) {
    if (element.parentNode !== parent || element.nextSibling !== next)
      relocateControl(element, parent, next);
    next = element;
  }
  let suffix: Node | null = body.nextSibling;
  while (suffix instanceof HTMLElement && after.includes(suffix)) suffix = suffix.nextSibling;
  for (const element of [...after].reverse()) {
    if (element.parentNode !== parent || element.nextSibling !== suffix)
      relocateControl(element, parent, suffix);
    suffix = element;
  }
}
