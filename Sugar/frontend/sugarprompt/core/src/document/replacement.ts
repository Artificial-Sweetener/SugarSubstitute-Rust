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
/** Pure exact-source replacement preparation; native transactions and request lifetimes belong to callers. */
import { requireValue } from '../required.js';
import { utf16ToCodePoint, type SourceRange } from '../offsets.js';
import { validateSourceRange } from '../normalization/ranges.js';
import { freezeStructure } from '../prompt/text.js';
import { prepareWildcardCsvEncoding } from './csv.js';
import { validateDocumentKind } from './semantics.js';
import type { PromptDocumentKind, PreparedDocumentValues } from './contracts.js';
import { documentValueById, prepareDocumentValues } from './values.js';
import { decodedRangeToSource, sourceRangeToDecoded } from './value-mapping.js';
/** Closed replacement intent; range is half-open exact-source UTF-16 and text is decoded literal input. */
export interface DocumentReplacementRequest {
  readonly source: string;
  readonly kind: PromptDocumentKind;
  readonly range: SourceRange;
  readonly text: string;
}
/** Stable failure codes retain no source; ambiguous targets include storage syntax and partial escaped characters. */
export type DocumentReplacementRejection =
  | 'invalid-input'
  | 'invalid-range'
  | 'invalid-boundary'
  | 'malformed-csv'
  | 'header-target'
  | 'cross-value'
  | 'ambiguous-target'
  | 'unavailable-target';
/** CSV identity and edited half-open UTF-16 range in the complete decoded cell, including retained padding. */
export interface DecodedCsvTarget {
  readonly row: number;
  readonly column: number;
  readonly range: SourceRange;
}
/** A source-local edit and absolute post-edit exact-source UTF-16 caret, or an explicit rejection. */
export type DocumentReplacementResult =
  | {
      readonly status: 'ready';
      readonly range: SourceRange;
      readonly text: string;
      readonly caret: number;
      readonly target: DecodedCsvTarget | null;
      /** Actual validated post-edit values/maps; unchanged output retains the supplied snapshot. */
      readonly preparedValues: PreparedDocumentValues;
    }
  | {
      readonly status: 'rejected';
      readonly reason: DocumentReplacementRejection;
    };
/** Snapshot exact enumerable own data fields, without invoking accessors or inherited fields.
 * Reflective proxies obey the same descriptor contract; trap failures remain observable.
 */
function ownFields(value: unknown, keys: readonly string[]): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const actual = Reflect.ownKeys(value);
  if (
    actual.length !== keys.length ||
    actual.some((key) => typeof key !== 'string' || !keys.includes(key))
  )
    return null;
  const fields: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !descriptor.enumerable || !Object.hasOwn(descriptor, 'value')) return null;
    fields[key] = descriptor.value as unknown;
  }
  return fields;
}
/** Validate only this closed boundary; existing kind/range/offset owners decide domain validity. */
function request(value: unknown): DocumentReplacementRequest | null {
  const fields = ownFields(value, ['kind', 'range', 'source', 'text']);
  if (!fields || typeof fields.source !== 'string' || typeof fields.text !== 'string') return null;
  try {
    validateDocumentKind(fields.kind);
  } catch (error) {
    if (error instanceof TypeError) return null;
    throw error;
  }
  const range = ownFields(fields.range, ['from', 'to']);
  if (!range || typeof range.from !== 'number' || typeof range.to !== 'number') return null;
  return {
    source: fields.source,
    text: fields.text,
    kind: fields.kind,
    range: { from: range.from, to: range.to },
  };
}
/** Delegate exact newline and scalar boundary validation; expected range errors never mask invariant failures. */
function boundaryFailure(source: string, range: SourceRange): DocumentReplacementRejection | null {
  if (
    !Number.isSafeInteger(range.from) ||
    !Number.isSafeInteger(range.to) ||
    range.from < 0 ||
    range.to < range.from ||
    range.to > source.length
  )
    return 'invalid-range';
  try {
    validateSourceRange(source, range);
    utf16ToCodePoint(source, range.from);
    utf16ToCodePoint(source, range.to);
  } catch (error) {
    if (error instanceof RangeError) return 'invalid-boundary';
    throw error;
  }
  return null;
}
/** Prepare one edit without normalizing source or repairing malformed CSV, CRLF, or scalar seams.
 * CSV ranges must align with one trimmed non-header value, including both endpoints.
 * The returned edit replaces only that cell's storage; target coordinates refer to its full decoded value.
 * Carets follow inserted decoded text and precede retained trailing padding/closing quotes.
 * Required CSV quoting retains empty records and separates existing record endings.
 */
