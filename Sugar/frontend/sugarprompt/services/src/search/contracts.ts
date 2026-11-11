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
/** Host-owned query and field identities contain no query text. */
export interface SearchContext {
  readonly fieldId: string;
  readonly queryId: string;
}

/** Opaque issued identity binds a host recomputation to one source revision and query lifetime. */
export interface SearchRequest extends SearchContext {
  readonly sourceRevision: number;
  readonly identity: symbol;
}

/** Authored match boundaries are exact-source UTF-16, including original newline spellings. */
export interface SearchMatch {
  readonly matchId: string;
  readonly from: number;
  readonly to: number;
}

/** Monotonic result revision rejects late publications within the same source/query request. */
export interface SearchResults {
  readonly request: SearchRequest;
  readonly resultRevision: number;
  readonly matches: readonly SearchMatch[];
  readonly activeIndex: number | null;
}

/** Host visibility belongs to the shell; the editor publishes source-free filter and reveal intent. */
export type SearchHostIntent =
  | { readonly kind: 'filter'; readonly request: SearchRequest }
  | { readonly kind: 'clear'; readonly fieldId: string }
  | {
      readonly kind: 'reveal';
      readonly request: SearchRequest;
      readonly matchId: string;
    };

/** Host reveal completes after the target field has usable layout; cancellation must release host work. */
export interface SearchRevealAdapter {
  reveal(
    intent: Extract<SearchHostIntent, { kind: 'reveal' }>,
    signal: AbortSignal,
  ): Promise<'ready' | 'unavailable'>;
}

/** Explicit native selection, viewport and focus intent never changes editable source. */
export interface SearchNavigation {
  readonly request: SearchRequest;
  readonly resultRevision: number;
  readonly matchId: string;
  readonly select: boolean;
  readonly scroll: boolean;
  readonly focus: boolean;
}

/** Expected supersession and unavailable focus are data; unexpected host failures remain observable. */
export interface SearchOutcome {
  readonly status: 'applied' | 'stale' | 'cancelled' | 'unavailable';
}

/** Search observation exposes only identities, counts and the active authored match. */
export interface SearchSnapshot {
  readonly request: SearchRequest | null;
  readonly resultRevision: number | null;
  readonly matchCount: number;
  readonly activeMatchId: string | null;
}
