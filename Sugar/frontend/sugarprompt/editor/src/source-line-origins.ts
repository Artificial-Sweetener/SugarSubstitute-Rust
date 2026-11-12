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
/** Cold original hard-line indexing composes Core origins without rescanning proposed source. */
import { requireValue } from '../../core/src/required.js';
import type {
  ReorderOutput,
  ReorderOriginPiece,
  SourceRange,
  PreparedDocumentPromptScope,
} from '../../core/src/index.js';
import type { ReorderProjection } from './reorder-projection.js';
import { SourceWidget } from './projection-widgets.js';
import type { NativeReorderCapture } from './reorder-command.js';
import { atomicReorderOrigins } from './reorder-origin-fragments.js';
/** A proposed hard line retains exact decoded-source UTF-16 bounds and canonical original origins. */
export interface SourceLineOrigin {
  readonly range: SourceRange;
  readonly pieces: readonly ReorderOriginPiece[];
  /** Compact capture-local token identifies the complete canonical origin recipe without copying it into each target. */
  readonly identity: number;
  /** Canonical replacement widgets can introduce breaks without exposing their underlying source newlines. */
  readonly widgetBreaks: number;
}
const capturedIndexes = new WeakMap<
  NativeReorderCapture,
  {
    readonly index: SourceLineOrigins;
    readonly outputs: WeakMap<ReorderOutput, readonly SourceLineOrigin[]>;
  }
