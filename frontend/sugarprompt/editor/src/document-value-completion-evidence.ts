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
/** Immutable completion result evidence rejects changed DTO observations without owning source, native transitions or selection. */
import { requireValue } from '../../core/src/required.js';
import type {
  PreparedCompletionResult,
  PreparedCompletionEdit,
  PreparedCompletionSplice,
} from './prompt-completion-model.js';
import type { PreparedDocumentValueEdit } from './document-value-edits.js';
/** Captured result observations include only genuine recipe target membership supplied by the continuation owner. */
export interface CompletionEvidence {
  readonly result: PreparedCompletionResult | null;
  readonly candidates: readonly Candidate[];
  readonly queryBounds: readonly number[] | null;
  readonly kind: PreparedCompletionResult['query']['kind'] | null;
  readonly query: PreparedCompletionResult['query'] | null;
  readonly recipes: readonly {
    readonly edit: PreparedDocumentValueEdit;
  }[];
}
/** Exact activation observations retain the original variant membership. */
export interface Candidate {
  readonly option: PreparedCompletionResult['options'][number];
  readonly keyboard: PreparedCompletionEdit;
  readonly pointer: PreparedCompletionEdit;
  readonly kind: Candidate['option']['kind'];
  readonly caret: number;
  readonly freshness: Candidate['option']['freshness'];
  readonly edits: readonly {
    readonly acceptance: PreparedCompletionEdit;
    readonly edit: PreparedDocumentValueEdit;
    readonly splice: PreparedCompletionSplice | null;
    readonly right: PreparedCompletionEdit['right'];
    readonly rightSplice: PreparedCompletionSplice | null;
    readonly rightMatch: NonNullable<PreparedCompletionEdit['right']>['match'] | null;
    readonly maximum: number | null;
    readonly consumes: ((prefix: string) => boolean) | null;
  }[];
}
const mutatedResults = new WeakSet<PreparedCompletionResult>();
/** Detect mutation rather than trusting the installed DTO as a new authority. */
export function completionResultMatches(receipt: CompletionEvidence): boolean {
  const result = receipt.result;
  if (!result) return true;
  const matches =
    !mutatedResults.has(result) &&
    result.query === receipt.query &&
    result.from === requireValue(receipt.queryBounds, 'document-value-completion-evidence')[3] &&
    result.to === requireValue(receipt.queryBounds, 'document-value-completion-evidence')[4] &&
    result.query.scope ===
      requireValue(receipt.recipes[0], 'document-value-completion-evidence').edit.target.scope &&
    result.query.from ===
      requireValue(receipt.queryBounds, 'document-value-completion-evidence')[0] &&
    result.query.contentFrom ===
      requireValue(receipt.queryBounds, 'document-value-completion-evidence')[1] &&
    result.query.to ===
      requireValue(receipt.queryBounds, 'document-value-completion-evidence')[2] &&
    result.query.kind === receipt.kind &&
    result.options.length === receipt.candidates.length &&
    receipt.candidates.every(
      (candidate, index) =>
        Object.hasOwn(result.options, index) &&
        result.options[index] === candidate.option &&
        candidate.option.kind === candidate.kind &&
        candidate.option.caret === candidate.caret &&
        candidate.option.freshness === candidate.freshness &&
        candidate.option.acceptance.keyboard === candidate.keyboard &&
        candidate.option.acceptance.pointer === candidate.pointer &&
        candidate.edits.every(
          (expected) =>
            expected.acceptance.edit === expected.edit &&
            expected.acceptance.splice === expected.splice &&
            expected.acceptance.right === expected.right &&
            (expected.right === null ||
              (expected.right.splice === expected.rightSplice &&
                expected.right.match === expected.rightMatch &&
                expected.right.match.maximumSourcePrefixLength === expected.maximum &&
                expected.right.match.consumesSourcePrefix === expected.consumes)),
        ),
    );
  if (!matches) mutatedResults.add(result);
  return matches;
}
