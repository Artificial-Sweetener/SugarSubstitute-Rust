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
/** Prepare actual decoded candidates once without provider calls or source reparsing. */
import { prepareSpellcheckCandidates } from '../../../core/src/diagnostics/spellcheck-candidates.js';
import type { PreparedDocumentPromptScopes } from '../../../core/src/document/prompt-scopes.js';
import type { TagLexicon } from '../../../core/src/normalization/contracts.js';
import type { PreparedSpellingCandidates } from './spelling-contracts.js';

const preparations = new WeakSet<PreparedSpellingCandidates>();

/** Populated reuse requires the same canonical scopes and actual exact-tag knowledge. */
export function prepareSpellingCandidates(
  scopes: PreparedDocumentPromptScopes,
  lexicon: TagLexicon | null = null,
  previous?: PreparedSpellingCandidates,
): PreparedSpellingCandidates {
  const reusable = previous && preparations.has(previous) ? previous : undefined;
  if (reusable?.scopes === scopes && reusable.lexicon === lexicon) return reusable;
  const groups = new Map(reusable?.groups.map((group) => [group.scope, group]));
  const prepared = Object.freeze({
    scopes,
    lexicon,
    groups: Object.freeze(
      scopes.scopes.map((scope) =>
        prepareSpellcheckCandidates(scope, lexicon ?? undefined, groups.get(scope)),
      ),
    ),
  });
  preparations.add(prepared);
  return prepared;
}

/** Service admission is genuine preparation membership, never a caller-created snapshot shape. */
export function isPreparedSpellingCandidates(prepared: PreparedSpellingCandidates): boolean {
  return preparations.has(prepared);
}
