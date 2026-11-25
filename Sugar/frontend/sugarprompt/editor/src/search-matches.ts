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
/** Native prepared ranges own which text is a match; no text/query parser or parallel editable state exists. */
import { requireValue } from '../../core/src/required.js';
import {
  RangeSet,
  RangeValue,
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
} from '@codemirror/state';
import { EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import type { SearchResults, SearchSnapshot } from '../../services/src/search/contracts.js';
import { captureSearchResults } from '../../services/src/search/capture.js';
import { SearchHighlightRanges } from '../../services/src/search/highlights.js';
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
/**
 * Contribute the visible matches to the document's shared search highlights as ranges over the
 * editor's own text nodes, so the browser paints them exactly on their glyphs through wrapping,
 * scrolling and bidi, in the look the host gives every match. The current match goes to the
 * current-match highlight, which the host draws as a selection.
 */
class MatchHighlights {
  readonly #ranges: SearchHighlightRanges;
  readonly #measure: { read(view: EditorView): MatchRanges; write(ranges: MatchRanges): void };

  constructor(view: EditorView) {
    this.#ranges = new SearchHighlightRanges(view.dom.ownerDocument.defaultView);
    this.#measure = {
      read: (measured) => matchRanges(measured),
      write: ({ matches, current }) => this.#ranges.replace(matches, current),
    };
    view.requestMeasure(this.#measure);
  }

  update(update: ViewUpdate): void {
    if (
      update.startState.field(searchMatches) !== update.state.field(searchMatches) ||
      update.docChanged ||
      update.geometryChanged ||
      update.viewportChanged
    )
      update.view.requestMeasure(this.#measure);
  }

  destroy(): void {
    this.#ranges.clear();
  }
}

interface MatchRanges {
  readonly matches: Range[];
  readonly current: Range[];
}

/** One DOM range per match in the viewport, sorted into current and other. */
function matchRanges(view: EditorView): MatchRanges {
  const found: MatchRanges = { matches: [], current: [] };
  const prepared = view.state.field(searchMatches);
  if (!prepared) return found;
  const atomic = view.state.field(richProjection, false)?.atomic;
  prepared.indexed.between(view.viewport.from, view.viewport.to, (from, to, value) => {
    let atom: { from: number; to: number } | null = null;
    atomic?.between(from, to, (start, end) => {
      if (start < to && end > from) atom = { from: start, to: end };
    });
    const range = atom
      ? atomRange(view, atom, view.state.sliceDoc(from, to))
      : textRange(view, from, to);
    if (range)
      (value.index === prepared.results.activeIndex ? found.current : found.matches).push(range);
  });
  return found;
}

/** The range of plain document text between two positions. */
function textRange(view: EditorView, from: number, to: number): Range | null {
  const start = view.domAtPos(from, 1);
  const end = view.domAtPos(to, -1);
  const range = view.dom.ownerDocument.createRange();
  try {
    range.setStart(start.node, start.offset);
    range.setEnd(end.node, end.offset);
  } catch {
    return null;
  }
  return range.collapsed ? null : range;
}

/**
 * The range for a match inside a replaced token. The token draws its own glyphs: where they read
 * the matched text, as a LoRA chip's name does, exactly those glyphs are the match; a token that
 * shows the text some other way is the match as a whole.
 */
function atomRange(
  view: EditorView,
  atom: { from: number; to: number },
  text: string,
): Range | null {
  const at = view.domAtPos(atom.from, 1);
  const widget = at.node.childNodes[at.offset];
  if (!widget) return null;
  const document = view.dom.ownerDocument;
  const range = document.createRange();
  const needle = text.toLowerCase();
  const walker = document.createTreeWalker(widget, 4);
  for (let node = walker.nextNode(); needle && node; node = walker.nextNode()) {
    const index = (node.nodeValue ?? '').toLowerCase().indexOf(needle);
    if (index < 0) continue;
    range.setStart(node, index);
    range.setEnd(node, index + needle.length);
    return range;
  }
  range.selectNodeContents(widget);
  return range.collapsed ? null : range;
}

/** The prepared matches and their contribution to the document's shared search highlights. */
export const searchMatchExtension: Extension = [
  searchMatches,
  ViewPlugin.fromClass(MatchHighlights),
];
