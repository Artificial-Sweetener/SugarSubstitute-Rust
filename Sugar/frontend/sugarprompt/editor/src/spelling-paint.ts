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
/** Derived spelling paint maps through native changes without granting service finding authority. */
import { requireValue } from '../../core/src/required.js';
import {
  RangeValue,
  RangeSet,
  type EditorState,
  type Transaction,
  type Text,
} from '@codemirror/state';
import { Decoration, type DecorationSet } from '@codemirror/view';
import {
  diagnosticIsActive,
  isDiagnosticCommitBoundary,
} from '../../core/src/diagnostics/visibility.js';
import { decodedRangeToSource } from '../../core/src/document/value-mapping.js';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import type { PreparedSpellingCandidates } from '../../services/src/diagnostics/spelling-contracts.js';
import { lineEndings } from './exact-source.js';
/** Visual word metadata has no command admission or dictionary receipt of its own. */
export class SpellingPaint extends RangeValue {
  override readonly startSide = 1;
  override readonly endSide = -1;
  readonly text: string;
  readonly committedAtEnd: boolean;
  /** Actual candidate text is retained only for exact off-input canonical reconciliation. */
  constructor(text: string, committedAtEnd: boolean) {
    super();
    this.text = text;
    this.committedAtEnd = committedAtEnd;
  }
}
/** Native projected ranges and one active range share the existing diagnostic field lifetime. */
export interface SpellingPaintState {
  readonly paint: RangeSet<SpellingPaint>;
  readonly nativeBoundaries: boolean;
  readonly decorations: DecorationSet;
  readonly muted: SpellingPaint | null;
  readonly activeRange: {
    readonly from: number;
    readonly to: number;
  } | null;
}
/** Construct identical literal marks for genuine and provisional visual output. */
export function spellingMark(value: SpellingPaint, active = false): Decoration {
  return Decoration.mark({
    class: 'sp-spelling-diagnostic',
    attributes: { 'data-diagnostic-active': String(active), title: 'Spelling' },
    spellingPaint: value,
  });
}
/** Read one adjacent scalar; conservative carry also refuses apostrophes that can extend the unchanged word. */
function boundary(
  document: Text,
  position: number,
  before = false,
  unchangedWord = false,
): boolean {
  if (position === (before ? 0 : document.length)) return true;
  const text = before
    ? requireValue(
        Array.from(document.sliceString(Math.max(0, position - 2), position)).at(-1),
        'spelling-paint',
      )
    : String.fromCodePoint(
        requireValue(
          document.sliceString(position, Math.min(document.length, position + 2)).codePointAt(0),
          'spelling-paint',
        ),
      );
  return isDiagnosticCommitBoundary(text) && (!unchangedWord || text !== "'");
}
/** Touch only marks beside actual changed intervals, then delegate offset mapping to CodeMirror. */
export function mapSpellingPaint(
  value: SpellingPaintState,
  transaction: Transaction,
): SpellingPaintState {
  let paint = value.paint,
    decorations = value.decorations;
  if (value.muted && value.activeRange)
    decorations = decorations.update({
      filterFrom: value.activeRange.from,
      filterTo: value.activeRange.to,
      filter: (_from, _to, mark) => mark.spec.spellingPaint !== value.muted,
      add: [spellingMark(value.muted).range(value.activeRange.from, value.activeRange.to)],
    });
  transaction.changes.iterChangedRanges((from, to) => {
    const keep = (start: number, end: number): boolean => {
      if (start > to || end < from) return true;
      if ((from < end && to > start) || (from === to && start < from && from < end)) return false;
      const mappedFrom = transaction.changes.mapPos(start, 1),
        mappedTo = transaction.changes.mapPos(end, -1);
      return (
        mappedFrom < mappedTo &&
        boundary(transaction.newDoc, mappedFrom, true, true) &&
        boundary(transaction.newDoc, mappedTo, false, true)
      );
    };
    paint = paint.update({ filterFrom: from, filterTo: to, filter: keep });
    decorations = decorations.update({
      filterFrom: from,
      filterTo: to,
      filter: keep,
    });
  });
  return {
    paint: paint.map(transaction.changes),
    nativeBoundaries: true,
    decorations: decorations.map(transaction.changes),
    muted: null,
    activeRange: null,
  };
}
/** Active visibility uses current native range coordinates, including carried ranges moved by linebreaks. */
export function spellingPaintVisibility<T extends SpellingPaintState>(
  value: T,
  state: EditorState,
): T {
  const active: {
    muted: SpellingPaint | null;
    range: SpellingPaintState['activeRange'];
  } = { muted: null, range: null };
  value.paint.between(state.selection.main.head, state.selection.main.head, (from, to, item) => {
    if (
      diagnosticIsActive(
        from,
        to,
        state.selection.main.head,
        value.nativeBoundaries
          ? to < state.doc.length && boundary(state.doc, to)
          : item.committedAtEnd,
      )
    ) {
      active.muted = item;
      active.range = { from, to };
    }
  });
  const { muted, range: activeRange } = active;
  if (
    muted === value.muted &&
    activeRange?.from === value.activeRange?.from &&
    activeRange?.to === value.activeRange?.to
  )
    return value;
  let decorations = value.decorations;
  for (const [item, range] of [
    [value.muted, value.activeRange],
    [muted, activeRange],
  ] as const) {
    if (item && range)
      decorations = decorations.update({
        filterFrom: range.from,
        filterTo: range.to,
        filter: (_from, _to, mark) => mark.spec.spellingPaint !== item,
        add: [spellingMark(item, item === muted).range(range.from, range.to)],
      });
  }
  return Object.freeze({ ...value, muted, activeRange, decorations });
}
/** Intersect carried paint with actual new canonical candidates, outside the native input transaction. */
export function reconcileSpellingPaint(
  value: SpellingPaintState,
  state: EditorState,
  prepared: PreparedSpellingCandidates,
): SpellingPaintState {
  const candidates = prepared.groups.flatMap((group) =>
    group.candidates.map((candidate) => {
      const range = decodedRangeToSource(group.scope.mapping, candidate.range);
      if (!range) throw new Error('spelling-range-unmapped');
      const next = group.scope.mapping.text.codePointAt(candidate.range.to);
      return {
        range,
        text: candidate.text,
        committedAtEnd: isDiagnosticCommitBoundary(
          next === undefined ? '' : String.fromCodePoint(next),
        ),
      };
    }),
  );
  const positions = sourcePositionsToDocument(
    state.doc.toString(),
    state.field(lineEndings),
    candidates.flatMap(({ range }) => [range.from, range.to]),
  );
  const eligible = new Map(
    candidates.map((candidate, index) => [
      `${positions[index * 2]}:${positions[index * 2 + 1]}`,
      candidate,
    ]),
  );
  const retained: Array<{
    from: number;
    to: number;
    value: SpellingPaint;
  }> = [];
  value.paint.between(0, state.doc.length, (from, to, item) => {
    const candidate = eligible.get(`${from}:${to}`);
    if (candidate?.text !== item.text) return;
    retained.push({
      from,
      to,
      value:
        candidate.committedAtEnd === item.committedAtEnd
          ? item
          : Object.freeze(new SpellingPaint(item.text, candidate.committedAtEnd)),
    });
  });
  return {
    paint: RangeSet.of(
      retained.map(({ from, to, value }) => value.range(from, to)),
      true,
    ),
    nativeBoundaries: false,
    decorations: Decoration.set(
      retained.map(({ from, to, value }) => spellingMark(value).range(from, to)),
      true,
    ),
    muted: null,
    activeRange: null,
  };
}
