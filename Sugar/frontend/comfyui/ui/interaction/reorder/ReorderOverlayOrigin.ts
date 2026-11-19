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
/** Find where a lifted card's fixed overlay is measured from inside its host. */

/**
 * Return the viewport position of the origin that `position: fixed` uses inside `host`.
 *
 * That origin is the viewport's corner unless an ancestor, such as a dialog with a transform,
 * makes itself the containing block. A lifted card is drawn from this origin, so the caller
 * subtracts it from the card's viewport position to keep the card under the pointer.
 */
export function fixedOverlayOrigin(host: HTMLElement): { readonly x: number; readonly y: number } {
  const probe = host.ownerDocument.createElement('div');
  probe.style.cssText =
    'position:fixed;left:0;top:0;width:0;height:0;visibility:hidden;pointer-events:none;';
  host.append(probe);
  const bounds = probe.getBoundingClientRect();
  probe.remove();
  return { x: bounds.left, y: bounds.top };
}
