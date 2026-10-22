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
/** Native prepared ranges own local search paint; no text/query parser or parallel editable state exists. */
import { requireValue } from '../../core/src/required.js';
import {
  EditorSelection,
  RangeSet,
  RangeValue,
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
} from '@codemirror/state';
import { EditorView, RectangleMarker, layer, type LayerMarker } from '@codemirror/view';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import type { SearchResults, SearchSnapshot } from '../../services/src/search/contracts.js';
import { captureSearchResults } from '../../services/src/search/capture.js';
import { lineEndings } from './exact-source.js';
import { editorFreshness, freshnessRejection, type EditorFreshness } from './freshness.js';
import { richProjection } from './rich-projection.js';
/** Immutable logical-document UTF-16 ranges derive once from exact host boundaries. */
export interface PreparedSearch {
  readonly results: SearchResults;
  readonly freshness: EditorFreshness;
  readonly ranges: readonly {
    readonly from: number;
    readonly to: number;
  }[];
  readonly indexed: RangeSet<SearchRange>;
  readonly byId: ReadonlyMap<string, number>;
}
/** Indexed prepared match ordinal makes viewport paint proportional to visible results. */
class SearchRange extends RangeValue {
  readonly index: number;
  /** Capture the stable host ordering independently of native sorted range indexing. */
  constructor(index: number) {
    super();
    this.index = index;
  }
}
/** Host publication and explicit clear use supported source-neutral native transactions. */
export const installSearchMatches = StateEffect.define<PreparedSearch | null>();
/** State retirement precedes painting and host callbacks; obsolete ranges are never mapped to new source. */
export const searchMatches = StateField.define<PreparedSearch | null>({
  create: () => null,
  update(current, transaction) {
    const freshness = transaction.state.field(editorFreshness);
    let next = current && freshnessRejection(freshness, current.freshness) ? null : current;
    for (const effect of transaction.effects)
      if (effect.is(installSearchMatches)) {
        const value = effect.value;
        next = value && freshnessRejection(freshness, value.freshness) ? null : value;
      }
    return next;
  },
});
/** Validate all exact-source UTF-16 boundaries outside native rendering, rejecting CRLF interiors. */
export function prepareSearchMatches(state: EditorState, input: SearchResults): PreparedSearch {
  const captured = captureSearchResults(input, () => true);
  if (captured.status !== 'captured') throw new Error('Search capture unexpectedly retired');
  const results = captured.value;
  const logical = state.doc.toString(),
    endings = state.field(lineEndings),
    matches = results.matches;
  const positions = sourcePositionsToDocument(
    logical,
    endings,
    matches.flatMap((match) => [match.from, match.to]),
  );
  const ranges = matches.map((_match, index) =>
    Object.freeze({
      from: requireValue(positions[index * 2], 'search-matches'),
      to: requireValue(positions[index * 2 + 1], 'search-matches'),
    }),
  );
  return Object.freeze({
    results,
    freshness: state.field(editorFreshness),
    ranges: Object.freeze(ranges),
    indexed: RangeSet.of(
      ranges.map((range, index) => new SearchRange(index).range(range.from, range.to)),
      true,
    ),
    byId: new Map(matches.map((match, index) => [match.matchId, index])),
  });
}
/** Observe source-free public identity without exposing prepared logical positions or query text. */
export function searchSnapshot(
  state: EditorState,
  request: SearchSnapshot['request'],
): SearchSnapshot {
  const results = state.field(searchMatches)?.results;
  return Object.freeze({
    request,
    resultRevision: results?.resultRevision ?? null,
    matchCount: results?.matches.length ?? 0,
    activeMatchId:
      results?.activeIndex === null || results?.activeIndex === undefined
        ? null
        : requireValue(results.matches[results.activeIndex], 'search-matches').matchId,
  });
}
/** CodeMirror's measured selection rectangles own bidi, wrapping, replaced-token and viewport geometry. */
export const searchMatchExtension: Extension = [
  searchMatches,
  layer({
    above: true,
    class: 'sugarprompt-search-layer',
    update: (update) =>
      update.startState.field(searchMatches) !== update.state.field(searchMatches) ||
      update.geometryChanged ||
      update.viewportChanged,
    markers(view) {
      const prepared = view.state.field(searchMatches);
      if (!prepared) return [];
      const markers: LayerMarker[] = [];
      prepared.indexed.between(view.viewport.from, view.viewport.to, (from, to, value) => {
        // A substring inside a replacement has no independent visible glyphs; paint its actual native atomic widget.
        view.state.field(richProjection, false)?.atomic.between(from, to, (start, end) => {
          if (start < to && end > from) {
            from = Math.min(from, start);
            to = Math.max(to, end);
          }
        });
        markers.push(
          ...RectangleMarker.forRange(
            view,
            value.index === prepared.results.activeIndex
              ? 'sugarprompt-search-active'
              : 'sugarprompt-search-passive',
            EditorSelection.range(from, to),
          ),
        );
      });
      return markers;
    },
  }),
  EditorView.baseTheme({
    '.sugarprompt-search-layer': { pointerEvents: 'none' },
    '.sugarprompt-search-passive': {
      backgroundColor:
        'rgb(from var(--sugarprompt-search-highlight, Highlight) r g b / calc(90 / 255))',
      outline: 'none',
    },
    '.sugarprompt-search-active': {
      backgroundColor:
        'rgb(from var(--sugarprompt-search-highlight, Highlight) r g b / calc(150 / 255))',
      outline: 'none',
    },
    // Empty decorative markers retain the native system palette and absolute donor alpha in forced colors.
    '@media (forced-colors: active)': {
      '.sugarprompt-search-passive': {
        forcedColorAdjust: 'none',
        backgroundColor: 'rgb(from Highlight r g b / calc(90 / 255))',
      },
      '.sugarprompt-search-active': {
        forcedColorAdjust: 'none',
        backgroundColor: 'rgb(from Highlight r g b / calc(150 / 255))',
      },
    },
  }),
];
