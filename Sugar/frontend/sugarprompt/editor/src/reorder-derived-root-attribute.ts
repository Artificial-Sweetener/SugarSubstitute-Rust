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
/** Exact native root metadata owned by reorder publication is excluded from appearance inputs and pending foreign-mutation rejection. */
/** Match only issued paint-mask and primary-grab output; null and unknown attributes remain foreign input. */
export function isReorderDerivedRootAttribute(name: string | null): boolean {
  return (
    name === 'data-reorder-grab' ||
    name === 'data-reorder-motion' ||
    name === 'data-reorder-motion-issue' ||
    name === 'data-reorder-motion-scope' ||
    name === 'data-reorder-motion-coverage' ||
    name === 'data-reorder-pointer-paint'
  );
}
