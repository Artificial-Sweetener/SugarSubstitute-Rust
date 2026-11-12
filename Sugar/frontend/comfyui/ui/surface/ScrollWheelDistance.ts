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
/** Normalize wheel axes and units consistently for scroll surfaces and their Fluent handles. */

/** Preserve explicit horizontal input and map a vertical wheel to a horizontal-only surface. */
export function scrollWheelDistance(
  event: Pick<WheelEvent, 'deltaX' | 'deltaY' | 'deltaMode'>,
  axis: 'vertical' | 'horizontal',
  page: number,
): number {
  const factor = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? page : 1;
  const delta = axis === 'vertical' ? event.deltaY : event.deltaX || event.deltaY;
  return delta * factor;
}
