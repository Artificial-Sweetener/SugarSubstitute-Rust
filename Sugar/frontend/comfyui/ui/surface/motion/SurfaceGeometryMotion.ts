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
/** Publish identified geometry-motion lifecycles for observers that consume settled layouts. */
export const SURFACE_GEOMETRY_MOTION_EVENT = 'sugarcubes:surface-geometry-motion';

/** Mark one owned moving region and release deferred geometry on completion or cancellation. */
export function setSurfaceGeometryMoving(element: HTMLElement, moving: boolean): void {
  if (element.hasAttribute('data-sc-geometry-moving') === moving) return;
  if (moving) element.dataset.scGeometryMoving = '';
  else delete element.dataset.scGeometryMoving;
  element.dispatchEvent(new Event(SURFACE_GEOMETRY_MOTION_EVENT, { bubbles: true }));
}
