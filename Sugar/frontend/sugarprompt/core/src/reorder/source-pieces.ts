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
/** Immutable carrier operations preserve exact source identity through the existing reorder decisions. */
import { requireValue } from '../required.js';
import type { PreparedDocumentPromptScope } from '../document/prompt-scopes.js';
import { freezeStructure } from '../prompt/text.js';
import type {
  ReorderDerivedReason,
  ReorderText,
  ReorderTextPiece,
  ReorderOriginPiece,
  ReorderOriginComponent,
  ReorderOutput,
  ReorderOriginBoundary,
} from './contracts.js';
/** A valid exact UTF-16 endpoint never separates a scalar pair or an authored CRLF. */
export function isReorderBoundary(text: string, at: number): boolean {
  if (!Number.isSafeInteger(at) || at < 0 || at > text.length) return false;
  const previous = text.charCodeAt(at - 1),
    next = text.charCodeAt(at);
  return (
    !(previous >= 0xd800 && previous <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) &&
    !(text[at - 1] === '\r' && text[at] === '\n')
  );
}
/** Capture an actual source slice; empty carriers own no phantom interval. */
export function sourceText(
  scope: PreparedDocumentPromptScope,
  from: number,
  to: number,
): ReorderText {
  const text = scope.document.sourceText;
  if (from > to || !isReorderBoundary(text, from) || !isReorderBoundary(text, to))
    throw new Error('reorder-source-boundary');
  return freezeStructure({
    scope,
    text: text.slice(from, to),
    pieces: from === to ? [] : [{ kind: 'source' as const, sourceRange: { from, to } }],
  });
}
/** Name newly synthesized separator bytes without claiming an original authored interval. */
export function derivedText(
  scope: PreparedDocumentPromptScope,
  text: string,
  reason: ReorderDerivedReason,
): ReorderText {
  return freezeStructure({
    scope,
    text,
    pieces: text ? [{ kind: 'derived' as const, text, reason }] : [],
  });
}
/** Join the existing chosen carriers while keeping equal-looking source intervals distinct. */
export function concatText(first: ReorderText, ...rest: readonly ReorderText[]): ReorderText {
  if (rest.some((value) => value.scope !== first.scope)) throw new Error('reorder-carrier-scope');
  return freezeStructure({
    scope: first.scope,
    text: [first, ...rest].map((value) => value.text).join(''),
    pieces: [first, ...rest].flatMap((value) => value.pieces),
  });
}
/** Slice carrier pieces by output-relative offsets; source identity translates rather than being inferred from text. */
export function sliceText(value: ReorderText, from: number, to = value.text.length): ReorderText {
  if (from > to || !isReorderBoundary(value.text, from) || !isReorderBoundary(value.text, to))
    throw new Error('reorder-carrier-boundary');
  const pieces: ReorderTextPiece[] = [];
  let cursor = 0;
  for (const piece of value.pieces) {
    const length =
      piece.kind === 'source' ? piece.sourceRange.to - piece.sourceRange.from : piece.text.length;
    const start = Math.max(from, cursor),
      end = Math.min(to, cursor + length);
    if (start < end)
      pieces.push(
        piece.kind === 'source'
          ? {
              kind: 'source',
              sourceRange: {
                from: piece.sourceRange.from + start - cursor,
                to: piece.sourceRange.from + end - cursor,
              },
            }
          : { ...piece, text: piece.text.slice(start - cursor, end - cursor) },
      );
    cursor += length;
  }
  return freezeStructure({
    scope: value.scope,
    text: value.text.slice(from, to),
    pieces,
  });
}
/** Reject malformed, foreign, sparse, or lossy carrier operands against the actual prepared scope. */
export function validReorderText(scope: PreparedDocumentPromptScope, value: ReorderText): boolean {
  if (
    !value ||
    value.scope !== scope ||
    typeof value.text !== 'string' ||
    !Array.isArray(value.pieces)
  )
    return false;
  const source = scope.document.sourceText;
  const texts: string[] = [];
  for (const piece of [...value.pieces]) {
    if (!piece) return false;
    if (piece.kind === 'source') {
      const range = piece.sourceRange;
      if (
        !range ||
        range.from >= range.to ||
        !isReorderBoundary(source, range.from) ||
        !isReorderBoundary(source, range.to)
      )
        return false;
      texts.push(source.slice(range.from, range.to));
    } else if (
      piece.kind === 'derived' &&
      piece.text &&
      typeof piece.text === 'string' &&
      (piece.reason === 'row-separator'
        ? /^(?: |, )$/u.test(piece.text)
        : piece.reason === 'blank-line-comma'
          ? piece.text === ','
          : piece.reason === 'empty-partition-ending' && /^(?:\r\n|\r|\n)$/u.test(piece.text))
    )
      texts.push(piece.text);
    else return false;
  }
  return texts.join('') === value.text;
}
/** Group only newly joined atomic CRLF/scalar units; all source intervals retain valid original endpoints. */
export function atomicOriginPieces(
  text: string,
  pieces: readonly ReorderOriginPiece[],
): readonly ReorderOriginPiece[] {
  const joined = new Set(
    pieces
      .slice(1)
      .map((piece) => piece.outputRange.from)
      .filter((at) => !isReorderBoundary(text, at)),
  );
  if (!joined.size) return freezeStructure(pieces);
  const split: ReorderOriginPiece[] = [];
  for (const piece of pieces) {
    if (piece.kind === 'composed') throw new Error('reorder-origin-already-composed');
    const cuts = [
      piece.outputRange.from,
      ...(joined.has(piece.outputRange.from) ? [piece.outputRange.from + 1] : []),
      ...(joined.has(piece.outputRange.to) ? [piece.outputRange.to - 1] : []),
      piece.outputRange.to,
    ].sort((a, b) => a - b);
    for (let index = 0; index < cuts.length - 1; index++) {
      const from = requireValue(cuts[index], 'source-pieces'),
        to = requireValue(cuts[index + 1], 'source-pieces');
      if (from === to) continue;
      const outputRange = { from, to };
      if (piece.kind === 'derived') split.push({ ...piece, outputRange });
      else {
        const sourceRange = {
          from: piece.sourceRange.from + from - piece.outputRange.from,
          to: piece.sourceRange.from + to - piece.outputRange.from,
        };
        split.push({
          kind: sourceRange.from === from && sourceRange.to === to ? 'unchanged' : 'copied',
          sourceRange,
          outputRange,
        });
      }
    }
  }
  const result: ReorderOriginPiece[] = [];
  /** Retain the exact provenance of a half-unit, exposing its position only through the aggregate range. */
  function component(piece: ReorderOriginPiece): ReorderOriginComponent {
    if (piece.kind === 'composed') throw new Error('reorder-origin-component');
    return piece.kind === 'derived'
      ? {
          kind: 'derived',
          text: text.slice(piece.outputRange.from, piece.outputRange.to),
          reason: piece.reason,
        }
      : { kind: piece.kind, sourceRange: piece.sourceRange };
  }
  for (let index = 0; index < split.length; index++) {
    const left = requireValue(split[index], 'source-pieces');
    if (joined.has(left.outputRange.to)) {
      const right = split[++index];
      if (
        !right ||
        left.outputRange.to !== right.outputRange.from ||
        left.outputRange.to - left.outputRange.from !== 1 ||
        right.outputRange.to - right.outputRange.from !== 1
      )
        throw new Error('reorder-origin-atomic-unit');
      result.push({
        kind: 'composed',
        outputRange: { from: left.outputRange.from, to: right.outputRange.to },
        components: [component(left), component(right)],
      });
    } else result.push(left);
  }
  return freezeStructure(result);
}
/** Observe every associated source-boundary image; removed bytes and output-unit interiors never become a guessed caret. */
export function reorderOriginBoundary(
  output: ReorderOutput,
  position: number,
  association: -1 | 1,
): ReorderOriginBoundary {
  if (!isReorderBoundary(output.origin.scope.document.sourceText, position))
    return freezeStructure({
      status: 'rejected',
      reason: 'invalid-source-boundary',
    });
  const images: Extract<
    ReorderOriginBoundary,
    {
      status: 'observed';
    }
  >['images'][number][] = [];
  const seen = new Set<string>();
  /** Inspect exact copied fragments; source-boundary association chooses a side of any removed/reordered junction. */
  function observe(
    component: ReorderOriginComponent,
    from: number,
    unitRange: import('../offsets.js').SourceRange,
  ): void {
    if (component.kind === 'derived') return;
    const source = component.sourceRange;
    const included =
      association < 0
        ? source.from < position && position <= source.to
        : source.from <= position && position < source.to;
    if (!included) return;
    const at = from + position - source.from;
    const image = isReorderBoundary(output.text, at)
      ? { kind: 'boundary' as const, position: at }
      : { kind: 'interior' as const, unitRange };
    const key =
      image.kind === 'boundary'
        ? 'b:' + String(image.position)
        : 'i:' + String(image.unitRange.from) + ':' + String(image.unitRange.to);
    if (!seen.has(key)) {
      seen.add(key);
      images.push(image);
    }
  }
  for (const piece of output.origin.pieces) {
    if (piece.kind === 'derived') continue;
    if (piece.kind === 'composed') {
      let from = piece.outputRange.from;
      for (const component of piece.components) {
        observe(component, from, piece.outputRange);
        from +=
          component.kind === 'derived'
            ? component.text.length
            : component.sourceRange.to - component.sourceRange.from;
      }
    } else observe(piece, piece.outputRange.from, piece.outputRange);
  }
  return freezeStructure({ status: 'observed', images });
}
