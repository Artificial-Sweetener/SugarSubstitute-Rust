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
/** Parse lossless prompt decimals without binary floating-point conversion. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import { decimalZeros } from './unicode-decimal-data.js';
/** A recognized weight field in exact-source UTF-16 coordinates. */
export interface WeightField {
  readonly range: SourceRange;
  readonly value: string;
}
/** Normalize Unicode decimal digits supported by Python 3.12's Unicode 15.0 database. */
function asciiDigits(text: string): string | null {
  let result = '';
  for (const character of text) {
    if ('+-.'.includes(character)) {
      result += character;
      continue;
    }
    const point = requireValue(character.codePointAt(0), 'weights');
    const zero = decimalZeros.find((start) => point >= start && point < start + 10);
    if (zero === undefined) return null;
    result += String(point - zero);
  }
  return result;
}
/** Match Decimal's exact spelling, including signed zero, precision and small-value exponents. */
export function parseWeight(text: string, allowPlus: boolean): string | null {
  const ascii = asciiDigits(text);
  if (
    ascii === null ||
    !(allowPlus ? /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/u : /^-?(?:\d+(?:\.\d*)?|\.\d+)$/u).test(ascii)
  )
    return null;
  const sign = ascii.startsWith('-') ? '-' : '';
  const unsigned = ascii.replace(/^[+-]/u, '');
  const [integer = '', fraction = ''] = unsigned.split('.');
  const coefficient = (integer + fraction).replace(/^0+/u, '') || '0';
  const adjusted = coefficient.length - 1 - fraction.length;
  if (adjusted < -6)
    return (
      sign +
      String(coefficient[0]) +
      (coefficient.length > 1 ? '.' + coefficient.slice(1) : '') +
      'E' +
      String(adjusted)
    );
  return sign + (integer.replace(/^0+/u, '') || '0') + (fraction ? '.' + fraction : '');
}
/** LoRA numeric ranges exclude leading spaces/tabs; emphasis fields retain strict boundaries. */
export function weightField(
  source: string,
  from: number,
  to: number,
  lora: boolean,
): WeightField | null {
  if (lora) while (from < to && /[ \t]/u.test(requireValue(source[from], 'weights'))) from++;
  const value = parseWeight(source.slice(from, to), lora);
  return value === null ? null : { range: { from, to }, value };
}
