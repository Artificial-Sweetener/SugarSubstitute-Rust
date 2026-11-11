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
/** Pure reorder plans consume one canonical decoded value; all ranges use its UTF-16 coordinates. */
import type { SourceRange } from '../offsets.js';
import type { PreparedDocumentPromptScope } from '../document/prompt-scopes.js';
import type { EmphasisSpan } from '../prompt/contracts.js';

/** Reasons for newly synthesized reorder syntax with no original source interval. */
export type ReorderDerivedReason = 'row-separator' | 'blank-line-comma' | 'empty-partition-ending';

/** Exact source fragments and deliberately synthesized separator bytes never share provenance. */
export type ReorderTextPiece =
  | { readonly kind: 'source'; readonly sourceRange: SourceRange }
  | {
      readonly kind: 'derived';
      readonly text: string;
      readonly reason: ReorderDerivedReason;
    };

/** A finite immutable carrier caches the exact concatenation of its own source or derived pieces. */
export interface ReorderText {
  readonly scope: PreparedDocumentPromptScope;
  readonly text: string;
  readonly pieces: readonly ReorderTextPiece[];
}

/** Atomic compound components retain provenance without exposing an inadmissible internal output boundary. */
export type ReorderOriginComponent =
  | { readonly kind: 'unchanged' | 'copied'; readonly sourceRange: SourceRange }
  | {
      readonly kind: 'derived';
      readonly text: string;
      readonly reason: ReorderDerivedReason;
    };

/** A source position can have several images or become an interior of a newly joined output unit. */
export type ReorderOriginBoundary =
  | { readonly status: 'rejected'; readonly reason: 'invalid-source-boundary' }
  | {
      readonly status: 'observed';
      readonly images: readonly (
        | { readonly kind: 'boundary'; readonly position: number }
        | { readonly kind: 'interior'; readonly unitRange: SourceRange }
      )[];
    };

/** A dense output fragment either retains exact original bytes or names newly synthesized syntax. */
export type ReorderOriginPiece =
  | {
      readonly kind: 'unchanged' | 'copied';
      readonly sourceRange: SourceRange;
      readonly outputRange: SourceRange;
    }
  | {
      readonly kind: 'derived';
      readonly outputRange: SourceRange;
      readonly reason: ReorderDerivedReason;
    }
  | {
      readonly kind: 'composed';
      readonly outputRange: SourceRange;
      readonly components: readonly ReorderOriginComponent[];
    };

/** Each emitted shell occurrence pairs one original parsed identity with its complete output bounds. */
export interface ReorderEnvelopeOccurrence {
  readonly span: EmphasisSpan;
  readonly outerRange: SourceRange;
  readonly contentRange: SourceRange;
}

/** One source-owned transparent shell; identity keeps distinct equal-weight shells distinct. */
export interface ReorderEnvelope {
  readonly span: EmphasisSpan;
  readonly opening: string;
  readonly closing: string;
}

/** A source-derived content item, including its exact surrounding horizontal padding. */
export interface ReorderChip {
  readonly index: number;
  readonly partition: number;
  readonly text: string;
  readonly contentRange: SourceRange;
  readonly separatorRange: SourceRange | null;
  readonly envelopes: readonly ReorderEnvelope[];
  readonly leading: ReorderText;
  readonly trailing: ReorderText;
}

/** Immutable order and separators; it is a transformation operand, never a mutable editing session. */
export interface ReorderState {
  readonly order: readonly number[];
  readonly partitions: readonly number[];
  readonly slots: readonly ReorderText[];
  readonly prefix: ReorderText;
  readonly suffix: ReorderText;
}

/** One prepared canonical value and its initial populated reorder output. */
export interface PreparedValueReorder {
  readonly scope: PreparedDocumentPromptScope;
  readonly chips: readonly ReorderChip[];
  readonly initial: ReorderState;
}

/** A populated source row, independent of browser layout and wrapping. */
export interface ReorderRow {
  readonly index: number;
  readonly partition: number;
  readonly start: number;
  readonly chips: readonly number[];
}

/** An exact separator destination; boundaries also represent empty regional partitions. */
export interface ReorderGap {
  readonly index: number;
  readonly partition: number;
  readonly slot: number;
  readonly carrier: 'prefix' | 'slot' | 'suffix';
  readonly text: string;
  readonly blankOffsets: readonly number[];
}

/** Source topology consumed by a native measured geometry owner. */
export interface ReorderLayout {
  readonly rows: readonly ReorderRow[];
  readonly gaps: readonly ReorderGap[];
}

/** Targets refer to the layout with the held chip removed, never to another canonical value. */
export type ReorderDropTarget =
  | { readonly kind: 'row'; readonly row: number; readonly insertion: number }
  | { readonly kind: 'gap'; readonly gap: number; readonly blank: number }
  | { readonly kind: 'partition'; readonly partition: number };

/** An empty parsed regional destination points into an exact owned structural carrier, including edge regions. */
export interface EmptyReorderPartition {
  readonly partition: number;
  readonly carrier: 'prefix' | 'slot' | 'suffix';
  readonly slot: number;
  readonly offset: number;
  readonly ending: string;
}

/** Serialized source plus content/rendered ranges indexed by original chip identity. */
export interface ReorderOutput {
  readonly text: string;
  readonly contentRanges: Readonly<Record<number, SourceRange>>;
  readonly renderedRanges: Readonly<Record<number, SourceRange>>;
  readonly slotRanges: readonly SourceRange[];
  readonly origin: {
    readonly scope: PreparedDocumentPromptScope;
    readonly pieces: readonly ReorderOriginPiece[];
    readonly envelopes: readonly ReorderEnvelopeOccurrence[];
  };
}

/** A pure capture of directional endpoints relative to one active chip, with no native selection ownership. */
export interface ReorderSelection {
  readonly chip: number;
  readonly anchorOffset: number | null;
  readonly headOffset: number | null;
  readonly reversed: boolean;
}