export function prepareDocumentReplacement(
  input: unknown,
  prepared?: PreparedDocumentValues,
): DocumentReplacementResult {
  const intent = request(input);
  if (!intent) return { status: 'rejected', reason: 'invalid-input' };
  const { source, kind, range, text } = intent;
  const failure = boundaryFailure(source, range);
  if (failure) return { status: 'rejected', reason: failure };
  if (prepared && (prepared.source !== source || prepared.kind !== kind))
    return { status: 'rejected', reason: 'unavailable-target' };
  if (kind !== 'wildcard-csv') {
    const updated = source.slice(0, range.from) + text + source.slice(range.to);
    const caret = range.from + text.length;
    if (boundaryFailure(updated, { from: range.from, to: caret }))
      return { status: 'rejected', reason: 'invalid-boundary' };
    return freezeStructure({
      status: 'ready',
      range,
      text,
      caret,
      target: null,
      preparedValues: prepareDocumentValues(updated, kind, prepared),
    });
  }
  const values = prepared ?? prepareDocumentValues(source, kind);
  const document = values.csv;
  if (!document) throw new Error('missing-prepared-csv');
  if (!document.valid) return { status: 'rejected', reason: 'malformed-csv' };
  const candidates = document.records.flatMap((record, row) =>
    record.flatMap((cell, column) =>
      cell.range.from <= range.from && range.to <= cell.range.to ? [{ cell, row, column }] : [],
    ),
  );
  if (candidates.length > 1) return { status: 'rejected', reason: 'ambiguous-target' };
  const candidate = candidates[0];
  if (!candidate) {
    const intersects = document.records
      .flat()
      .some(
        (cell) =>
          range.from < range.to && cell.range.from <= range.to && range.from <= cell.range.to,
      );
    return {
      status: 'rejected',
      reason: intersects ? 'cross-value' : 'unavailable-target',
    };
  }
  const { cell, row, column } = candidate;
  if (row === 0) return { status: 'rejected', reason: 'header-target' };
  const selected = documentValueById(values, source, kind, `csv-cell:${row}:${column}`);
  if (!selected) throw new Error('missing-prepared-value');
  const decoded = sourceRangeToDecoded(selected.mapping, range);
  if (!decoded) return { status: 'rejected', reason: 'ambiguous-target' };
  const decodedRange = {
    from: selected.decodedRange.from + decoded.from,
    to: selected.decodedRange.from + decoded.to,
  };
  const value = cell.value.slice(0, decodedRange.from) + text + cell.value.slice(decodedRange.to);
  const decodedCaret = decodedRange.from + text.length;
  if (boundaryFailure(value, { from: decodedRange.from, to: decodedCaret }))
    return { status: 'rejected', reason: 'invalid-boundary' };
  let encoded = prepareWildcardCsvEncoding(cell, value).text;
  /** Check all decoded identities against the prepared original, including outer record seams and EOF. */
  const preservesStructure = (updated: NonNullable<PreparedDocumentValues['csv']>): boolean =>
    updated.valid &&
    updated.records.length === document.records.length &&
    document.records.every(
      (record, r) =>
        record.length === requireValue(updated.records[r], 'replacement').length &&
        record.every(
          (original, c) =>
            requireValue(requireValue(updated.records[r], 'replacement')[c], 'replacement')
              .value === (r === row && c === column ? value : original.value),
        ),
    );
  let updatedSource = source.slice(0, cell.range.from) + encoded + source.slice(cell.range.to);
  let updatedValues = prepareDocumentValues(updatedSource, kind, values);
  if (!updatedValues.csv) throw new Error('missing-post-edit-csv');
  if (!preservesStructure(updatedValues.csv) && !cell.quoted) {
    encoded = prepareWildcardCsvEncoding(cell, value, true).text;
    updatedSource = source.slice(0, cell.range.from) + encoded + source.slice(cell.range.to);
    updatedValues = prepareDocumentValues(updatedSource, kind, values);
  }
  const updated = updatedValues.csv;
  if (!updated) throw new Error('missing-post-edit-csv');
  if (!preservesStructure(updated)) return { status: 'rejected', reason: 'invalid-boundary' };
  const updatedCell = requireValue(
    requireValue(updated.records[row], 'replacement')[column],
    'replacement',
  );
  if (
    updatedCell.range.from !== cell.range.from ||
    updatedCell.range.to !== cell.range.from + encoded.length
  )
    return { status: 'rejected', reason: 'invalid-boundary' };
  const updatedMapping = updatedValues.containersById[`csv-container:${row}:${column}`]?.mapping;
  if (!updatedMapping) throw new Error('missing-post-edit-container-map');
  const mappedCaret = decodedRangeToSource(updatedMapping, {
    from: decodedCaret,
    to: decodedCaret,
  });
  if (!mappedCaret) return { status: 'rejected', reason: 'invalid-boundary' };
  const caret = mappedCaret.from;
  if (
    boundaryFailure(updatedSource, {
      from: cell.range.from,
      to: cell.range.from + encoded.length,
    }) ||
    boundaryFailure(updatedSource, { from: caret, to: caret })
  )
    return { status: 'rejected', reason: 'invalid-boundary' };
  return freezeStructure({
    status: 'ready',
    range: cell.range,
    text: encoded,
    caret,
    target: { row, column, range: decodedRange },
    preparedValues: updatedValues,
  });
}
