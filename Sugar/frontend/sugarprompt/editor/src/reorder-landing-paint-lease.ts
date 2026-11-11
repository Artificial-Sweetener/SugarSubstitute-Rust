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
/** Landing geometry borrows the actual canonical replacement or original native paint owner under one finite admission. */
import { requireValue } from '../../core/src/required.js';
import { nativeReorderRecipe } from './reorder-native-recipe.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderRect, ReorderMeasuredGeometry } from './reorder-geometry.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { reorderPreview } from './reorder-preview.js';
import { currentReorderPaintLease } from './reorder-widget.js';
import { currentNativeReorder } from './reorder-command.js';
import { readReorderBaseContext } from './reorder-base-context.js';
import { admitReorderLineLayout } from './reorder-layout-admission.js';
import { nativeReorderViewportFootprint } from './reorder-native-paint.js';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import { reorderHeldMarker } from './reorder-geometry.js';
/** One existing landing footprint retains its exact publication and geometry authority, never another source or layout owner. */
export interface ReorderLandingObservation {
  readonly preview: ReorderPreview;
  readonly held: number;
  readonly current: () => boolean;
  readonly rects: readonly ReorderRect[];
  readonly geometry: readonly ReorderRect[];
  readonly viewport: ReorderMeasuredGeometry['viewport'];
  readonly color: string;
}
/** Observe native or issued complete endpoint paint under the caller's exact READ lease; epochs remain with pointer placement. */
export function observeReorderLandingPaint(
  view: EditorView,
  input: ReorderMeasureInput,
  preview: ReorderPreview,
  lease: () => boolean,
  cached: ReorderLandingObservation | null,
  rects: readonly ReorderRect[],
  display: ReorderLandingObservation | null,
) {
  if (view.state.field(reorderPreview, false)?.pointer?.mode === 'stage' && !display) return null;
  const viewport = display?.viewport ?? view.scrollDOM.getBoundingClientRect(),
    held = input.held;
  let observation = cached,
    footprint: readonly ReorderRect[] | null | undefined;
  if (
    !cached ||
    cached.held !== held ||
    cached.geometry !== rects ||
    cached.viewport.left !== viewport.left ||
    cached.viewport.right !== viewport.right ||
    cached.viewport.top !== viewport.top ||
    cached.viewport.bottom !== viewport.bottom ||
    !cached.current()
  )
    footprint = display?.rects ?? nativeReorderViewportFootprint(view, held);
  const recipe = nativeReorderRecipe(preview.capture.context.values, preview.capture.target.entry);
  if (!recipe && !display) return null;
  const heldNode = display
      ? null
      : view.contentDOM.querySelector<HTMLElement>(
          `[data-reorder-native-value="${requireValue(recipe, 'reorder-landing-paint-lease').key}"][data-reorder-chip="${held}"]`,
        ),
    color =
      display?.color ??
      (heldNode
        ? requireValue(
            view.dom.ownerDocument.defaultView,
            'reorder-landing-paint-lease',
          ).getComputedStyle(heldNode).outlineColor
        : 'currentColor');
  if (footprint !== undefined)
    observation = footprint
      ? {
          held,
          preview,
          current: lease,
          rects: footprint,
          geometry: rects,
          color,
          viewport: {
            left: viewport.left,
            right: viewport.right,
            top: viewport.top,
            bottom: viewport.bottom,
          },
        }
      : null;
  return {
    observation,
    lease,
    landingRects: observation?.rects ?? [],
    rect: input.hidden ? reorderHeldMarker(input.order, input.hidden, held, rects, viewport) : null,
    rects,
    viewport,
    color,
  };
}
/** READ admits exact native publication, source/context and actual held subtree; WRITE may only borrow that unchanged output. */
function currentNativeReorderPaintLease(
  view: EditorView,
  preview: ReorderPreview,
  retained: Pick<ReorderLandingObservation, 'held' | 'current' | 'preview'> | null,
): (() => boolean) | null {
  const field = view.state.field(reorderPreview, false);
  /** A later mask, source publication or canonical transition cannot renew this native issue. */
  const issued = (): boolean =>
    field?.native === true &&
    field.value === preview &&
    view.state.field(reorderPreview, false) === field &&
    currentNativeReorder(view.state, preview.capture) &&
    view.state.field(reorderPreview, false) === field;
  if (!issued()) return null;
  if (
    retained?.preview === preview &&
    retained.held === preview.activeChip &&
    retained.current() &&
    issued()
  )
    return retained.current;
  const context = readReorderBaseContext(view, preview);
  if (!context || !issued()) return null;
  const recipe = nativeReorderRecipe(preview.capture.context.values, preview.capture.target.entry);
  if (!recipe) return null;
  const roots = Array.from(
    view.contentDOM.querySelectorAll<HTMLElement>(
      `[data-reorder-native-value="${recipe.key}"][data-reorder-chip="${preview.activeChip}"]`,
    ),
  );
  if (!roots.length || roots.length > 4096) return null;
  const nodes = admitReorderLineLayout(roots);
  if (!nodes || !issued()) return null;
  const stamps = nodes.map((node) => ({
    node,
    parent: node.parentNode,
    children: Array.from(node.childNodes),
    classes: node.nodeType === 1 ? (node as Element).getAttribute('class') : null,
    style: node.nodeType === 1 ? (node as Element).getAttribute('style') : null,
  }));
  /** Context and native subtree remain borrowed from their real owners; no geometry is read during consumption. */
  const current = (): boolean =>
    issued() &&
    context.current() &&
    issued() &&
    roots.every(
      (root) =>
        view.contentDOM.contains(root) &&
        root.getAttribute('data-reorder-chip') === String(preview.activeChip),
    ) &&
    stamps.every(
      (stamp) =>
        stamp.node.isConnected &&
        stamp.node.parentNode === stamp.parent &&
        stamp.node.childNodes.length === stamp.children.length &&
        stamp.children.every((child, at) => stamp.node.childNodes[at] === child) &&
        (stamp.node.nodeType !== 1 ||
          ((stamp.node as Element).getAttribute('class') === stamp.classes &&
            (stamp.node as Element).getAttribute('style') === stamp.style)),
    ) &&
    issued();
  return current() ? current : null;
}
/** Borrow an existing exact-current lease before cold admission, then fence geometry against synchronous native supersession. */
export function readReorderLandingPaint<T>(
  view: EditorView,
  preview: ReorderPreview | null,
  retained: Pick<ReorderLandingObservation, 'held' | 'current' | 'preview'> | null,
  read: (current: () => boolean, preview: ReorderPreview) => T,
): T | null {
  if (!preview || view.state.field(reorderPreview, false)?.pointer?.mode === 'stage') return null;
  const lease =
    currentReorderPaintLease(view, preview) ??
    currentNativeReorderPaintLease(view, preview, retained);
  if (!lease || !lease()) return null;
  const output = read(lease, preview);
  return lease() ? output : null;
}
