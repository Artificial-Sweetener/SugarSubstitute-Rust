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
/** Filter soft coverage in bounded linear work without thresholding its existing feather. */

/** Apply signed pixel-radius morphology while retaining grayscale coverage. */
export function growMaskCoverage(
  source: Uint8Array,
  width: number,
  height: number,
  amount: number,
): Uint8Array {
  if (!amount) return source;
  const radius = Math.abs(amount),
    grow = amount > 0;
  const horizontal = extrema(source, width, height, radius, grow, false);
  return extrema(horizontal, width, height, radius, grow, true);
}

/** Soften coverage with a separable box filter, excluding samples outside the input frame. */
export function featherMaskCoverage(
  source: Uint8Array,
  width: number,
  height: number,
  radius: number,
): Uint8Array {
  if (!radius) return source;
  const horizontal = new Float32Array(source.length),
    result = new Uint8Array(source.length);
  for (let y = 0; y < height; y++) {
    let sum = 0,
      start = 0,
      end = 0;
    for (let x = 0; x < width; x++) {
      const left = Math.max(0, x - radius),
        right = Math.min(width, x + radius + 1);
      while (end < right) sum += source[y * width + end++] ?? 0;
      while (start < left) sum -= source[y * width + start++] ?? 0;
      horizontal[y * width + x] = sum / (right - left);
    }
  }
  for (let x = 0; x < width; x++) {
    let sum = 0,
      start = 0,
      end = 0;
    for (let y = 0; y < height; y++) {
      const top = Math.max(0, y - radius),
        bottom = Math.min(height, y + radius + 1);
      while (end < bottom) sum += horizontal[end++ * width + x] ?? 0;
      while (start < top) sum -= horizontal[start++ * width + x] ?? 0;
      result[y * width + x] = Math.round(sum / (bottom - top));
    }
  }
  return result;
}

/** Use a monotonic window so large grow/shrink values do not multiply per-pixel work. */
function extrema(
  source: Uint8Array,
  width: number,
  height: number,
  radius: number,
  grow: boolean,
  vertical: boolean,
): Uint8Array {
  const length = vertical ? height : width,
    lines = vertical ? width : height;
  const result = new Uint8Array(source.length),
    queue = new Int32Array(length);
  for (let line = 0; line < lines; line++) {
    let head = 0,
      tail = 0,
      end = 0;
    const offset = (position: number): number =>
      vertical ? position * width + line : line * width + position;
    for (let position = 0; position < length; position++) {
      const right = Math.min(length, position + radius + 1);
      while (end < right) {
        const value = source[offset(end)] ?? 0;
        while (tail > head) {
          const previous = source[offset(queue[tail - 1] ?? 0)] ?? 0;
          if (grow ? previous > value : previous < value) break;
          tail--;
        }
        queue[tail++] = end++;
      }
      while (tail > head && (queue[head] ?? 0) < position - radius) head++;
      result[offset(position)] = source[offset(queue[head] ?? 0)] ?? 0;
    }
  }
  return result;
}
