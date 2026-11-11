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
/** Resolve dimension-row gestures independently of theme annotations and native value ownership. */
import type { NativeEditorField } from '../infrastructure/NativeEditorCardAdapter.js';
import { inferDimensionPairs } from '../domain/DimensionPolicy.js';

/** Include each field's label and padding, using the nearest column only within a real shared row. */
export function dimensionFieldTarget(
  root: HTMLElement,
  fields: readonly NativeEditorField[],
  target: Element,
  x: number,
  y: number,
): NativeEditorField | null {
  const direct = fields.find((field) => field.element !== root && field.element.contains(target));
  if (direct) return direct;
  for (const pair of inferDimensionPairs(fields.map((field) => field.name))) {
    const width = fields.find((field) => field.name === pair.width);
    const height = fields.find((field) => field.name === pair.height);
    if (
      !width ||
      !height ||
      width.kind !== 'number' ||
      height.kind !== 'number' ||
      width.element === root ||
      height.element === root ||
      width.element.parentElement !== target ||
      height.element.parentElement !== target
    )
      continue;
    const first = width.element.getBoundingClientRect();
    const second = height.element.getBoundingClientRect();
    const top = Math.max(first.top, second.top),
      bottom = Math.min(first.bottom, second.bottom);
    if (
      first.width <= 0 ||
      second.width <= 0 ||
      bottom <= top ||
      y < top ||
      y > bottom ||
      x < Math.min(first.left, second.left) ||
      x > Math.max(first.right, second.right)
    )
      continue;
    const widthDistance = Math.abs(x - (first.left + first.right) / 2);
    const heightDistance = Math.abs(x - (second.left + second.right) / 2);
    return widthDistance <= heightDistance ? width : height;
  }
  return null;
}
