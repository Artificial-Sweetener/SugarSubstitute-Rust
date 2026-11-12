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
/** Hosts supply decoded catalog snapshots; services own prepared lookup and exact tag knowledge. */
import type { TagLexicon, TagQuery } from '../../../core/src/index.js';
import type { LoraTriggerSuggestion } from '../lora/scheduled-contracts.js';

/** Bundled dataset metadata, ranked by popularity then scalar length and Unicode-folded spelling. */
export interface BundledTag {
  readonly tag: string;
  readonly popularity: number | null;
}

/** One host-prepared revision; custom tags preserve enabled-list order and censorship applies to both sources. */
export interface TagCatalogInput {
  readonly revision: string;
  readonly bundled: readonly BundledTag[];
  readonly customTags: readonly string[];
  readonly censoredTags: readonly string[];
}

/** Immutable literal row; source label distinguishes custom additions from bundled popularity metadata. */
export interface TagSuggestion extends BundledTag {
  readonly sourceLabel: 'custom' | null;
}

/** Prepared synchronous lookup; construction owns validation/indexing, and each revision has a bounded result memo. */
export interface TagCompletionCatalog {
  readonly revision: string;
  readonly lexicon: TagLexicon;
  /** Return up to 50 ranked prefix matches; spaces/underscores and pinned Unicode folding share exact-tag identity. */
  search(prefix: string, limit?: number): readonly TagSuggestion[];
}

/** Current concrete query after full-miss fallback; hadCandidates prevents no-op-only primary results from falling back. */
export interface TagCompletionResult {
  readonly query: TagQuery;
  readonly matches: readonly TagSuggestion[];
  readonly hadCandidates: boolean;
}

/** Trigger rows retain their supplying model; ordinary tags retain bundled/custom metadata. */
export type PromptTextSuggestion =
  | { readonly kind: 'tag'; readonly value: TagSuggestion }
  | { readonly kind: 'trigger'; readonly value: LoraTriggerSuggestion };

/** One concrete tag query merges configured model triggers before catalog tags and shares no-op/fallback rules. */
export interface PromptTextCompletionResult {
  readonly query: TagQuery;
  readonly matches: readonly PromptTextSuggestion[];
  readonly hadCandidates: boolean;
}
