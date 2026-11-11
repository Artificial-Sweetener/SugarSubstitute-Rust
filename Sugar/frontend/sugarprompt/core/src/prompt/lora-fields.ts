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
/** One owner parses LoRA numeric and optional block fields for syntax and model-name completion. */
import type { SourceRange } from '../offsets.js';
import { weightField, type WeightField } from './weights.js';

/** Numeric spelling and exact-source UTF-16 field ranges remain independent of the model name. */
export interface LoraWeightFields {
  readonly first: WeightField;
  readonly second: WeightField | null;
  readonly blockWeightsRange: SourceRange | null;
}

/** Parse fields following the name's colon, with an exclusive body end in exact-source UTF-16. */
export function loraWeightFields(
  source: string,
  separator: number,
  bodyEnd: number,
): LoraWeightFields | null {
  /** Find the next owned field separator without crossing the token body. */
  const fieldEnd = (start: number): number => {
    const colon = source.indexOf(':', start);
    return colon < 0 || colon >= bodyEnd ? bodyEnd : colon;
  };
  const firstEnd = fieldEnd(separator + 1),
    first = weightField(source, separator + 1, firstEnd, true);
  if (!first) return null;
  let second: WeightField | null = null,
    blockWeightsRange: SourceRange | null = null;
  if (firstEnd < bodyEnd) {
    const secondFrom = firstEnd + 1,
      secondEnd = fieldEnd(secondFrom);
    second = weightField(source, secondFrom, secondEnd, true);
    if (!second) blockWeightsRange = { from: secondFrom, to: bodyEnd };
    else if (secondEnd < bodyEnd) blockWeightsRange = { from: secondEnd + 1, to: bodyEnd };
  }
  return { first, second, blockWeightsRange };
}
