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
/** Convert an imported picture, already stretched to its surface, into mask coverage. */
/**
 * Read each RGBA pixel's gray level as coverage, ignoring alpha, with the integer weights of a
 * Qt grayscale conversion: `(11 r + 16 g + 5 b) / 32`. White is fully masked.
 */
export function grayscaleCoverage(rgba) {
    if (rgba.length % 4)
        throw new RangeError('Mask import pixels must be RGBA.');
    const coverage = new Uint8Array(rgba.length / 4);
    for (let index = 0; index < coverage.length; index++) {
        const offset = index * 4;
        coverage[index] =
            ((rgba[offset] ?? 0) * 11 + (rgba[offset + 1] ?? 0) * 16 + (rgba[offset + 2] ?? 0) * 5) >> 5;
    }
    return coverage;
}
