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
/** Pure source-weight mutations consume a parsed document; editor state owns target freshness and application. */
import type { SourceRange } from '../offsets.js';
import { utf16ToCodePoint } from '../offsets.js';
import { validateSourceRange } from '../normalization/ranges.js';
import { formatWeight, stepWeight } from '../normalization/decimals.js';
import type { PromptDocument } from './contracts.js';
import { parseWeight } from './weights.js';

/** An explicit source action uses a fixed keyboard/pointer step or an exact authored decimal. */
export type WeightChange =
  | { readonly kind: 'step'; readonly direction: 'increase' | 'decrease' }
  | { readonly kind: 'set'; readonly weight: string };

/** The proposed exact source and resulting content selection use exact-source UTF-16, including CRLF units. */
export interface WeightMutation {
  readonly text: string;
  readonly selectionRange: SourceRange;
}

/** Core identifies an explicit neutral target independently of enclosing syntax after a shell unwraps. */
export interface EmphasisWeightMutation extends WeightMutation {
  readonly neutral: boolean;
}

/** Attach the authoritative neutral outcome to an immutable source mutation. */
function emphasisResult(mutation: WeightMutation, neutral: boolean): EmphasisWeightMutation {
  return Object.freeze({ ...mutation, neutral });
}

/** Wrap a deliberate plain-content target, including a transient neutral layer inside another shell. */
export function wrapPlainEmphasisWeight(
  source: string,
  selection: SourceRange,
  change: WeightChange,
): EmphasisWeightMutation {
  validateRange(source, selection);
  const weight = changedWeight('1.00', change);
  if (selection.from === selection.to || weight === '1.00')
    return emphasisResult(
      result(source, selection),
      weight === '1.00' && selection.from !== selection.to,
    );
  return emphasisResult(
    replace(source, selection, `(${source.slice(selection.from, selection.to)}:${weight})`, {
      from: selection.from + 1,
      to: selection.to + 1,
    }),
    false,
  );
}

/** Validate exact source endpoints, rejecting scalar splits and half a CRLF before mutation. */
function validateRange(source: string, range: SourceRange): void {
  validateSourceRange(source, range);
  utf16ToCodePoint(source, range.from);
  utf16ToCodePoint(source, range.to);
}

/** Resolve the requested numeric value without floating-point arithmetic or implicit syntax repair. */
function changedWeight(current: string, change: WeightChange): string {
  if (change.kind === 'step') return stepWeight(current, change.direction);
  const value = parseWeight(change.weight, true);
  if (value === null) throw new TypeError('Invalid exact weight');
  return formatWeight(value);
}

/** Freeze derived mutation data without retaining a second parsed document or editable buffer. */
function result(text: string, selectionRange: SourceRange): WeightMutation {
  return Object.freeze({
    text,
    selectionRange: Object.freeze({ ...selectionRange }),
  });
}

/** Replace one semantic source interval and retain all surrounding bytes. */
function replace(
  source: string,
  range: SourceRange,
  text: string,
  selection: SourceRange,
): WeightMutation {
  return result(source.slice(0, range.from) + text + source.slice(range.to), selection);
}

/** Adjust an exact content/outer range, or wrap a nonempty plain selection; a neutral existing shell unwraps. */
export function mutateEmphasisWeight(
  document: PromptDocument,
  selection: SourceRange,
  change: WeightChange,
): EmphasisWeightMutation {
  const source = document.sourceText;
  validateRange(source, selection);
  const outer = document.emphasisSpans.find(
    (candidate) =>
      candidate.outerRange.from === selection.from && candidate.outerRange.to === selection.to,
  );
  const content = outer?.contentRange ?? selection;
  const span = document.emphasisSpans.find(
    (candidate) =>
      candidate.contentRange.from === content.from && candidate.contentRange.to === content.to,
  );
  if (span) {
    const weight = changedWeight(span.weight, change);
    if (weight === '1.00') {
      const content = source.slice(span.contentRange.from, span.contentRange.to);
      return emphasisResult(
        replace(source, span.outerRange, content, {
          from: span.outerRange.from,
          to: span.outerRange.from + content.length,
        }),
        true,
      );
    }
    return emphasisResult(replace(source, span.weightRange, weight, span.contentRange), false);
  }
  return wrapPlainEmphasisWeight(source, selection, change);
}

/** Change only the first numeric field of an exact LoRA outer range; absent/moved syntax is an explicit missing target. */
export function mutateLoraWeight(
  document: PromptDocument,
  outer: SourceRange,
  change: WeightChange,
): WeightMutation | null {
  const source = document.sourceText;
  validateRange(source, outer);
  const span = document.loraSpans.find(
    (candidate) => candidate.outerRange.from === outer.from && candidate.outerRange.to === outer.to,
  );
  if (!span) return null;
  const weight = changedWeight(span.firstWeight, change);
  return replace(source, span.firstWeightRange, weight, {
    from: span.firstWeightRange.from,
    to: span.firstWeightRange.from + weight.length,
  });
}
