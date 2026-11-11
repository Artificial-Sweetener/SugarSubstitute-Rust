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
/** Prompt Control LoRA fields with exact weight and block-weight ownership. */
import type { LoraSpan } from './contracts.js';
import { loraWeightFields } from './lora-fields.js';

/** Parse a balanced angle group; all arguments use exact-source UTF-16 offsets. */
export function loraSpan(source: string, from: number, close: number): LoraSpan | null {
  if (!source.startsWith('lora:', from + 1)) return null;
  const nameFrom = from + 6,
    nameTo = source.indexOf(':', nameFrom);
  if (nameTo <= nameFrom || nameTo >= close || /[<>:]/u.test(source.slice(nameFrom, nameTo)))
    return null;
  const fields = loraWeightFields(source, nameTo, close);
  if (!fields) return null;
  return {
    outerRange: { from, to: close + 1 },
    nameRange: { from: nameFrom, to: nameTo },
    firstWeightRange: fields.first.range,
    firstWeight: fields.first.value,
    secondWeightRange: fields.second?.range ?? null,
    secondWeight: fields.second?.value ?? null,
    blockWeightsRange: fields.blockWeightsRange,
    depth: 0,
  };
}
