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
/** Preview preparation delegates exact output to Core and compares only prepared source provenance. */
import { requireValue } from '../../core/src/required.js';
import { nativeReorderRecipe } from './reorder-native-recipe.js';
import {
  parseScenes,
  serializeReorderPreview,
  serializeValueReorder,
  type ReorderState,
  type ReorderText,
} from '../../core/src/index.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { prepareReorderStoragePaint } from './reorder-storage-paint.js';
/** Every publication input comes from the current native gesture; this adapter grants no commit authority. */
export type ReorderPreviewInput = Omit<ReorderPreview, 'output' | 'presentation' | 'scenes'>;
const prepared = new WeakSet<ReorderPreview>();
/** Weak captured-order membership retires with existing source/profile admission, retaining no global source catalog. */
const sceneDocuments = new WeakMap<
  ReorderPreview['capture'],
  WeakMap<ReorderState, ReorderPreview['scenes']>
>();
/** Changed prompt orders delegate recognition solely to Core; output is decoded scope-local UTF-16 with the prompt identity mapping preserving exact CR/LF/CRLF. Structured values and raw projections have no Scene document; native logical-CM positions never enter this parser. Unchanged canonical order borrows its actual existing document. */
function preparePreviewScenes(
  input: ReorderPreviewInput,
  source: string,
): ReorderPreview['scenes'] {
  const canonical = input.capture.projection
    ? input.capture.prepared.scope.sceneContexts?.document
    : null;
  if (!canonical) return null;
  let orders = sceneDocuments.get(input.capture);
  if (!orders) {
    orders = new WeakMap();
    sceneDocuments.set(input.capture, orders);
  }
  const previous = orders.get(input.order);
  if (previous) return previous;
  const scenes = sameReorderOrder(input.order, input.capture.prepared.initial)
    ? canonical
    : parseScenes(source);
  orders.set(input.order, scenes);
  return scenes;
}
/** Serialize complete and hidden operands once through the canonical exact-source owner. */
export function prepareReorderPreview(
  input: ReorderPreviewInput,
  presentation: ReorderPreview['presentation'],
): ReorderPreview | null {
  const output =
    presentation === 'collision'
      ? serializeReorderPreview(input.capture.prepared, input.order)
      : input.order === input.capture.prepared.initial
        ? (nativeReorderRecipe(input.capture.context.values, input.capture.target.entry)?.output ??
          null)
        : serializeValueReorder(input.capture.prepared, input.order);
  if (!output) return null;
  const scenes = preparePreviewScenes(input, output.text);
  const preview = Object.freeze({ ...input, output, scenes, presentation });
  prepareReorderStoragePaint(preview);
  prepared.add(preview);
  return preview;
}
/** Origin-keyed layout consumes only exact Core-prepared immutable publications, never assembled display DTOs. */
export function preparedReorderPreview(preview: ReorderPreview): boolean {
  return prepared.has(preview);
}
/** Issued serialization and canonical origin identity admit reuse without another full text comparison. */
export function samePreparedReorderPaint(a: ReorderPreview, b: ReorderPreview): boolean {
  return (
    a === b ||
    (prepared.has(a) &&
      prepared.has(b) &&
      a.token === b.token &&
      a.capture === b.capture &&
      a.activeChip === b.activeChip &&
      a.presentation === b.presentation &&
      sameReorderOrder(a.order, b.order) &&
      a.order.order.every((id) => {
        const left = a.output.renderedRanges[id],
          right = b.output.renderedRanges[id];
        return !!left && !!right && left.from === right.from && left.to === right.to;
      }))
  );
}
/** Carrier origin identity preserves exact captured bytes without scanning large source strings. */
function sameCarrier(a: ReorderText, b: ReorderText): boolean {
  return (
    a === b ||
    (a.scope === b.scope &&
      a.pieces.length === b.pieces.length &&
      a.pieces.every((piece, at) => {
        const other = requireValue(b.pieces[at], 'reorder-preview-preparation');
        return piece.kind === 'source'
          ? other.kind === 'source' &&
              piece.sourceRange.from === other.sourceRange.from &&
              piece.sourceRange.to === other.sourceRange.to
          : other.kind === 'derived' && piece.reason === other.reason && piece.text === other.text;
      }))
  );
}
/** Canonical chip/shell and carrier identities completely determine source-neutral paint equality. */
export function sameReorderOrder(a: ReorderState, b: ReorderState): boolean {
  return (
    a === b ||
    (a.order.length === b.order.length &&
      a.order.every((id, at) => id === b.order[at]) &&
      a.partitions.length === b.partitions.length &&
      a.partitions.every((partition, at) => partition === b.partitions[at]) &&
      sameCarrier(a.prefix, b.prefix) &&
      sameCarrier(a.suffix, b.suffix) &&
      a.slots.length === b.slots.length &&
      a.slots.every((slot, at) =>
        sameCarrier(slot, requireValue(b.slots[at], 'reorder-preview-preparation')),
      ))
  );
}
