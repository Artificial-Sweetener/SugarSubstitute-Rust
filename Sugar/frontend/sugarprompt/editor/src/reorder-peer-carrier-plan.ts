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
/** Native READ prepares one aggregate complete atomic peer-copy recipe; it owns no publication or retained lifecycle state. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderMotionScene } from './reorder-motion-scene.js';
import type { ReorderGrabCopy, ReorderGrabClip } from './reorder-grab-materialization.js';
import type { ReorderGrabGeometry } from './reorder-grab-geometry.js';
import { reorderGrabGeometry } from './reorder-grab-geometry.js';
import { atomicReorderPaintIsland } from './reorder-atomic-paint-island.js';
import { admitNativeReorderPaint } from './reorder-native-paint.js';
import {
  prepareNativeReorderPaint,
  preparedAtomicPaintRole,
  type PreparedReorderPaint,
} from './reorder-paint-preparation.js';
/** Immutable canonical endpoint metadata remains the sole semantic and geometry roster. */
export type ReorderCarrierSource = ReorderMotionScene['chips'][number];
/** One complete admitted root retains exact native scale, chrome clip and source identity. */
export interface ReorderPeerCopyPlan {
  readonly source: ReorderCarrierSource;
  readonly root: HTMLElement;
  readonly geometry: ReorderGrabGeometry;
  readonly copy: ReorderGrabCopy;
  readonly clip: ReorderGrabClip;
}
/** Explicit refusal keeps existing native offsets; stale publication never becomes an empty successful recipe. */
export type ReorderPeerCarrierPreparation =
  | {
      readonly kind: 'refused';
    }
  | {
      readonly kind: 'unavailable';
    }
  | {
      readonly kind: 'ready';
      readonly prepared: PreparedReorderPaint;
      readonly peers: readonly ReorderPeerCopyPlan[];
      readonly placement: {
        readonly left: number;
        readonly top: number;
        readonly scaleX: number;
        readonly scaleY: number;
      };
    };
/** Capture one union under unchanged native copy budgets and current request-local mutation authority. */
export function prepareReorderPeerCarriers(
  view: EditorView,
  parent: HTMLElement,
  chips: readonly ReorderCarrierSource[],
  scaleX: number,
  scaleY: number,
  current: () => boolean,
): ReorderPeerCarrierPreparation {
  const refused = (): ReorderPeerCarrierPreparation =>
    current() ? { kind: 'refused' } : { kind: 'unavailable' };
  if (
    !current() ||
    chips.length > 64 ||
    ![scaleX, scaleY].every((scale) => Number.isFinite(scale) && scale > 0)
  )
    return refused();
  const sources: ReorderCarrierSource[] = [];
  for (const chip of chips) {
    if (chip.roots.length !== 1) continue;
    const children = requireValue(chip.roots[0], 'reorder-peer-carrier-plan').node.childNodes;
    if (!current()) return { kind: 'unavailable' };
    const count = children.length;
    if (!current() || !Number.isSafeInteger(count) || count < 0 || count > 1024) return refused();
    let outsideText = false;
    for (let index = 0; index < count; index++) {
      const child = requireValue(children[index], 'reorder-peer-carrier-plan');
      if (!current()) return { kind: 'unavailable' };
      const kind = child.nodeType;
      if (!current()) return { kind: 'unavailable' };
      if (kind !== 3) continue;
      const length = (child as Text).length;
      if (!current()) return { kind: 'unavailable' };
      if (length > 0) {
        outsideText = true;
        break;
      }
    }
    // Any direct nonempty text is outside an independent child island and already refuses the shared complete predicate.
    if (!outsideText) sources.push(chip);
  }
  const roots = sources.map(
      (chip) => requireValue(chip.roots[0], 'reorder-peer-carrier-plan').node,
    ),
    admitted = roots.length ? admitNativeReorderPaint(roots) : null;
  if (!admitted || !current()) return refused();
  const prepared = prepareNativeReorderPaint(view, roots, admitted, 'atomic-peers');
  if (!prepared || !current()) return refused();
  const origin = parent.getBoundingClientRect();
  if (!current()) return refused();
  const left = origin.left;
  if (!current()) return refused();
  const top = origin.top;
  if (!current() || ![left, top].every(Number.isFinite)) return refused();
  const peers: ReorderPeerCopyPlan[] = [];
  for (const source of sources) {
    const root = requireValue(source.roots[0], 'reorder-peer-carrier-plan').node;
    if (!atomicReorderPaintIsland(root, (node) => preparedAtomicPaintRole(prepared.get(node))))
      continue;
    if (!current()) return refused();
    const native = root.getClientRects();
    if (!current()) return refused();
    const count = native.length;
    if (!current() || !Number.isSafeInteger(count) || count < 0 || count > 256) return refused();
    let fragment: {
      left: number;
      right: number;
      top: number;
      bottom: number;
      width: number;
      height: number;
    } | null = null;
    for (let index = 0; index < count; index++) {
      const part = requireValue(native[index], 'reorder-peer-carrier-plan');
      if (!current()) return refused();
      const copy = {
        left: part.left,
        right: part.right,
        top: part.top,
        bottom: part.bottom,
        width: part.width,
        height: part.height,
      };
      if (!current() || !Object.values(copy).every(Number.isFinite)) return refused();
      if (copy.width <= 0 || copy.height <= 0) continue;
      if (fragment !== null) {
        fragment = null;
        break;
      }
      fragment = copy;
    }
    if (
      !fragment ||
      source.rects.some(
        (rect) =>
          rect.left < requireValue(fragment, 'reorder-peer-carrier-plan').left ||
          rect.right > requireValue(fragment, 'reorder-peer-carrier-plan').right ||
          rect.top < requireValue(fragment, 'reorder-peer-carrier-plan').top ||
          rect.bottom > requireValue(fragment, 'reorder-peer-carrier-plan').bottom,
      )
    )
      continue;
    const geometry = reorderGrabGeometry(source.rects, source.id, {
      x: fragment.left,
      y: fragment.top,
    });
    if (!geometry || !current()) continue;
    const window = view.dom.ownerDocument.defaultView;
    if (!window || !current()) return refused();
    const style = window.getComputedStyle(root);
    if (!current()) return refused();
    const width = Number.parseFloat(style.outlineWidth);
    if (!current()) return refused();
    const offset = Number.parseFloat(style.outlineOffset);
    if (!current()) return refused();
    const outlineStyle = style.outlineStyle;
    if (!current()) return refused();
    const outline =
      outlineStyle !== 'none' && Number.isFinite(width)
        ? Math.max(0, width + (Number.isFinite(offset) ? offset : 0))
        : 0;
    peers.push({
      source,
      root,
      geometry,
      copy: Object.freeze({
        node: root,
        kind: 'fragment',
        rect: Object.freeze({ left: fragment.left, top: fragment.top }),
        width: fragment.width / scaleX,
        height: fragment.height / scaleY,
        scaleX,
        scaleY,
      }),
      clip: Object.freeze({
        rect: Object.freeze({
          left: fragment.left,
          right: fragment.right,
          top: fragment.top,
          bottom: fragment.bottom,
        }),
        outlineX: outline * scaleX,
        outlineY: outline * scaleY,
      }),
    });
  }
  return current() && peers.length
    ? {
        kind: 'ready',
        prepared,
        peers: Object.freeze(peers),
        placement: Object.freeze({ left, top, scaleX, scaleY }),
      }
    : refused();
}
