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
/** Cold native DOM observation supplies physical line/caret evidence; target resolution never repeats this traversal. */
import { requireValue } from '../../core/src/required.js';
import { admitReorderTargetLayout } from './reorder-layout-admission.js';
import type { MountedReorderTarget } from './reorder-mounted-target.js';
import type { ReorderRect } from './reorder-geometry.js';
import type { ReorderPhysicalFragment } from './reorder-placement-topology.js';
/** Text offsets are node-relative UTF-16; element offsets count children. Native caret coordinates use viewport CSS pixels. */
interface Caret {
  readonly x: number;
  readonly top: number;
  readonly bottom: number;
}
/** Actual parent-line and zero-flow text identities guard warmed geometry without layout reads. */
export function reorderPhysicalTargetNodes(
  targets: readonly MountedReorderTarget[],
): readonly Node[] | null {
  if (targets.length > 8192) return null;
  const nodes: Node[] = [];
  for (const target of targets) {
    if (!target.node.isConnected) return null;
    const parentLine = target.node.closest('.cm-line');
    if (!parentLine?.isConnected) return null;
    nodes.push(target.node, parentLine);
    if (target.kind === 'chip') continue;
    const parent = target.node.parentNode,
      caret = parent?.childNodes[0];
    if (!caret || caret.nodeType !== 3 || !caret.isConnected || caret.parentNode !== parent)
      return null;
    nodes.push(caret);
  }
  return nodes;
}
/** A collapsed native Range observes the browser's actual insertion anchor, including bidi boundary affinity. */
function carets(range: Range, node: Node, offset: number): readonly Caret[] | null {
  range.setStart(node, offset);
  range.collapse(true);
  const rectangles = range.getClientRects();
  if (rectangles.length > 8192) return null;
  return Array.from(rectangles)
    .filter(
      (rect) => [rect.left, rect.top, rect.bottom].every(Number.isFinite) && rect.bottom > rect.top,
    )
    .map((rect) => ({ x: rect.left, top: rect.top, bottom: rect.bottom }));
}
/** Native font-box membership associates actual caret readings with one wrapped inline fragment. */
function belongs(
  caret: Caret,
  rect: {
    readonly top: number;
    readonly bottom: number;
  },
): boolean {
  const center = (caret.top + caret.bottom) / 2;
  return center >= rect.top && center <= rect.bottom;
}
/** Read exact native endpoints and full rich paint extents only after finite node/text admission. */
export function readReorderPhysicalFragments(
  targets: readonly MountedReorderTarget[],
  rects: readonly ReorderRect[],
): readonly ReorderPhysicalFragment[] | null {
  if (rects.length > 8192) return null;
  if (!reorderPhysicalTargetNodes(targets)) return null;
  const nodes = admitReorderTargetLayout(targets);
  if (!nodes) return null;
  const fragments: ReorderPhysicalFragment[] = [];
  const lines = new Map<Element, Map<string, ReorderPhysicalFragment['line']>>();
  let lineCount = 0;
  const byTarget = new Map<string, ReorderRect[]>();
  /** Numeric canonical target identity indexes each measured fragment once, independently of target count. */
  const identity = (target: {
    readonly kind: ReorderRect['kind'];
    readonly index: number;
    readonly blank?: number;
  }): string => `${target.kind}:${target.index}:${target.blank ?? ''}`;
  for (const rect of rects) {
    const key = identity(rect),
      group = byTarget.get(key) ?? [];
    group.push(rect);
    byTarget.set(key, group);
  }
  let measurements = 0,
    associations = 0;
  for (const target of targets) {
    if (!target.node.isConnected) return null;
    const parentLine = target.node.closest('.cm-line');
    if (!parentLine?.isConnected) return null;
    const physical = byTarget.get(identity(target)) ?? [];
    if (!physical.length) return null;
    const range = target.node.ownerDocument.createRange();
    const anchors: Caret[] = [];
    /** The whole cold read shares one observation budget across caret and paint rectangles. */
    const observeCaret = (node: Node, offset: number): readonly Caret[] | null => {
      const observed = carets(range, node, offset);
      if (!observed || measurements + Math.max(1, observed.length) > 32768) return null;
      measurements += Math.max(1, observed.length);
      return observed;
    };
    if (target.kind === 'chip') {
      const first = observeCaret(target.node, 0);
      if (!first) return null;
      anchors.push(...first);
      for (const node of requireValue(nodes.get(target.node), 'reorder-placement-measurement')) {
        if (node.nodeType !== 3) continue;
        const text = node as Text;
        let offset = 0;
        const first = observeCaret(text, offset);
        if (!first) return null;
        anchors.push(...first);
        for (const scalar of text.data) {
          offset += scalar.length;
          const next = observeCaret(text, offset);
          if (!next) return null;
          anchors.push(...next);
        }
      }
      const last = observeCaret(target.node, target.node.childNodes.length);
      if (!last) return null;
      anchors.push(...last);
    } else {
      const empty = observeCaret(
        requireValue(
          requireValue(target.node.parentNode, 'reorder-placement-measurement').childNodes[0],
          'reorder-placement-measurement',
        ),
        0,
      );
      if (!empty) return null;
      anchors.push(...empty);
    }
    const extents: {
      readonly box: DOMRect;
      readonly anchor: readonly Caret[];
    }[] = [];
    for (const node of requireValue(nodes.get(target.node), 'reorder-placement-measurement')) {
      if (node.nodeType !== 1 || node === target.node) continue;
      const element = node as HTMLElement;
      const boxes = element.getClientRects();
      if (boxes.length + measurements > 32768) return null;
      measurements += boxes.length;
      const parent = element.parentNode;
      if (!parent) return null;
      let at = -1;
      for (let index = 0; index < parent.childNodes.length; index++) {
        if (associations >= 32768) return null;
        associations++;
        if (parent.childNodes[index] === element) {
          at = index;
          break;
        }
      }
      if (at < 0) return null;
      const before = observeCaret(parent, at),
        after = observeCaret(parent, at + 1);
      if (!before || !after) return null;
      const anchor = [...before, ...after];
      for (const box of boxes) if (box.width > 0 && box.height > 0) extents.push({ box, anchor });
    }
    for (const rect of physical) {
      const actual: Caret[] = [];
      for (const anchor of anchors) {
        if (associations >= 32768) return null;
        associations++;
        if (target.kind !== 'chip' || belongs(anchor, rect)) actual.push(anchor);
      }
      if (!actual.length) return null;
      /** Ambient inline font boxes own membership; absolute markers and rich overflowing paint own only hit extents. */
      const fontLine =
          target.kind === 'chip' ? rect : requireValue(actual[0], 'reorder-placement-measurement'),
        lineKey = `${fontLine.top}:${fontLine.bottom}`,
        ownerLines = lines.get(parentLine) ?? new Map<string, ReorderPhysicalFragment['line']>();
      lines.set(parentLine, ownerLines);
      let line = ownerLines.get(lineKey);
      if (!line) {
        line = Object.freeze({
          top: fontLine.top,
          bottom: fontLine.bottom,
          identity: lineCount++,
        });
        ownerLines.set(lineKey, line);
      }
      let top = rect.top,
        bottom = rect.bottom,
        left = rect.left,
        right = rect.right;
      for (const extent of extents) {
        if (associations >= 32768) return null;
        associations++;
        let matched = false;
        for (const anchor of extent.anchor) {
          if (associations >= 32768) return null;
          associations++;
          if (!belongs(anchor, rect)) continue;
          if (associations >= 32768) return null;
          associations++;
          if (belongs(anchor, extent.box)) {
            matched = true;
            break;
          }
        }
        if (!matched) continue;
        top = Math.min(top, extent.box.top);
        bottom = Math.max(bottom, extent.box.bottom);
        left = Math.min(left, extent.box.left);
        right = Math.max(right, extent.box.right);
      }
      fragments.push(
        Object.freeze({
          rect: Object.freeze({ ...rect, left, right, top, bottom }),
          line,
          leading: requireValue(actual[0], 'reorder-placement-measurement').x,
          trailing: requireValue(actual.at(-1), 'reorder-placement-measurement').x,
        }),
      );
    }
  }
  return Object.freeze(fragments);
}
