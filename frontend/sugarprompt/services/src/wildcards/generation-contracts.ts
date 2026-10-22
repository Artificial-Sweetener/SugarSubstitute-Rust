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
/** Closed generation contracts; all source ranges use exact-source UTF-16 in their recorded iteration. */
import type { SourceRange, WildcardSyntax } from '../../../core/src/index.js';

/** Host-decoded candidates preserve the provider's selection order and stable provenance identity. */
export type WildcardCandidateSource =
  | {
      readonly status: 'ready';
      readonly form: 'simple';
      readonly identifier: string;
      readonly sourceId: string;
      readonly lines: readonly string[];
    }
  | {
      readonly status: 'ready';
      readonly form: 'csv';
      readonly identifier: string;
      readonly sourceId: string;
      readonly rows: readonly Readonly<Record<string, string>>[];
    }
  | {
      readonly status: 'unavailable';
      readonly form: 'simple' | 'csv';
      readonly identifier: string;
    };

/** One immutable candidate revision and one finite generation context; seed strings are canonical signed decimal integers. */
export interface WildcardGenerationInput {
  readonly revision: string;
  readonly sources: readonly WildcardCandidateSource[];
  readonly seed: string | null;
  /** Required only for an unseeded pass; hosts inject entropy once, while provenance retains a null effective seed. */
  readonly entropySeed?: string;
  readonly syntax?: WildcardSyntax;
  /** Generation resolution defaults to enabled. */
  readonly enabled?: boolean;
}

/** Each unique token records the complete retained donor provenance plus its actually replaced ranges. */
export interface WildcardReplacementDetail {
  readonly outerText: string;
  readonly value: string;
  readonly form: 'simple' | 'csv';
  readonly identifier: string;
  readonly sourceId: string;
  readonly selectedIndex: number;
  readonly lineNumber: number;
  readonly itemCount: number;
  readonly tag: string | null;
  readonly csvColumn: string | null;
  readonly seed: string | null;
  readonly iteration: number;
  /** Ranges refer to the current iteration's input, before simultaneous source replacements. */
  readonly ranges: readonly SourceRange[];
}

/** Expected generation failures preserve unresolved token text and never become empty success. */
export interface WildcardGenerationIssue {
  readonly reason:
    | 'source-missing'
    | 'source-unavailable'
    | 'empty-source'
    | 'column-missing'
    | 'invalid-tag'
    | 'iteration-limit'
    | 'output-limit'
    | 'context-limit';
  /** Input iteration for this range; terminal iteration 25 is the unresolved resolvedText after all 25 replacements. */
  readonly iteration: number;
  readonly range: SourceRange;
  readonly form: 'simple' | 'csv';
  readonly identifier: string;
  readonly tag: string | null;
  readonly csvColumn: string | null;
}

/** Host submission carries exact authored source separately from prepared generation output and provenance. */
export interface WildcardGenerationResolution {
  readonly status: 'resolved' | 'partial' | 'disabled';
  readonly revision: string;
  readonly sourceText: string;
  readonly resolvedText: string;
  readonly replacements: readonly (readonly [string, string])[];
  readonly replacementDetails: readonly WildcardReplacementDetail[];
  readonly issues: readonly WildcardGenerationIssue[];
}

/** Finite request admission failures contain stable codes and no source payload. */
export type WildcardGenerationOutcome =
  | WildcardGenerationResolution
  | {
      readonly status: 'rejected';
      readonly reason: 'disposed' | 'invalid-source' | 'invalid-seed' | 'request-limit';
    };

/** One pass owns seeded streams and coordinated choices across host prompt fields; hosts dispose it after submission. */
export interface WildcardGenerationPass {
  /** Resolve a complete prompt outside typing/paint; null or omitted seed falls back to the captured pass seed. */
  resolve(source: string, seed?: string | null): WildcardGenerationOutcome;
  /** Release all captured candidate values and pass-local choice/stream state; subsequent calls reject. */
  dispose(): void;
}

/** Unknown host data is validated before adaptation; malformed input remains a typed rejection. */
export type WildcardGenerationPreparation =
  | { readonly status: 'ready'; readonly pass: WildcardGenerationPass }
  | { readonly status: 'rejected'; readonly reason: 'invalid-input' };
