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
/** A whole-line native paint admission refusal can retain complete unwrapped held paint under the unchanged native snapshot limits. */
import { requireValue } from '../../core/src/required.js';
import { atomicReorderPaintIsland } from './reorder-atomic-paint-island.js';
import { nativeReorderFootprint } from './reorder-native-footprint.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderGrabGeometry } from './reorder-grab-geometry.js';
import type { PreparedReorderPaint } from './reorder-paint-preparation.js';
import type { ReorderGrabCopy, ReorderGrabClip } from './reorder-grab-materialization.js';
import { admitNativeReorderPaint, nativeReorderLineSize } from './reorder-native-paint.js';
import { prepareNativeReorderPaint, preparedAtomicPaintRole } from './reorder-paint-preparation.js';
import { reorderGrabGeometry } from './reorder-grab-geometry.js';
import type { ReorderRect } from './reorder-geometry.js';
/** Immutable held-only native output is fully prepared before any destination node exists. */
export interface PreparedSparseReorderGrab {
  readonly geometry: ReorderGrabGeometry;
  readonly prepared: PreparedReorderPaint;
  readonly copies: readonly ReorderGrabCopy[];
  readonly clips: readonly ReorderGrabClip[];
}
/** Native geometry admits a single-fragment carrier and retains its complete descendant footprint before allocation. */
export function prepareSparseReorderGrab(
  view: EditorView,
  roots: readonly HTMLElement[],
  held: number,
  press: {
    readonly x: number;
    readonly y: number;
  },
): PreparedSparseReorderGrab | null {
  if (!roots.length || roots.length > 16) return null;
  const nodes = admitNativeReorderPaint(roots);
  if (!nodes) return null;
  const rects: ReorderRect[] = [],
    metrics: {
      readonly root: HTMLElement;
      readonly rect: DOMRect;
      readonly scaleX: number;
      readonly scaleY: number;
      readonly outlineX: number;
      readonly outlineY: number;
      readonly footprint: ReorderGrabClip['rect'];
    }[] = [];
  for (const root of roots) {
    const native = root.getClientRects();
    if (native.length > 4096) return null;
    const fragments: DOMRect[] = [];
    for (const rect of native) {
      if (
        ![rect.left, rect.right, rect.top, rect.bottom, rect.width, rect.height].every(
          Number.isFinite,
        ) ||
        rect.width < 0 ||
        rect.height < 0
      )
        return null;
      if (rect.width > 0 && rect.height > 0) fragments.push(rect);
    }
    if (fragments.length !== 1) return null;
    const rect = requireValue(fragments[0], 'reorder-sparse-grab-paint');
    if (
      ![rect.left, rect.right, rect.top, rect.bottom, rect.width, rect.height].every(
        Number.isFinite,
      ) ||
      rect.width <= 0 ||
      rect.height <= 0
    )
      return null;
    const line = root.closest<HTMLElement>('.cm-line');
    if (!line || !view.contentDOM.contains(line)) return null;
    const size = nativeReorderLineSize(view, line),
      box = line.getBoundingClientRect();
    if (!size) return null;
    const scaleX = box.width / size.width,
      scaleY = box.height / size.height;
    if (![scaleX, scaleY].every((scale) => Number.isFinite(scale) && scale > 0)) return null;
    const complete = nativeReorderFootprint(root, nodes, held);
    if (!complete?.length || complete.length + rects.length > 4096) return null;
    rects.push(...complete);
    const style = requireValue(
        view.dom.ownerDocument.defaultView,
        'reorder-sparse-grab-paint',
      ).getComputedStyle(root),
      width = Number.parseFloat(style.outlineWidth),
      offset = Number.parseFloat(style.outlineOffset);
    const outline =
      style.outlineStyle !== 'none' && Number.isFinite(width)
        ? Math.max(0, width + (Number.isFinite(offset) ? offset : 0))
        : 0;
    metrics.push({
      root,
      rect,
      scaleX,
      scaleY,
      outlineX: outline * scaleX,
      outlineY: outline * scaleY,
      // Inline font boxes need not contain rich chrome or fractionally rounded descendants.
      footprint: {
        left: Math.min(...complete.map((part) => part.left)),
        right: Math.max(...complete.map((part) => part.right)),
        top: Math.min(...complete.map((part) => part.top)),
        bottom: Math.max(...complete.map((part) => part.bottom)),
      },
    });
  }
  const geometry = reorderGrabGeometry(rects, held, press);
  if (!geometry) return null;
  const prepared = prepareNativeReorderPaint(view, roots, nodes, 'atomic-grab');
  if (!prepared) return null;
  return Object.freeze({
    geometry,
    prepared,
    copies: Object.freeze(
      metrics.map(({ root, rect, scaleX, scaleY }) =>
        Object.freeze({
          node: root,
          kind: 'fragment' as const,
          rect: Object.freeze({ left: rect.left, top: rect.top }),
          width: rect.width / scaleX,
          height: rect.height / scaleY,
          scaleX,
          scaleY,
        }),
      ),
    ),
    clips: Object.freeze(
      metrics.map(({ footprint, outlineX, outlineY }) =>
        Object.freeze({
          rect: Object.freeze(footprint),
          outlineX,
          outlineY,
        }),
      ),
    ),
  });
}
/** Only one outermost independent inline island may replace an admitted whole line; complete wrapper chrome remains in the plan. */
export function atomicSparseReorderGrab(plan: PreparedSparseReorderGrab): boolean {
  return (
    plan.copies.length === 1 &&
    atomicReorderPaintIsland(
      requireValue(plan.copies[0], 'reorder-sparse-grab-paint').node,
      (node) => preparedAtomicPaintRole(plan.prepared.get(node)),
    ) !== null
  );
}
