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
/** Native reorder delegates grammar, origin serialization, storage, selection and history to their existing owners. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import {
  captureReorderSelection,
  mapReorderSelection,
  type PreparedValueReorder,
  type ReorderState,
  type ReorderSelection,
  type SourceRange,
} from '../../core/src/index.js';
import { featureEnabled } from '../../services/src/index.js';
import { featureProfile } from './features.js';
import {
  nativeDocumentValueAt,
  currentNativeDocumentValue,
  type NativeDocumentValueContext,
  type NativeDocumentValueTarget,
} from './document-values.js';
import {
  prepareDocumentValueEdit,
  currentDocumentValueEdit,
} from './document-value-edit-issuance.js';
import {
  selectionContents,
  selectionMatches,
  type NativeSelectionContents,
} from './native-selection.js';
import type {
  DocumentValueEditOutcome,
  DocumentValueSelectionRange,
} from './document-value-edits.js';
import {
  prepareNativeReorderPlan,
  prepareReorderProvenance,
  type PreparedNativeReorderPlan,
} from './reorder-provenance.js';
import { authorship } from './authorship.js';
import type { Authorship } from './authorship-ranges.js';
import { semanticStructures } from './structure.js';
import {
  prepareReorderProjection,
  currentReorderProjection,
  type ReorderProjection,
} from './reorder-projection.js';
import { richProjection, projectsRichSyntax } from './rich-projection.js';
import { regionPalette } from './region-paint.js';
/** One actual scope and original native selection intent; preview cannot grant editing authority. */
export interface NativeReorderCapture {
  readonly context: NativeDocumentValueContext;
  readonly target: NativeDocumentValueTarget;
  readonly prepared: PreparedValueReorder;
  readonly plan: PreparedNativeReorderPlan;
  readonly selection: readonly (
    | {
        readonly kind: 'inside';
        readonly selection: ReorderSelection;
      }
    | {
        readonly kind: 'outside';
        readonly index: number;
      }
  )[];
  readonly mainIndex: number;
  readonly active: number;
  readonly nativeSelection: NativeSelectionContents;
  readonly authored: Authorship | undefined;
  readonly projection: ReorderProjection | null;
  /** The regional palette base when the capture was taken. */
  readonly regionPalette: string;
}
const captures = new WeakSet<NativeReorderCapture>();
/** Current native permission and feature policy admit reorder in either presentation mode. */
export function reorderAllowed(state: EditorState): boolean {
  return !state.readOnly && featureEnabled(state.facet(featureProfile), 'segment_reorder');
}
/** Admit one canonical value and every directional range; regional and scene-bearing scopes use the same real Core transform. */
export function prepareNativeReorder(
  state: EditorState,
  context: NativeDocumentValueContext,
  range: SourceRange,
): NativeReorderCapture | null {
  if (!reorderAllowed(state) || context.composing) return null;
  const semantic = state.field(semanticStructures, false)?.native;
  if (
    !semantic ||
    semantic.nativeValues !== context.values ||
    semantic.documentSemantics !== context.semantics
  )
    return null;
  const admitted = nativeDocumentValueAt(state, context, range);
  if (admitted.status !== 'ready') return null;
  const target = admitted.target,
    plan = prepareNativeReorderPlan(state, context, target);
  if (!plan) return null;
  const prepared = plan.prepared;
  const projection = prepareReorderProjection(state, target);
  if (state.field(richProjection, false) && projectsRichSyntax(state) && !projection) return null;
  const selection: NativeReorderCapture['selection'][number][] = [];
  for (const [index, original] of state.selection.ranges.entries()) {
    if (
      original.to < target.entry.logicalStorage.from ||
      original.from > target.entry.logicalStorage.to
    ) {
      selection.push({ kind: 'outside', index });
      continue;
    }
    const map = requireValue(target.entry.semanticMapping, 'reorder-command'),
      anchor = map.logicalToDecoded[original.anchor - map.logicalRange.from],
      head = map.logicalToDecoded[original.head - map.logicalRange.from];
    if (anchor == null || head == null) return null;
    const captured = captureReorderSelection(prepared, anchor, head);
    if (!captured) return null;
    selection.push({ kind: 'inside', selection: captured });
  }
  const hit = captureReorderSelection(prepared, target.decodedRange.from, target.decodedRange.to);
  if (!hit) return null;
  const capture = Object.freeze({
    context: Object.freeze({ ...context }),
    target,
    prepared,
    plan,
    selection: Object.freeze(selection.map((s) => Object.freeze(s))),
    mainIndex: state.selection.mainIndex,
    active: hit.chip,
    nativeSelection: selectionContents(state.selection),
    authored: state.field(authorship, false),
    projection,
    regionPalette: state.facet(regionPalette),
  });
  captures.add(capture);
  return capture;
}
/** Metadata-only preview links retain actual target/selection/source membership; external changes retire the capture. */
export function currentNativeReorder(state: EditorState, capture: NativeReorderCapture): boolean {
  const semantic = state.field(semanticStructures, false)?.native;
  if (
    captures.has(capture) &&
    (!selectionMatches(state.selection, capture.nativeSelection) ||
      state.field(authorship, false) !== capture.authored ||
      !semantic ||
      !currentReorderProjection(state, capture.projection) ||
      semantic.nativeValues !== capture.context.values ||
      semantic.documentSemantics !== capture.context.semantics)
  ) {
    captures.delete(capture);
    return false;
  }
  return (
    captures.has(capture) &&
    reorderAllowed(state) &&
    currentNativeDocumentValue(state, capture.context, capture.target).status === 'ready'
  );
}
/** Gesture authority checks the genuine capture before delegating finite authored provenance preparation. */
export function prepareReorderAuthorship(
  state: EditorState,
  capture: NativeReorderCapture,
  order: ReorderState,
): ReturnType<typeof prepareReorderProvenance> {
  if (!currentNativeReorder(state, capture)) return null;
  return prepareReorderProvenance(state, capture.plan, order);
}
/** Compile one complete output and construct one genuine isolated transaction before any publication. */
export function prepareReorderCommit(
  state: EditorState,
  capture: NativeReorderCapture,
  order: ReorderState,
): DocumentValueEditOutcome {
  if (!currentNativeReorder(state, capture)) return { status: 'rejected', reason: 'stale-context' };
  const authored = prepareReorderAuthorship(state, capture, order);
  if (!authored) return { status: 'rejected', reason: 'unsafe-origin' };
  const { output, provenance } = authored;
  const unchanged = output.text === capture.prepared.scope.document.sourceText;
  const ranges: DocumentValueSelectionRange[] = [];
  for (const saved of capture.selection) {
    if (saved.kind === 'outside') {
      ranges.push(saved);
      continue;
    }
    const index = ranges.length,
      original = requireValue(state.selection.ranges[index], 'reorder-command'),
      map = requireValue(capture.target.entry.semanticMapping, 'reorder-command');
    const mapped = unchanged
      ? {
          anchor: requireValue(
            map.logicalToDecoded[original.anchor - map.logicalRange.from],
            'reorder-command',
          ),
          head: requireValue(
            map.logicalToDecoded[original.head - map.logicalRange.from],
            'reorder-command',
          ),
        }
      : mapReorderSelection(capture.prepared, output, saved.selection);
    if (!mapped) return { status: 'rejected', reason: 'unsafe-selection' };
    ranges.push({ kind: 'decoded', ...mapped });
  }
  const result = prepareDocumentValueEdit(state, {
    context: capture.context,
    target: capture.target,
    range: { from: 0, to: capture.prepared.scope.document.sourceText.length },
    text: output.text,
    selection: { ranges, mainIndex: capture.mainIndex },
    publication: {
      purpose: 'reorder',
      ...(unchanged
        ? {}
        : {
            reorder: provenance,
          }),
    },
  });
  if (
    result.status !== 'rejected' &&
    currentDocumentValueEdit(state, capture.context, result.edit) !== null
  )
    return { status: 'rejected', reason: 'foreign' };
  return result;
}
