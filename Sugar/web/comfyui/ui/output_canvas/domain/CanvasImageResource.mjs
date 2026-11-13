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
/** Define browser image admissions through opaque payload and handle ports. */
/** Count estimated RGBA storage without claiming control over browser/GPU caches. */
export function canvasImageBytes(dimensions) {
    if (![dimensions.width, dimensions.height].every((value) => Number.isSafeInteger(value) && value > 0))
        throw new RangeError('Canvas decoded dimensions are invalid.');
    const bytes = dimensions.width * dimensions.height * 4;
    if (!Number.isSafeInteger(bytes))
        throw new RangeError('Canvas decoded estimate is invalid.');
    return bytes;
}