>();
/** Genuine captured source/projection prepares one cold index; exact output identity reuses immutable recipes. */
export function preparedReorderSourceLines(
  capture: NativeReorderCapture,
  output: ReorderOutput,
): readonly SourceLineOrigin[] {
  if (output.origin.scope !== capture.prepared.scope)
    throw new Error('source-line-origin-source-mismatch');
  let captured = capturedIndexes.get(capture);
  if (!captured) {
    captured = {
      index: new SourceLineOrigins(capture.prepared.scope, capture.projection),
      outputs: new WeakMap(),
    };
    capturedIndexes.set(capture, captured);
  }
  const previous = captured.outputs.get(output);
  if (previous) return previous;
  const lines = captured.index.lines(output);
  captured.outputs.set(output, lines);
  return lines;
}
/** Original and finite derived carrier newline indexes have one captured-source lifetime. */
export class SourceLineOrigins {
  private readonly scope: PreparedDocumentPromptScope;
  private readonly endings: readonly SourceRange[];
  private readonly derived = new Map<string, readonly SourceRange[]>();
  private readonly identities = new Map<string, number>();
  private nextIdentity = 0;
  private readonly widgets: readonly {
    readonly range: SourceRange;
    readonly breaks: number;
  }[];
  /** Index canonical native widget occupancy in source order once; this derived index grants no editable state. */
  constructor(scope: PreparedDocumentPromptScope, projection: ReorderProjection | null) {
    this.scope = scope;
    const widgets: {
      readonly range: SourceRange;
      readonly breaks: number;
    }[] = [];
    projection?.decoded.between(0, scope.document.sourceText.length, (from, to, value) => {
      const widget: unknown = value.spec.widget;
      if (!(widget instanceof SourceWidget)) return;
      const breaks = widget.lineBreaks;
      if (!Number.isSafeInteger(breaks) || breaks < 0)
        throw new Error('source-line-widget-breaks-invalid');
      widgets.push(Object.freeze({ range: Object.freeze({ from, to }), breaks }));
    });
    // RangeSet.between has no callback ordering contract; both occupancy consumers require one source-ordered index.
    widgets.sort(
      (left, right) => left.range.from - right.range.from || left.range.to - right.range.to,
    );
    this.widgets = Object.freeze(widgets);
    let at = 0;
    this.endings = Object.freeze(
      this.breaks(scope.document.sourceText).filter((ending) => {
        while (
          at < widgets.length &&
          requireValue(widgets[at], 'source-line-origins').range.to <= ending.from
        )
          at++;
        return (
          !widgets[at] ||
          requireValue(widgets[at], 'source-line-origins').range.from > ending.from ||
          requireValue(widgets[at], 'source-line-origins').range.to < ending.to
        );
      }),
    );
  }
  /** Exact CR, LF and CRLF boundaries describe hard lines, without parsing prompt syntax. */
  private breaks(text: string): readonly SourceRange[] {
    const result: SourceRange[] = [];
    for (let at = 0; at < text.length; at++) {
      if (text[at] !== '\r' && text[at] !== '\n') continue;
      const from = at;
      if (text[at] === '\r' && text[at + 1] === '\n') at++;
      result.push(Object.freeze({ from, to: at + 1 }));
    }
    return Object.freeze(result);
  }
  /** Locate only original newline records overlapping the emitted source fragment. */
  private start(endings: readonly SourceRange[], from: number): number {
    let low = 0,
      high = endings.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (requireValue(endings[mid], 'source-line-origins').to <= from) low = mid + 1;
      else high = mid;
    }
    return low;
  }
  /** Intern complete prepared recipes once; retired entries never recycle tokens into different line layouts. */
  private identity(key: string): number {
    const previous = this.identities.get(key);
    if (previous !== undefined) return previous;
    if (!Number.isSafeInteger(this.nextIdentity + 1))
      throw new Error('source-line-identity-exhausted');
    if (this.identities.size === 256)
      this.identities.delete(
        requireValue(this.identities.keys().next().value, 'source-line-origins'),
      );
    const token = ++this.nextIdentity;
    this.identities.set(key, token);
    return token;
  }
  /** Compose indexed origin metadata; proposed text is read only at two-byte newline joins. */
  lines(output: ReorderOutput): readonly SourceLineOrigin[] {
    if (output.origin.scope !== this.scope) throw new Error('source-line-origin-source-mismatch');
    const pieces = atomicReorderOrigins(output),
      endings: SourceRange[] = [];
    for (const piece of pieces) {
      if (piece.kind === 'composed') throw new Error('source-line-origin-not-atomic');
      const literal =
        piece.kind === 'derived'
          ? output.text.slice(piece.outputRange.from, piece.outputRange.to)
          : null;
      let indexed = this.endings;
      if (literal !== null) {
        indexed = this.derived.get(literal) ?? this.breaks(literal);
        if (!this.derived.has(literal)) {
          if (this.derived.size === 64)
            this.derived.delete(
              requireValue(this.derived.keys().next().value, 'source-line-origins'),
            );
          this.derived.set(literal, indexed);
        }
      }
      const from = piece.kind === 'derived' ? 0 : piece.sourceRange.from,
        to =
          piece.kind === 'derived'
            ? requireValue(literal, 'source-line-origins').length
            : piece.sourceRange.to,
        shift = piece.outputRange.from - from;
      for (
        let at = this.start(indexed, from);
        at < indexed.length && requireValue(indexed[at], 'source-line-origins').from < to;
        at++
      ) {
        const ending = requireValue(indexed[at], 'source-line-origins');
        endings.push({
          from: Math.max(from, ending.from) + shift,
          to: Math.min(to, ending.to) + shift,
        });
      }
    }
    const joined: SourceRange[] = [];
    for (const ending of endings) {
      const previous = joined.at(-1);
      if (
        previous &&
        previous.to === ending.from &&
        output.text[previous.to - 1] === '\r' &&
        output.text[ending.from] === '\n'
      )
        joined[joined.length - 1] = { from: previous.from, to: ending.to };
      else joined.push(ending);
    }
    const result: SourceLineOrigin[] = [];
    let from = 0,
      pieceAt = 0;
    for (let number = 0; number <= joined.length; number++) {
      const to = joined[number]?.from ?? output.text.length,
        linePieces: ReorderOriginPiece[] = [],
        keys: string[] = [];
      let widgetBreaks = 0;
      while (
        pieceAt < pieces.length &&
        requireValue(pieces[pieceAt], 'source-line-origins').outputRange.to <= from
      )
        pieceAt++;
      for (
        let at = pieceAt;
        at < pieces.length && requireValue(pieces[at], 'source-line-origins').outputRange.from < to;
        at++
      ) {
        const piece = requireValue(pieces[at], 'source-line-origins'),
          start = Math.max(from, piece.outputRange.from),
          end = Math.min(to, piece.outputRange.to);
        if (piece.kind === 'composed') throw new Error('source-line-origin-not-atomic');
        const outputRange = Object.freeze({
          from: start - from,
          to: end - from,
        });
        if (piece.kind === 'derived') {
          const literal = output.text.slice(start, end);
          linePieces.push(
            Object.freeze({
              kind: 'derived',
              reason: piece.reason,
              outputRange,
            }),
          );
          keys.push(JSON.stringify([piece.reason, literal]));
        } else {
          const sourceRange = Object.freeze({
            from: piece.sourceRange.from + start - piece.outputRange.from,
            to: piece.sourceRange.from + end - piece.outputRange.from,
          });
          linePieces.push(Object.freeze({ kind: piece.kind, sourceRange, outputRange }));
          keys.push(`${sourceRange.from}:${sourceRange.to}`);
          let low = 0,
            high = this.widgets.length;
          while (low < high) {
            const mid = (low + high) >>> 1;
            if (requireValue(this.widgets[mid], 'source-line-origins').range.to <= sourceRange.from)
              low = mid + 1;
            else high = mid;
          }
          for (
            let at = low;
            at < this.widgets.length &&
            requireValue(this.widgets[at], 'source-line-origins').range.from < sourceRange.to;
            at++
          ) {
            const widget = requireValue(this.widgets[at], 'source-line-origins');
            if (widget.range.from < sourceRange.from || widget.range.to > sourceRange.to)
              throw new Error('source-line-origin-split-widget');
            widgetBreaks += widget.breaks;
          }
        }
      }
      result.push(
        Object.freeze({
          range: Object.freeze({ from, to }),
          pieces: Object.freeze(linePieces),
          identity: this.identity(keys.join('|')),
          widgetBreaks,
        }),
      );
      from = joined[number]?.to ?? to;
    }
    return Object.freeze(result);
  }
}
