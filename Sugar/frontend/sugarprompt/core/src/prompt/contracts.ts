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
/** Immutable semantic snapshots; every range uses exact-source UTF-16 code units. */
import type { SourceRange } from '../offsets.js';

/** A comma-delimited segment and its exact owned separator, including hard-line whitespace. */
export interface PromptSegment {
  readonly index: number;
  readonly text: string;
  readonly contentRange: SourceRange;
  readonly separatorRange: SourceRange | null;
  /** Direct eligible hard endings from the sole grammar scan; protected quoted/literal/nested content is excluded. */
  readonly hardBreaks: readonly SourceRange[];
}

/** Weighted emphasis with a lossless decimal value and same-kind nesting depth. */
export interface EmphasisSpan {
  readonly outerRange: SourceRange;
  readonly contentRange: SourceRange;
  readonly weightRange: SourceRange;
  readonly weight: string;
  readonly depth: number;
  /** Direct comma segments in this valid content, emitted by the same scanner in exact-source UTF-16. */
  readonly segments: readonly PromptSegment[];
}

/** Parsed wildcard identity; catalog existence is resolved separately from syntax. */
export interface WildcardSpan {
  readonly outerRange: SourceRange;
  readonly contentRange: SourceRange;
  readonly form: 'simple' | 'csv';
  readonly identifier: string;
  readonly csvColumn: string | null;
  readonly tag: string | null;
  readonly depth: number;
}

/** LoRA scheduling syntax, preserving both decimal fields and optional block-weight source. */
export interface LoraSpan {
  readonly outerRange: SourceRange;
  readonly nameRange: SourceRange;
  readonly firstWeightRange: SourceRange;
  readonly firstWeight: string;
  readonly secondWeightRange: SourceRange | null;
  readonly secondWeight: string | null;
  readonly blockWeightsRange: SourceRange | null;
  readonly depth: number;
}

/** Renderer-independent syntax index, sorted by exact-source range. */
export interface SyntaxSpan {
  readonly kind: 'emphasis' | 'wildcard' | 'lora';
  readonly sourceRange: SourceRange;
  readonly depth: number;
}

/** Complete regional syntax independent of its line placement; all ranges use exact-source UTF-16. */
export interface RegionToken {
  readonly tokenRange: SourceRange;
  readonly nameRange: SourceRange | null;
  readonly name: string | null;
}

/** A whole-line regional separator; its line range includes the original line ending. */
export interface RegionSeparator extends RegionToken {
  readonly lineRange: SourceRange;
}

/** Global or regional source between separator lines, including empty partitions. */
export interface RegionPartition {
  readonly index: number;
  readonly sourceRange: SourceRange;
  readonly isGlobal: boolean;
}

/** Lossless regional structure with one more partition than separators. */
export interface RegionStructure {
  readonly separators: readonly RegionSeparator[];
  readonly partitions: readonly RegionPartition[];
}

/** Parsed source snapshot; serialization returns sourceText without normalization. */
export interface PromptDocument {
  readonly sourceText: string;
  readonly segments: readonly PromptSegment[];
  readonly emphasisSpans: readonly EmphasisSpan[];
  readonly wildcardSpans: readonly WildcardSpan[];
  readonly loraSpans: readonly LoraSpan[];
  readonly syntaxSpans: readonly SyntaxSpan[];
  readonly regions: RegionStructure;
  readonly hasTrailingComma: boolean;
}

/** Scene heading with trimmed title, Unicode-folded identity, and duplicate state. */
export interface SceneMarker {
  readonly title: string;
  readonly normalizedKey: string;
  readonly markerRange: SourceRange;
  readonly titleRange: SourceRange;
  readonly lineRange: SourceRange;
  readonly duplicate: boolean;
}

/** Scene-local source after its heading and before the next heading. */
export interface SceneBlock {
  readonly marker: SceneMarker;
  readonly contentRange: SourceRange;
  readonly text: string;
}

/** Universal source precedes the first valid scene heading. */
export interface SceneDocument {
  readonly sourceText: string;
  readonly universalRange: SourceRange;
  readonly universalText: string;
  readonly scenes: readonly SceneBlock[];
}

/** Supported wildcard wrappers; custom delimiters are validated before parsing. */
export type WildcardSyntax =
  | { readonly style: 'curly' }
  | { readonly style: 'double-underscore' }
  | {
      readonly style: 'custom';
      readonly prefix: string;
      readonly suffix: string;
      readonly alsoRecognizeCurly: boolean;
    };

/** Combined independent syntax structures derived from one exact source snapshot. */
export interface PromptStructure {
  readonly document: PromptDocument;
  readonly scenes: SceneDocument;
}
