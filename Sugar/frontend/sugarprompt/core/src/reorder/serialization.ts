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
/** Exact serialization emits source provenance and paired shell occurrences without reparsing. */
import { requireValue } from '../required.js';
import { freezeStructure } from '../prompt/text.js';
import type {
  ReorderChip,
  ReorderEnvelope,
  ReorderEnvelopeOccurrence,
  ReorderOriginPiece,
  ReorderOutput,
  ReorderState,
  PreparedValueReorder,
  ReorderText,
  ReorderTextPiece,
} from './contracts.js';
import type { SourceRange } from '../offsets.js';
import {
  isReorderBoundary,
  sourceText,
  derivedText,
  validReorderText,
  atomicOriginPieces,
  concatText,
} from './source-pieces.js';
/** Count shared original shells; numeric weight equality cannot merge independently authored identities. */
function sharedDepth(left: readonly ReorderEnvelope[], right: readonly ReorderEnvelope[]): number {
  let depth = 0;
  while (depth < left.length && left[depth]?.span === right[depth]?.span) depth++;
  return depth;
}
/** Reject malformed source pieces and pure order operands against the actual source before emitting output. */
export function validReorderState(
  prepared: PreparedValueReorder,
  state: ReorderState,
  complete = true,
): boolean {
  const scope = prepared.scope,
    source = scope.document.sourceText,
    emphasis = new Set(scope.document.emphasisSpans);
  return (
    state.slots.length === Math.max(0, state.order.length - 1) &&
    [...state.slots, state.prefix, state.suffix].every((value) => validReorderText(scope, value)) &&
    prepared.chips.every(
      (chip) =>
        chip.text === source.slice(chip.contentRange.from, chip.contentRange.to) &&
        isReorderBoundary(source, chip.contentRange.from) &&
        isReorderBoundary(source, chip.contentRange.to) &&
        validReorderText(scope, chip.leading) &&
        validReorderText(scope, chip.trailing) &&
        chip.envelopes.every(
          (envelope) =>
            emphasis.has(envelope.span) &&
            envelope.opening ===
              source.slice(envelope.span.outerRange.from, envelope.span.contentRange.from) &&
            envelope.closing ===
              source.slice(envelope.span.contentRange.to, envelope.span.outerRange.to),
        ),
    ) &&
    state.partitions.length === prepared.chips.length &&
    (!complete || state.order.length === prepared.chips.length) &&
    new Set(state.order).size === state.order.length &&
    [...state.order].every((id) => Number.isSafeInteger(id) && !!prepared.chips[id]) &&
    [...state.partitions].every(
      (partition) =>
        Number.isSafeInteger(partition) &&
        partition >= 0 &&
        partition < scope.document.regions.partitions.length,
    )
  );
}
/** Serialize one complete current order with exact source origins and every reopened shell occurrence. */
export function serializeValueReorder(
  prepared: PreparedValueReorder,
  state: ReorderState = prepared.initial,
): ReorderOutput | null {
  return validReorderState(prepared, state) ? serializeState(prepared, state) : null;
}
/** Prepare geometry output only; a missing source chip never enters the complete commit serializer. */
export function serializeReorderPreview(
  prepared: PreparedValueReorder,
  hidden: ReorderState,
): ReorderOutput | null {
  return validReorderState(prepared, hidden, false) &&
    hidden.order.length === prepared.chips.length - 1
    ? serializeState(prepared, hidden)
    : null;
}
/** Source-bound append instructions share shell/padding placement between output and isolated chip geometry. */
type ChipPart =
  | {
      readonly kind: 'text';
      readonly value: ReorderText;
    }
  | {
      readonly kind: 'content';
      readonly value: ReorderText;
    }
  | {
      readonly kind: 'opening';
      readonly envelope: ReorderEnvelope;
    }
  | {
      readonly kind: 'closing';
      readonly envelope: ReorderEnvelope;
    };
