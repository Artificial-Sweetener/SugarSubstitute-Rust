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
/** Prepared scalar-safe translation between decoded values and exact authored source. */
import { requireValue } from '../required.js';
import { codePointBoundaries, type SourceRange } from '../offsets.js';
import { encodeSource, type LineEnding } from '../line-endings.js';
import { isSourceBoundary, validateSourceRange } from '../normalization/ranges.js';
import { freezeStructure, strip } from '../prompt/text.js';
/** Dense scalar conversion tables shared by all values in one exact source snapshot. */
export interface PreparedSourceOffsets {
  readonly source: string;
  readonly pythonToUtf16: readonly number[];
  readonly utf16ToPython: readonly (number | null)[];
  /** Exact physical newline spellings from the existing newline authority; no logical text buffer is retained. */
  readonly endings: readonly LineEnding[];
}
/** Immutable half-open UTF-16 mapping; null entries reject scalar, CRLF and escape interiors.
 * Inverse indices are relative to sourceRange.from, not absolute document positions.
 */
export interface PreparedValueMapping {
  readonly text: string;
  readonly sourceRange: SourceRange;
  readonly decodedToSource: readonly (number | null)[];
  readonly sourceToDecoded: readonly (number | null)[];
  readonly pythonToDecoded: readonly number[];
  readonly decodedToPython: readonly (number | null)[];
  readonly sourceOffsets: PreparedSourceOffsets;
}
/** Retain scalar conversion output once; lone surrogates follow the existing offset authority. */
export function prepareSourceOffsets(source: string): PreparedSourceOffsets {
  const pythonToUtf16 = codePointBoundaries(source);
  const singleWidthScalars = pythonToUtf16.length === source.length + 1;
  const inverse: (number | null)[] | null = singleWidthScalars
    ? null
    : Array.from({ length: source.length + 1 }, () => null);
  if (inverse)
    pythonToUtf16.forEach((position, index) => {
      inverse[position] = index;
    });
  return Object.freeze({
    source,
    pythonToUtf16: Object.freeze(pythonToUtf16),
    utf16ToPython: Object.freeze(inverse ?? pythonToUtf16),
    endings: Object.freeze(encodeSource(source).endings),
  });
}
/** Donor trimming preserves an all-whitespace value's empty anchor after its existing padding. */
export function trimmedValueRange(text: string): SourceRange {
  const from = text.length - strip(text, 'left').length;
  return Object.freeze({
    from,
    to: Math.max(from, strip(text, 'right').length),
  });
}
/** Derive unencoded boundaries directly and reuse the complete source's scalar tables. */
export function prepareIdentityMapping(
  sourceOffsets: PreparedSourceOffsets,
  text: string,
  from: number,
): PreparedValueMapping {
  const sourceRange = { from, to: from + text.length };
  validateSourceRange(sourceOffsets.source, sourceRange);
  if (
    sourceOffsets.utf16ToPython[from] == null ||
    sourceOffsets.utf16ToPython[sourceRange.to] == null
  )
    throw new Error('invalid-value-scalar-extents');
  const complete = from === 0 && text === sourceOffsets.source;
  const pythonToDecoded = complete ? sourceOffsets.pythonToUtf16 : codePointBoundaries(text);
  const identityCoordinates =
    complete && pythonToDecoded.length === text.length + 1 && !text.includes('\r\n');
  const inverse: (number | null)[] | null = complete
    ? null
    : Array.from({ length: text.length + 1 }, () => null);
  const decodedBoundaries: (number | null)[] | null = identityCoordinates
    ? null
    : Array.from({ length: text.length + 1 }, () => null);
  const sourceBoundaries: (number | null)[] | null =
    from === 0 ? decodedBoundaries : Array.from({ length: text.length + 1 }, () => null);
  if (decodedBoundaries && sourceBoundaries)
    pythonToDecoded.forEach((position, python) => {
      if (inverse) inverse[position] = python;
      const absolute = from + position;
      if (
        !isSourceBoundary(text, position) ||
        !isSourceBoundary(sourceOffsets.source, absolute) ||
        sourceOffsets.utf16ToPython[absolute] == null
      )
        return;
      decodedBoundaries[position] = absolute;
      sourceBoundaries[position] = position;
    });
  return Object.freeze({
    text,
    sourceRange: Object.freeze(sourceRange),
    decodedToSource: Object.freeze(decodedBoundaries ?? pythonToDecoded),
    sourceToDecoded: Object.freeze(sourceBoundaries ?? pythonToDecoded),
    pythonToDecoded: Object.freeze(pythonToDecoded),
    decodedToPython: Object.freeze(inverse ?? sourceOffsets.utf16ToPython),
    sourceOffsets: freezeStructure(sourceOffsets),
  });
}
/** Prepare dense bidirectional output from parser-owned UTF-16 extents; no decoding is repeated. */
export function prepareValueMapping(
  sourceOffsets: PreparedSourceOffsets,
  text: string,
  characterRanges: readonly SourceRange[],
  emptyAnchor: number,
): PreparedValueMapping {
  if (characterRanges.length !== text.length) throw new Error('invalid-value-extents');
  const sourceRange = {
    from: characterRanges[0]?.from ?? emptyAnchor,
    to: characterRanges.at(-1)?.to ?? emptyAnchor,
  };
  validateSourceRange(sourceOffsets.source, sourceRange);
  if (
    sourceOffsets.utf16ToPython[sourceRange.from] == null ||
    sourceOffsets.utf16ToPython[sourceRange.to] == null
  )
    throw new Error('invalid-value-scalar-extents');
  for (let index = 0; index < characterRanges.length; index++) {
    const range = requireValue(characterRanges[index], 'value-mapping');
    if (
      range.from >= range.to ||
      range.from < sourceRange.from ||
      range.to > sourceRange.to ||
      (index > 0 && requireValue(characterRanges[index - 1], 'value-mapping').to !== range.from)
    )
      throw new Error('invalid-value-extents');
  }
  const pythonToDecoded = codePointBoundaries(text);
  const decodedToPython: (number | null)[] = Array.from({ length: text.length + 1 }, () => null);
  const decodedToSource: (number | null)[] = Array.from({ length: text.length + 1 }, () => null);
  const sourceToDecoded: (number | null)[] = Array.from(
    { length: sourceRange.to - sourceRange.from + 1 },
    () => null,
  );
  pythonToDecoded.forEach((position, python) => {
    decodedToPython[position] = python;
    const absolute = characterRanges[position]?.from ?? sourceRange.to;
    if (!isSourceBoundary(text, position) || !isSourceBoundary(sourceOffsets.source, absolute))
      return;
    if (sourceOffsets.utf16ToPython[absolute] == null) return;
    decodedToSource[position] = absolute;
    sourceToDecoded[absolute - sourceRange.from] = position;
  });
  return Object.freeze({
    text,
    sourceRange: Object.freeze(sourceRange),
    decodedToSource: Object.freeze(decodedToSource),
    sourceToDecoded: Object.freeze(sourceToDecoded),
    pythonToDecoded: Object.freeze(pythonToDecoded),
    decodedToPython: Object.freeze(decodedToPython),
    sourceOffsets: freezeStructure(sourceOffsets),
  });
}
/** Reject malformed half-open numeric bounds before indexing prepared tables. */
function validRange(range: SourceRange, maximum: number): boolean {
  return (
    Number.isSafeInteger(range.from) &&
    Number.isSafeInteger(range.to) &&
    range.from >= 0 &&
    range.from <= range.to &&
    range.to <= maximum
  );
}
/** Translate decoded UTF-16 boundaries; source quoting, padding and escape interiors have no semantic positions. */
export function decodedRangeToSource(
  mapping: PreparedValueMapping,
  range: SourceRange,
): SourceRange | null {
  if (!validRange(range, mapping.text.length)) return null;
  const from = mapping.decodedToSource[range.from],
    to = mapping.decodedToSource[range.to];
  return from == null || to == null ? null : Object.freeze({ from, to });
}
/** Translate absolute exact-source UTF-16 boundaries; inverse gaps reject instead of clamping. */
export function sourceRangeToDecoded(
  mapping: PreparedValueMapping,
  range: SourceRange,
): SourceRange | null {
  if (!validRange(range, mapping.sourceRange.to) || range.from < mapping.sourceRange.from)
    return null;
  const from = mapping.sourceToDecoded[range.from - mapping.sourceRange.from],
    to = mapping.sourceToDecoded[range.to - mapping.sourceRange.from];
  return from == null || to == null ? null : Object.freeze({ from, to });
}
/** Translate Python code-point bounds in decoded text to exact-source UTF-16. */
export function pythonRangeToSource(
  mapping: PreparedValueMapping,
  range: SourceRange,
): SourceRange | null {
  if (!validRange(range, mapping.pythonToDecoded.length - 1)) return null;
  return decodedRangeToSource(mapping, {
    from: requireValue(mapping.pythonToDecoded[range.from], 'value-mapping'),
    to: requireValue(mapping.pythonToDecoded[range.to], 'value-mapping'),
  });
}
/** Translate exact-source UTF-16 to decoded Python code points, preserving all inverse rejections. */
export function sourceRangeToPython(
  mapping: PreparedValueMapping,
  range: SourceRange,
): SourceRange | null {
  const decoded = sourceRangeToDecoded(mapping, range);
  if (!decoded) return null;
  return Object.freeze({
    from: requireValue(mapping.decodedToPython[decoded.from], 'value-mapping'),
    to: requireValue(mapping.decodedToPython[decoded.to], 'value-mapping'),
  });
}
/** Translate exact-source Python code points to decoded UTF-16 using the shared source scalar table. */
export function sourcePythonRangeToDecoded(
  mapping: PreparedValueMapping,
  range: SourceRange,
): SourceRange | null {
  const boundaries = mapping.sourceOffsets.pythonToUtf16;
  if (!validRange(range, boundaries.length - 1)) return null;
  return sourceRangeToDecoded(mapping, {
    from: requireValue(boundaries[range.from], 'value-mapping'),
    to: requireValue(boundaries[range.to], 'value-mapping'),
  });
}
/** Translate decoded UTF-16 to exact-source Python code points without rebuilding scalar conversions. */
export function decodedRangeToSourcePython(
  mapping: PreparedValueMapping,
  range: SourceRange,
): SourceRange | null {
  const source = decodedRangeToSource(mapping, range);
  if (!source) return null;
  return Object.freeze({
    from: requireValue(mapping.sourceOffsets.utf16ToPython[source.from], 'value-mapping'),
    to: requireValue(mapping.sourceOffsets.utf16ToPython[source.to], 'value-mapping'),
  });
}
