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
/** Capture complete native line paint, or one complete unwrapped held subtree when its surrounding line exceeds admission, before hiding the original. */
import { requireValue } from '../../core/src/required.js';
import { withReorderPaintMaterialization } from './reorder-paint-materialization.js';
import { nativeReorderFootprint } from './reorder-native-footprint.js';
import { prepareSparseReorderGrab, atomicSparseReorderGrab } from './reorder-sparse-grab-paint.js';
import { materializeReorderGrab } from './reorder-grab-materialization.js';
import { prepareNativeReorderPaint } from './reorder-paint-preparation.js';
import type { EditorView } from '@codemirror/view';
import { admitNativeReorderPaint, nativeReorderLineSize } from './reorder-native-paint.js';
import { reorderGrabGeometry } from './reorder-grab-geometry.js';
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
import type { ReorderRect } from './reorder-geometry.js';
/** Capture admitted native held paint before the original carrier becomes hidden. */
export function captureReorderGrabPaint(
  view: EditorView,
  held: number,
  press: {
    readonly x: number;
    readonly y: number;
  },
): ReorderGrabPaint | null {
  const lines: HTMLElement[] = [];
  const nativeValue = view.dom.getAttribute('data-reorder-native-value');
  if (nativeValue !== null && !/^(0|[1-9][0-9]*)$/u.test(nativeValue)) return null;
  const heldRoots = view.contentDOM.querySelectorAll<HTMLElement>(
    '[data-reorder-chip="' +
      String(held) +
      '"]' +
      (nativeValue === null
        ? ':not([data-reorder-native-value])'
        : '[data-reorder-native-value="' + nativeValue + '"]'),
  );
  if (heldRoots.length > 1024) return null;
  for (const root of heldRoots) {
    const line = root.closest<HTMLElement>('.cm-line');
    if (!line || !view.contentDOM.contains(line)) return null;
    if (!lines.includes(line)) lines.push(line);
    if (lines.length > 16) return null;
  }
  if (!lines.length) return null;
  const sparse = prepareSparseReorderGrab(view, Array.from(heldRoots), held, press);
  const preferred = sparse !== null && atomicSparseReorderGrab(sparse);
  const nodes = preferred ? null : admitNativeReorderPaint(lines);
  if (preferred || !nodes) {
    return sparse
      ? withReorderPaintMaterialization(
          view.dom.ownerDocument,
          (materialization) =>
            materializeReorderGrab(
              view,
              materialization,
              sparse.geometry,
              sparse.prepared,
              sparse.copies,
              sparse.clips,
            ),
          (paint) => paint?.node.remove(),
        )
      : null;
  }
  const rects: ReorderRect[] = [];
  const metrics = new Map<
    HTMLElement,
    {
      readonly rect: DOMRect;
      readonly width: number;
      readonly height: number;
      readonly scaleX: number;
      readonly scaleY: number;
    }
  >();
  const clips: {
    readonly rect: ReorderRect;
    readonly outlineX: number;
    readonly outlineY: number;
  }[] = [];
  for (const heldNode of heldRoots) {
    const line = heldNode.closest<HTMLElement>('.cm-line');
    if (!line || !view.contentDOM.contains(line)) return null;
    let measured = metrics.get(line);
    if (!measured) {
      const size = nativeReorderLineSize(view, line),
        rect = line.getBoundingClientRect();
      if (!size) return null;
      const scaleX = rect.width / size.width,
        scaleY = rect.height / size.height;
      if (![scaleX, scaleY].every((scale) => Number.isFinite(scale) && scale > 0)) return null;
      measured = { rect, ...size, scaleX, scaleY };
      metrics.set(line, measured);
    }
    const style = requireValue(
      view.dom.ownerDocument.defaultView,
      'reorder-grab-paint',
    ).getComputedStyle(heldNode);
    const width = Number.parseFloat(style.outlineWidth),
      offset = Number.parseFloat(style.outlineOffset);
    const outline =
      style.outlineStyle !== 'none' && Number.isFinite(width)
        ? Math.max(0, width + (Number.isFinite(offset) ? offset : 0))
        : 0;
    const fragments = nativeReorderFootprint(heldNode, nodes, held);
    if (!fragments || fragments.length + rects.length > 4096) return null;
    for (const rect of fragments) {
      rects.push(rect);
      clips.push({
        rect,
        outlineX: outline * measured.scaleX,
        outlineY: outline * measured.scaleY,
      });
    }
  }
  const geometry = reorderGrabGeometry(rects, held, press);
  if (!geometry) return null;
  const prepared = prepareNativeReorderPaint(view, lines, nodes, 'copy');
  if (!prepared) return null;
  return withReorderPaintMaterialization(
    view.dom.ownerDocument,
    (materialization) =>
      materializeReorderGrab(
        view,
        materialization,
        geometry,
        prepared,
        lines.map((node) => ({
          node,
          kind: 'line' as const,
          ...requireValue(metrics.get(node), 'reorder-grab-paint'),
        })),
        clips,
      ),
    (paint) => paint?.node.remove(),
  );
}
