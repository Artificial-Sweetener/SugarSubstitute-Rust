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
/** Search boundaries capture host accessors once while the issuing operation remains authoritative. */
import type {
  SearchContext,
  SearchNavigation,
  SearchResults,
  SearchRevealAdapter,
} from './contracts.js';

/** Supersession during synchronous host access prevents further capture or publication. */
export type SearchCapture<Value> =
  | { readonly status: 'captured'; readonly value: Value }
  | { readonly status: 'stale' };

const stale = Object.freeze({ status: 'stale' as const });
/** Weak issuance membership reuses validated immutable output without retaining raw host inputs. */
const issuedResults = new WeakSet<SearchResults>();

/** Capture query identities before paint retirement; raw host accessors are never retained. */
export function captureSearchContext(
  context: SearchContext | null,
  current: () => boolean,
): SearchCapture<SearchContext | null> {
  if (!current()) return stale;
  if (context === null) return { status: 'captured', value: null };
  if (typeof context !== 'object' || Array.isArray(context))
    throw new TypeError('Invalid search context');
  const fieldId = context.fieldId;
  if (!current()) return stale;
  const queryId = context.queryId;
  if (!current()) return stale;
  if (typeof fieldId !== 'string' || !fieldId || typeof queryId !== 'string' || !queryId)
    throw new TypeError('Invalid search context');
  return {
    status: 'captured',
    value: Object.freeze({ fieldId, queryId }),
  };
}

/** Capture the callable and its receiver together; disposal during a getter prevents invocation. */
export function captureSearchRevealAdapter(
  adapter: SearchRevealAdapter | null,
  current: () => boolean,
): SearchCapture<SearchRevealAdapter | null> {
  if (!current()) return stale;
  if (adapter === null) return { status: 'captured', value: null };
  if (!adapter || (typeof adapter !== 'object' && typeof adapter !== 'function'))
    throw new TypeError('Invalid search reveal adapter');
  const reveal = adapter.reveal;
  if (!current()) return stale;
  if (typeof reveal !== 'function') throw new TypeError('Invalid search reveal adapter');
  const captured: SearchRevealAdapter = {
    /** Preserve the host method's actual receiver without rereading its property. */
    reveal: (intent, signal) => Reflect.apply(reveal, adapter, [intent, signal]),
  };
  return {
    status: 'captured',
    value: Object.freeze(captured),
  };
}

/** Capture one navigation intent before reveal so later selection/focus consumes stable validated values. */
export function captureSearchNavigation(
  intent: SearchNavigation,
  current: () => boolean,
): SearchCapture<SearchNavigation> {
  if (!current()) return stale;
  if (!intent || typeof intent !== 'object' || Array.isArray(intent))
    throw new TypeError('Invalid search navigation intent');
  const request = intent.request;
  if (!current()) return stale;
  const resultRevision = intent.resultRevision;
  if (!current()) return stale;
  const matchId = intent.matchId;
  if (!current()) return stale;
  const select = intent.select;
  if (!current()) return stale;
  const scroll = intent.scroll;
  if (!current()) return stale;
  const focus = intent.focus;
  if (!current()) return stale;
  if ([select, scroll, focus].some((value) => typeof value !== 'boolean'))
    throw new TypeError('Invalid search navigation intent');
  if (typeof matchId !== 'string' || !matchId) throw new TypeError('Invalid search match identity');
  if (!Number.isSafeInteger(resultRevision) || resultRevision < 0)
    throw new RangeError('Invalid search result revision');
  return {
    status: 'captured',
    value: Object.freeze({
      request,
      resultRevision,
      matchId,
      select,
      scroll,
      focus,
    }),
  };
}

/** Capture immutable host match metadata; exact-source coordinate conversion remains the native core adapter's responsibility. */
export function captureSearchResults(
  results: SearchResults,
  current: () => boolean,
): SearchCapture<SearchResults> {
  if (!current()) return stale;
  if (!results || typeof results !== 'object' || Array.isArray(results))
    throw new TypeError('Invalid search results');
  if (issuedResults.has(results)) return { status: 'captured', value: results };
  const request = results.request;
  if (!current()) return stale;
  const resultRevision = results.resultRevision;
  if (!current()) return stale;
  const activeIndex = results.activeIndex;
  if (!current()) return stale;
  const sourceMatches = results.matches;
  if (!current()) return stale;
  if (!Array.isArray(sourceMatches)) throw new TypeError('Invalid search matches');
  const count = sourceMatches.length;
  if (!current()) return stale;
  if (!Number.isSafeInteger(count) || count < 0) throw new RangeError('Invalid search match count');
  if (!Number.isSafeInteger(resultRevision) || resultRevision < 0)
    throw new RangeError('Invalid search result revision');
  if (
    activeIndex !== null &&
    (!Number.isSafeInteger(activeIndex) || activeIndex < 0 || activeIndex >= count)
  )
    throw new RangeError('Invalid active search index');
  const ids = new Set<string>(),
    matches: SearchResults['matches'][number][] = [];
  for (let index = 0; index < count; index++) {
    const match = sourceMatches[index];
    if (!current()) return stale;
    if (!match || typeof match !== 'object' || Array.isArray(match))
      throw new TypeError('Invalid search match');
    const matchId = match.matchId;
    if (!current()) return stale;
    const from = match.from;
    if (!current()) return stale;
    const to = match.to;
    if (!current()) return stale;
    if (typeof matchId !== 'string' || !matchId || ids.has(matchId))
      throw new TypeError('Invalid search match identity');
    if (!Number.isSafeInteger(from) || !Number.isSafeInteger(to) || from < 0 || from >= to)
      throw new RangeError('Empty or reversed search range');
    ids.add(matchId);
    matches.push(Object.freeze({ matchId, from, to }));
  }
  const value = Object.freeze({
    request,
    resultRevision,
    activeIndex,
    matches: Object.freeze(matches),
  });
  issuedResults.add(value);
  return { status: 'captured', value };
}
