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
/** Join parsed wildcard references and prepared catalog results into immutable source diagnostics. */
import type { SourceRange, WildcardSpan } from '../../../core/src/index.js';
import type { WildcardReference, WildcardResolution } from './contracts.js';

/** Missing-reference metadata retains authored identity and the catalog's available canonical headers. */
export interface WildcardDiagnosticPayload extends WildcardReference {
  readonly matchedCsvColumn: null;
  readonly availableColumns: readonly string[];
}

/** One confirmed error uses exact-source UTF-16 bounds; incomplete catalogs produce no finding. */
export interface WildcardDiagnostic {
  readonly kind: 'wildcard';
  readonly severity: 'error';
  readonly range: SourceRange;
  readonly message: string;
  readonly payload: WildcardDiagnosticPayload;
}

/** Prepare one source-backed finding from an already-resolved catalog row; source absence and missing CSV columns remain distinct. */
export function prepareWildcardDiagnostic(
  span: WildcardSpan,
  resolution: WildcardResolution,
): WildcardDiagnostic | null {
  if (resolution.status !== 'missing') return null;
  const message =
    span.form === 'csv' && resolution.reason === 'column'
      ? `Missing CSV wildcard column: ${span.identifier}:${span.csvColumn}`
      : `Missing wildcard: ${span.identifier}`;
  return Object.freeze({
    kind: 'wildcard',
    severity: 'error',
    range: span.outerRange,
    message,
    payload: Object.freeze({
      form: span.form,
      identifier: span.identifier,
      csvColumn: span.csvColumn,
      matchedCsvColumn: null,
      availableColumns: resolution.availableColumns,
    }),
  });
}
