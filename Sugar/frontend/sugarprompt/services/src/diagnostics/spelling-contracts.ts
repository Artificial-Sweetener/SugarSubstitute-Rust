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
/** Injected spelling transport owns dictionary access; services validate every external payload. */
import type { SourceRange } from '../../../core/src/offsets.js';
import type {
  PreparedSpellcheckCandidates,
  SpellcheckCandidate,
} from '../../../core/src/diagnostics/spellcheck-candidates.js';
import type { PreparedDocumentPromptScopes } from '../../../core/src/document/prompt-scopes.js';
import type { TagLexicon } from '../../../core/src/normalization/contracts.js';

/** Expected absence has static codes and never contains backend error or word text. */
export type SpellingUnavailableCode =
  | 'backend-unavailable'
  | 'dictionary-unavailable'
  | 'unsupported';

/** Adapter availability is captured once per operation. */
export type SpellingAvailability =
  | {
      readonly status: 'available';
      readonly dictionaryAdd: boolean;
      readonly sessionIgnore: boolean;
    }
  | { readonly status: 'unavailable'; readonly code: SpellingUnavailableCode };

/** Ready adapter data remains unknown until validated by the service. */
export type SpellingLookup =
  | { readonly status: 'ready'; readonly value: unknown }
  | { readonly status: 'unavailable'; readonly code: SpellingUnavailableCode }
  | { readonly status: 'cancelled' };

/** Host methods honor abort and release their own external listeners; unexpected failures reject unchanged. */
export interface SpellcheckGateway {
  readonly identity: symbol;
  readonly languageTag: string;
  availability(): SpellingAvailability;
  /** Ready payload is a dense boolean array with exactly one answer per ordered word. */
  checkWords(words: readonly string[], signal: AbortSignal): Promise<SpellingLookup>;
  /** Ready payload is at most eight literal nonempty strings of at most 4096 UTF16 units. */
  suggest(word: string, limit: 8, signal: AbortSignal): Promise<SpellingLookup>;
  /** Ready payload is true only when dictionary persistence accepted the word. */
  addToDictionary?(word: string, signal: AbortSignal): Promise<SpellingLookup>;
  /** Local suppression is authoritative even when optional backend delegation is unavailable. */
  ignoreForSession?(word: string, signal: AbortSignal): Promise<SpellingLookup>;
}

/** Canonical scopes and actual Core outputs are immutable preparation observations, not edit authority. */
export interface PreparedSpellingCandidates {
  readonly scopes: PreparedDocumentPromptScopes;
  readonly lexicon: TagLexicon | null;
  readonly groups: readonly PreparedSpellcheckCandidates[];
}

/** Exact storage UTF16 display range accompanies the actual decoded scope-local candidate. */
export interface SpellingIssue {
  readonly kind: 'spelling';
  readonly group: PreparedSpellcheckCandidates;
  readonly candidate: SpellcheckCandidate;
  readonly range: SourceRange;
}

/** Results retain the exact candidate preparation and dictionary-session revision. */
export interface PreparedSpellingSnapshot {
  readonly prepared: PreparedSpellingCandidates;
  readonly gateway: symbol;
  readonly languageTag: string;
  readonly revision: number;
  readonly dictionaryAdd: boolean;
  readonly issues: readonly SpellingIssue[];
}

/** Suggestions are literal transport results bound to an actual still-current service issue. */
export interface PreparedSpellingSuggestions {
  readonly snapshot: PreparedSpellingSnapshot;
  readonly issue: SpellingIssue;
  readonly suggestions: readonly string[];
}

/** Retirement and expected absence are distinct from successful empty findings. */
export type SpellingOutcome<T> =
  | { readonly status: 'ready'; readonly value: T }
  | { readonly status: 'unavailable'; readonly code: SpellingUnavailableCode }
  | { readonly status: 'cancelled' | 'disposed' | 'rejected' };

/** Accepted local suppression is independent of optional delegation success. */
export interface SpellingIgnoreResult {
  readonly revision: number;
  readonly delegation: 'accepted' | 'unsupported' | 'unavailable';
}
