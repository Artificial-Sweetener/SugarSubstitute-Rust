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
/** Exact decimal formatting uses string/integer arithmetic, independent of floating-point/context limits. */
import { requireValue } from '../required.js';
/** Increment a nonnegative decimal coefficient without imposing a fixed numeric width. */
function increment(digits: string): string {
  const characters = [...Array.from(digits)];
  for (let index = characters.length - 1; index >= 0; index--) {
    if (characters[index] !== '9') {
      characters[index] = String(Number(characters[index]) + 1);
      return characters.join('');
    }
    characters[index] = '0';
  }
  return '1' + characters.join('');
}
/** Decode one parser-produced decimal once for the common exact formatting and fixed-step owner. */
function decimalParts(value: string): {
  readonly sign: string;
  readonly coefficient: string;
  readonly scale: number;
} {
  const matched = /^(-?)(\d+)(?:\.(\d+))?(?:E([+-]?\d+))?$/u.exec(value);
  if (!matched) throw new TypeError('Invalid parsed decimal');
  const sign = requireValue(matched[1], 'decimals'),
    fraction = matched[3] ?? '';
  const coefficient = (requireValue(matched[2], 'decimals') + fraction).replace(/^0+/u, '') || '0';
  const scale = fraction.length - Number(matched[4] ?? 0);
  if (!Number.isSafeInteger(scale)) throw new RangeError('Invalid decimal scale');
  return { sign, coefficient, scale };
}
/** Quantize a parser-produced exact decimal to two places using half-even rounding and signed zero. */
export function formatWeight(value: string): string {
  const { sign, coefficient, scale } = decimalParts(value);
  let rounded: string;
  if (scale <= 2) rounded = coefficient + '0'.repeat(2 - scale);
  else {
    const cut = scale - 2,
      boundary = coefficient.length - cut;
    rounded = boundary > 0 ? coefficient.slice(0, boundary) : '0';
    const remainder = boundary >= 0 ? coefficient.slice(boundary) : '0';
    const greater =
      requireValue(remainder[0], 'decimals') > '5' ||
      (remainder.startsWith('5') && /[1-9]/u.test(remainder.slice(1)));
    const tie = remainder.startsWith('5') && !/[1-9]/u.test(remainder.slice(1));
    if (greater || (tie && Number(rounded.at(-1)) % 2 === 1)) rounded = increment(rounded);
  }
  rounded = rounded.padStart(3, '0');
  return sign + rounded.slice(0, -2) + '.' + rounded.slice(-2);
}
/** Compute the exact 1.1-per-shell multiplier with at least two fractional digits. */
export function implicitWeight(depth: number): string {
  if (!Number.isSafeInteger(depth) || depth < 1)
    throw new RangeError('Invalid implicit emphasis depth');
  const coefficient = (11n ** BigInt(depth)).toString().padStart(depth + 1, '0');
  return coefficient.slice(0, -depth) + '.' + coefficient.slice(-depth).padEnd(2, '0');
}
/** Add two parser-produced decimals exactly; quantization belongs solely to the explicit final formatWeight call. */
export function addWeights(left: string, right: string): string {
  const a = decimalParts(left),
    b = decimalParts(right);
  const precision = Math.max(0, a.scale, b.scale);
  const first = BigInt(a.coefficient) * 10n ** BigInt(precision - a.scale);
  const second = BigInt(b.coefficient) * 10n ** BigInt(precision - b.scale);
  const sum = (a.sign === '-' ? -first : first) + (b.sign === '-' ? -second : second);
  const digits = (sum < 0n ? -sum : sum).toString().padStart(precision + 1, '0');
  return (
    (sum < 0n ? '-' : '') +
    (precision ? digits.slice(0, -precision) + '.' + digits.slice(-precision) : digits)
  );
}
/** Add one signed 0.05 step before rounding through the sole exact decimal arithmetic owner. */
export function stepWeight(value: string, direction: 'increase' | 'decrease'): string {
  return formatWeight(addWeights(value, direction === 'increase' ? '0.05' : '-0.05'));
}