/** Place original padding at its actual enclosing shell boundary instead of moving it outside nested syntax. */
function chipParts(chip: ReorderChip, shared: number, trailingShared: number): readonly ChipPart[] {
  const parts: ChipPart[] = [],
    leading = [...chip.leading.pieces],
    trailing = [...chip.trailing.pieces];
  /** Preserve the exact identity of one previously prepared source or derived padding fragment. */
  function text(piece: ReorderTextPiece): void {
    parts.push({
      kind: 'text',
      value:
        piece.kind === 'source'
          ? sourceText(chip.leading.scope, piece.sourceRange.from, piece.sourceRange.to)
          : derivedText(chip.leading.scope, piece.text, piece.reason),
    });
  }
  for (const envelope of chip.envelopes.slice(shared)) {
    while (
      leading[0] &&
      (leading[0].kind === 'derived' || leading[0].sourceRange.to <= envelope.span.outerRange.from)
    )
      text(requireValue(leading.shift(), 'serialization'));
    parts.push({ kind: 'opening', envelope });
  }
  for (const piece of leading) text(piece);
  parts.push({
    kind: 'content',
    value: sourceText(chip.leading.scope, chip.contentRange.from, chip.contentRange.to),
  });
  for (const envelope of chip.envelopes.slice(trailingShared).reverse()) {
    while (
      trailing[0] &&
      (trailing[0].kind === 'derived' || trailing[0].sourceRange.from < envelope.span.outerRange.to)
    )
      text(requireValue(trailing.shift(), 'serialization'));
    parts.push({ kind: 'closing', envelope });
  }
  for (const piece of trailing) text(piece);
  return parts;
}
/** Compile source fragments and actual span occurrences together during the existing append decisions. */
function serializeState(prepared: PreparedValueReorder, state: ReorderState): ReorderOutput | null {
  const parts: string[] = [],
    pieces: ReorderOriginPiece[] = [],
    envelopes: ReorderEnvelopeOccurrence[] = [];
  let cursor = 0,
    open: readonly ReorderEnvelope[] = [];
  const active: {
    readonly envelope: ReorderEnvelope;
    readonly from: number;
    readonly contentFrom: number;
  }[] = [];
  const contentRanges: Record<number, SourceRange> = Object.create(null) as Record<
    number,
    SourceRange
  >;
  const renderedRanges: Record<number, SourceRange> = Object.create(null) as Record<
    number,
    SourceRange
  >;
  const slotRanges: SourceRange[] = [];
  /** Append each real carrier fragment without deriving its identity from text equality. */
  function append(value: ReorderText): void {
    for (const piece of value.pieces) {
      const text =
        piece.kind === 'source'
          ? prepared.scope.document.sourceText.slice(piece.sourceRange.from, piece.sourceRange.to)
          : piece.text;
      const outputRange = { from: cursor, to: cursor + text.length };
      pieces.push(
        piece.kind === 'source'
          ? {
              kind:
                piece.sourceRange.from === outputRange.from &&
                piece.sourceRange.to === outputRange.to
                  ? 'unchanged'
                  : 'copied',
              sourceRange: piece.sourceRange,
              outputRange,
            }
          : { kind: 'derived', outputRange, reason: piece.reason },
      );
      parts.push(text);
      cursor += text.length;
    }
  }
  /** Opening uses the actual original shell interval and starts one paired output occurrence. */
  function opening(envelope: ReorderEnvelope): void {
    const from = cursor;
    append(
      sourceText(prepared.scope, envelope.span.outerRange.from, envelope.span.contentRange.from),
    );
    active.push({ envelope, from, contentFrom: cursor });
  }
  /** Closing records the exact content and outer bounds for this emitted occurrence, including reopened copies. */
  function closing(envelope: ReorderEnvelope): void {
    const occurrence = active.pop();
    if (occurrence?.envelope.span !== envelope.span) throw new Error('reorder-envelope-pair');
    const contentTo = cursor;
    append(sourceText(prepared.scope, envelope.span.contentRange.to, envelope.span.outerRange.to));
    envelopes.push({
      span: envelope.span,
      outerRange: { from: occurrence.from, to: cursor },
      contentRange: { from: occurrence.contentFrom, to: contentTo },
    });
  }
  append(state.prefix);
  for (const [ordinal, id] of state.order.entries()) {
    const chip = requireValue(prepared.chips[id], 'serialization'),
      shared = sharedDepth(open, chip.envelopes);
    for (const envelope of open.slice(shared).reverse()) closing(envelope);
    const renderedFrom = cursor;
    const next = prepared.chips[state.order[ordinal + 1] ?? -1],
      trailingShared = sharedDepth(chip.envelopes, next?.envelopes ?? []);
    for (const part of chipParts(chip, shared, trailingShared)) {
      if (part.kind === 'opening') opening(part.envelope);
      else if (part.kind === 'closing') closing(part.envelope);
      else {
        const from = cursor;
        append(part.value);
        if (part.kind === 'content') contentRanges[id] = { from, to: cursor };
      }
    }
    renderedRanges[id] = { from: renderedFrom, to: cursor };
    open = chip.envelopes.slice(0, trailingShared);
    const slot = state.slots[ordinal];
    if (slot !== undefined) {
      const slotFrom = cursor;
      append(slot);
      slotRanges.push({ from: slotFrom, to: cursor });
    }
  }
  append(state.suffix);
  if (active.length) throw new Error('reorder-envelope-unclosed');
  const text = parts.join('');
  const origins = atomicOriginPieces(text, pieces);
  return freezeStructure({
    text,
    contentRanges,
    renderedRanges,
    slotRanges,
    origin: { scope: prepared.scope, pieces: origins, envelopes },
  });
}
/** Prepare isolated display source and its decoded UTF-16 origins through the same shell/padding serializer. */
export function reorderChipSource(chip: ReorderChip): ReorderText {
  const parts = chipParts(chip, 0, 0).map((part) =>
    part.kind === 'opening'
      ? sourceText(
          chip.leading.scope,
          part.envelope.span.outerRange.from,
          part.envelope.span.contentRange.from,
        )
      : part.kind === 'closing'
        ? sourceText(
            chip.leading.scope,
            part.envelope.span.contentRange.to,
            part.envelope.span.outerRange.to,
          )
        : part.value,
  );
  return concatText(requireValue(parts[0], 'serialization'), ...parts.slice(1));
}
/** Prepare isolated proxy text while retaining the same source-owned shell and padding spellings. */
export function reorderChipText(chip: ReorderChip): string {
  return reorderChipSource(chip).text;
}
