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
/** Scheduler-safe model insertion and friendly LoRA display suffixes are pure source transformations. */
import { requireValue } from '../required.js';
import type { LoraQuery, LoraReplacement } from './lora-contracts.js';
import { assertPreparedLoraQuery } from './lora-query.js';
import { assertCompletionBoundary } from './boundaries.js';
import { strip, caseFold } from '../prompt/text.js';
import { parseWeight } from '../prompt/weights.js';
/** Preserve a valid supplied numeric spelling; absent, empty and malformed weights use the reference's 1.00 default. */
export function loraScheduleWeight(weight: string | null | undefined): string {
  if (weight === null || weight === undefined) return '1.00';
  if (typeof weight !== 'string') throw new TypeError('Invalid LoRA weight input');
  const value = strip(weight);
  return value && parseWeight(value, true) !== null ? value : '1.00';
}
/** Literal name-field content may include filename punctuation and whitespace, but cannot introduce scheduling fields, tokens or line endings. */
export function loraNameContent(text: string): boolean {
  // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
  return !/[<>:\r\n\u0000]/u.test(text);
}
/** Reject canonical identities that would inject scheduling fields or another token, preserving safe names literally. */
function canonicalName(name: string): string {
  if (typeof name !== 'string' || !strip(name) || name.length > 4096 || !loraNameContent(name))
    throw new TypeError('Invalid LoRA schedule identity');
  return name;
}
/** Build one complete schedule from the deliberate canonical prompt name, retaining the host's path spelling. */
export function loraScheduleText(name: string, weight?: string | null): string {
  return `<lora:${canonicalName(name)}:${loraScheduleWeight(weight)}>`;
}
/** Propose one exact-source model-name completion from this source's prepared query; valid first/second/block field spelling survives the model change. */
export function loraReplacement(source: string, query: LoraQuery, name: string): LoraReplacement {
  assertPreparedLoraQuery(source, query);
  const range = query.replacementRange;
  for (const position of [range.from, range.to, query.nameRange.from, query.nameRange.to])
    assertCompletionBoundary(source, position);
  if (
    range.from > query.nameRange.from ||
    query.nameRange.from > query.nameRange.to ||
    query.nameRange.to > range.to
  )
    throw new RangeError('Invalid LoRA completion range');
  return Object.freeze({
    range: Object.freeze({ ...range }),
    text:
      query.preservedSuffix === null
        ? loraScheduleText(name, query.typedWeightText)
        : `<lora:${canonicalName(name)}${query.preservedSuffix}>`,
  });
}
/** Compare scalar characters with the donor's case-insensitive space/underscore and path-separator equivalence. */
function suffix(candidate: string, prefix: string): string {
  const proposed = Array.from(candidate),
    typed = Array.from(prefix);
  if (typed.length > proposed.length) return '';
  for (const [index, character] of typed.entries()) {
    const left = caseFold(character.replaceAll('\\', '/')),
      right = caseFold(requireValue(proposed[index], 'lora-text').replaceAll('\\', '/'));
    if (left !== right && !((left === '_' && right === ' ') || (left === ' ' && right === '_')))
      return '';
  }
  return proposed.slice(typed.length).join('');
}
/** Friendly display completion takes precedence; explicit path prefixes may instead preview the canonical path remainder. */
export function loraCompletionSuffix(query: string, display: string, name: string): string {
  if (!query) return display;
  const displaySuffix = suffix(display, query);
  return displaySuffix || (/[\\/]/u.test(query) ? suffix(name, query) : '');
}
