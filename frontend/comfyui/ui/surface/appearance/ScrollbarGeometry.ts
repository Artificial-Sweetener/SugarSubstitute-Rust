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
/** Calculate scrollbar travel independently of DOM layout and native scrolling. */
export interface ScrollbarGeometry {
  length: number;
  offset: number;
  travel: number;
  maximum: number;
}

/** Keep a bounded minimum handle and clamp overscroll to the authoritative range. */
export function scrollbarGeometry(
  track: number,
  viewport: number,
  content: number,
  position: number,
): ScrollbarGeometry | null {
  if (![track, viewport, content, position].every(Number.isFinite)) return null;
  if (track <= 0 || viewport <= 0 || content <= viewport) return null;
  const maximum = content - viewport;
  const length = Math.min(track, Math.max(24, (track * viewport) / content));
  const travel = track - length;
  return {
    length,
    travel,
    maximum,
    offset: (Math.min(maximum, Math.max(0, position)) / maximum) * travel,
  };
}

/** Map pointer movement to scrolling without dividing by an immovable handle. */
export function scrollbarDragPosition(
  geometry: ScrollbarGeometry,
  initial: number,
  distance: number,
): number {
  if (geometry.travel <= 0) return initial;
  return Math.min(
    geometry.maximum,
    Math.max(0, initial + (distance / geometry.travel) * geometry.maximum),
  );
}
