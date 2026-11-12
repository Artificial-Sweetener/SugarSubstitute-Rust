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
/** Canonical immutable document snapshot declarations shared by preparation, capabilities and lookup.
 * Algorithms consume these contracts; declarations never depend on their composing owners.
 * Source ranges are half-open exact-source UTF-16 unless explicitly documented as decoded UTF-16.
 */
import type { SourceRange } from '../offsets.js';
import type { PhysicalLine } from '../prompt/physical-lines.js';
import type { WildcardCsvCell, WildcardCsvDocument } from './csv.js';
import type { PreparedSourceOffsets, PreparedValueMapping } from './value-mapping.js';

/** Host-selected source meaning; candidate documents never acquire workflow generation scenes. */
export type PromptDocumentKind = 'prompt' | 'wildcard-txt' | 'wildcard-csv';

/** Immutable source-derived capability and diagnostic ranges use exact-source UTF-16. */
export interface PreparedDocumentSemantics {
  readonly kind: PromptDocumentKind;
  readonly scenesEnabled: boolean;
  readonly valid: boolean;
  readonly unsupportedSceneMarkerRanges: readonly SourceRange[];
  /** Current immutable enumeration, parser output, capabilities and scalar-safe maps. */
  readonly preparedValues: PreparedDocumentValues;
}

/** Candidate identities belong only to this exact source/kind snapshot; duplicate text remains separate. */
export interface PreparedDocumentValue {
  readonly id: string;
  readonly mapping: PreparedValueMapping;
  /** UTF-16 bounds in the complete decoded cell or physical line, including retained padding. */
  readonly decodedRange: SourceRange;
  readonly location:
    | { readonly kind: 'txt'; readonly line: number }
    | {
        readonly kind: 'csv';
        readonly row: number;
        readonly column: number;
        readonly cell: WildcardCsvCell;
      };
}

/** Prepared process state, not persistence; parser output and maps retain actual object identity. */
export interface PreparedDocumentValues {
  readonly source: string;
  readonly kind: PromptDocumentKind;
  readonly valid: boolean;
  readonly scenesEnabled: boolean;
  readonly usesStructuredPromptValues: boolean;
  readonly isolatesDuplicates: boolean;
  readonly values: readonly PreparedDocumentValue[];
  readonly byId: Readonly<Record<string, PreparedDocumentValue>>;
  readonly csv: WildcardCsvDocument | null;
  readonly sourceOffsets: PreparedSourceOffsets;
  /** Source-derived physical destinations; these are never semantic candidates. */
  readonly containers: readonly PreparedDocumentContainer[];
  readonly containersById: Readonly<Record<string, PreparedDocumentContainer>>;
  readonly containerOutcomes: Readonly<Record<string, DocumentContainerOutcome>>;
  readonly containerState: 'ready' | 'no-data' | 'header-only' | 'malformed';
}

/** Immutable physical storage; mapping coordinates are decoded UTF-16 and exact-source UTF-16/Python. */
export interface PreparedDocumentContainer {
  readonly id: string;
  /** Exact-source UTF-16 storage, including CSV quotes/padding but excluding TXT endings. */
  readonly storageRange: SourceRange;
  /** Full parser-owned decoded UTF-16 range, including retained padding. */
  readonly decodedRange: SourceRange;
  /** Decoded UTF-16 semantic extent or insertion anchor, including destinations with no semantic candidate. */
  readonly valueRange: SourceRange;
  /** Complete decoded UTF-16 content, including whitespace padding. Null only for malformed CSV. */
  readonly mapping: PreparedValueMapping | null;
  readonly value: PreparedDocumentValue | null;
  readonly editability: 'editable' | 'header' | 'malformed';
  readonly location:
    | { readonly kind: 'prompt' }
    | {
        readonly kind: 'txt';
        readonly line: number;
        readonly record: PhysicalLine;
      }
    | {
        readonly kind: 'csv';
        readonly row: number;
        readonly column: number;
        readonly cell: WildcardCsvCell;
      };
}

/** Closed admission distinguishes source context, snapshot identity, protected storage and absent destinations. */
export type DocumentContainerOutcome =
  | {
      readonly status: 'editable';
      readonly container: PreparedDocumentContainer;
    }
  | {
      readonly status: 'protected';
      readonly container: PreparedDocumentContainer;
      readonly reason: 'header' | 'malformed';
    }
  | {
      readonly status: 'rejected';
      readonly reason: 'stale' | 'foreign' | 'missing' | 'unmapped';
    };
