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
/** Expected native bitmap/context unavailability rejects the entire pre-hide snapshot. */
import { requireValue } from '../../core/src/required.js';
import { nativeReorderRecipe, nativeReorderElement } from './reorder-native-recipe.js';
import { readReorderPaintSizeWitness } from './reorder-paint-style.js';
import { NativeReorderAxes } from './reorder-native-axes.js';
import { nativeReorderFootprint } from './reorder-native-footprint.js';
import { currentReorderProjectionTargets } from './reorder-paint-projection.js';
import { reorderPreview } from './reorder-preview.js';
import { admitReorderLineLayout, admitReorderTargetLayout } from './reorder-layout-admission.js';
import { ReorderPaintMaterialization } from './reorder-paint-materialization.js';
import type { EditorView } from '@codemirror/view';
import {
  nativeReorderWidgetBuffer,
  type PreparedReorderPaint,
} from './reorder-paint-preparation.js';
class PaintUnavailable extends Error {}
/** An untransferred native node remains owned through failure; cleanup never replaces the original cause. */
function discardFailedPaint(node: ChildNode, cause: unknown): never {
  try {
    node.remove();
  } catch (cleanup) {
    throw new AggregateError([cause, cleanup], 'Native paint cleanup failed');
  }
  throw cause;
}
/** Admit complete decoded intrinsic pixels under a whole-snapshot budget before any canvas allocation. */
function mediaBudget(nodes: readonly Node[]): boolean {
  let pixels = 0;
  for (const node of nodes) {
    if (node.nodeType !== 1) continue;
    const element = node as HTMLElement;
    if (
      nativeReorderWidgetBuffer(element) ||
      (element.localName !== 'canvas' && element.localName !== 'img')
    )
      continue;
    const image = element as HTMLImageElement,
      canvas = element as HTMLCanvasElement;
    const width = element.localName === 'img' ? image.naturalWidth : canvas.width;
    const height = element.localName === 'img' ? image.naturalHeight : canvas.height;
    if (
      (element.localName === 'img' && !image.complete) ||
      !Number.isSafeInteger(width) ||
      !Number.isSafeInteger(height) ||
      width <= 0 ||
      height <= 0 ||
      width * height > 1048576
    )
      return false;
    pixels += width * height;
    if (pixels > 4194304) return false;
  }
  return true;
}
/** Native border-box dimensions use the same positive ancestor-axis authority as prepared descendant widths. */
export function nativeReorderLineSize(
  view: EditorView,
  line: HTMLElement,
): {
  readonly width: number;
  readonly height: number;
} | null {
  const state = view.state,
    axes = new NativeReorderAxes(
      requireValue(view.dom.ownerDocument.defaultView, 'reorder-native-paint'),
      () => view.state === state,
    ),
    style = requireValue(
      view.dom.ownerDocument.defaultView,
      'reorder-native-paint',
    ).getComputedStyle(line),
    scale = view.state === state ? axes.read(line, style) : null;
  if (!scale) return null;
  const witnessWidth = readReorderPaintSizeWitness(style, 'width', () => axes.currentNode(line)),
    witnessHeight = readReorderPaintSizeWitness(style, 'height', () => axes.currentNode(line));
  if (witnessWidth === null || witnessHeight === null) return null;
  const rect = line.getBoundingClientRect();
  if (!axes.current()) return null;
  const physicalWidth = rect.width;
  if (!axes.current()) return null;
  const physicalHeight = rect.height;
  if (!axes.current()) return null;
  const width = physicalWidth / scale.x,
    height = physicalHeight / scale.y;
  return axes.current() &&
    Math.abs(physicalWidth - witnessWidth * scale.x) <= 1 / 16 &&
    Math.abs(physicalHeight - witnessHeight * scale.y) <= 1 / 16 &&
    Number.isFinite(width) &&
    Number.isFinite(height) &&
    width > 0 &&
    height > 0
    ? { width, height }
    : null;
}
/** Bound cold press work before creating any copied paint; source and input owners never travel with it. */
function boundedNodes(lines: readonly HTMLElement[]): readonly Node[] | null {
  const nodes: Node[] = [],
    pending: Node[] = [...lines];
  let characters = 0;
  while (pending.length) {
    const node = requireValue(pending.pop(), 'reorder-native-paint');
    nodes.push(node);
    if (node.nodeType === 3) {
      const length = (node as CharacterData).length;
      if (!Number.isSafeInteger(length) || length < 0) return null;
      characters += length;
    }
    if (nodes.length > 1024 || characters > 16384) return null;
    if (node.childNodes.length + pending.length + nodes.length > 1024) return null;
    for (const child of node.childNodes) pending.push(child);
  }
  return nodes;
}
/** Materialize only frozen derived data; original identities are keys, never live DOM/style read operands. */
function paintNode(
  view: EditorView,
  original: Node,
  prepared: PreparedReorderPaint,
  materialization: ReorderPaintMaterialization,
  copies?: WeakMap<Node, Node>,
): Node {
  const descriptor = prepared.get(original);
  if (!descriptor) throw new Error('reorder-prepared-paint-node-missing');
  const doc = view.dom.ownerDocument;
  if (descriptor.kind === 'text') {
    const copy = doc.createTextNode(descriptor.text);
    try {
      copies?.set(original, copy);
      return copy;
    } catch (cause) {
      discardFailedPaint(copy, cause);
    }
  }
  const copy = materialization.create(descriptor);
  try {
    copies?.set(original, copy);
    if (descriptor.vector) copy.setAttribute('class', descriptor.className);
    else copy.className = descriptor.className;
    for (const [name, value] of descriptor.attributes) copy.setAttribute(name, value);
    if (descriptor.readonly) copy.contentEditable = 'false';
    if (descriptor.buffer) {
      (copy as HTMLCanvasElement).width = 0;
      (copy as HTMLCanvasElement).height = 0;
    } else if (descriptor.media) {
      const canvas = copy as HTMLCanvasElement;
      canvas.width = descriptor.media.width;
      canvas.height = descriptor.media.height;
      const context = canvas.getContext('2d');
      if (!context) throw new PaintUnavailable('reorder-paint-context-unavailable');
      context.drawImage(descriptor.media.source, 0, 0);
    } else
      for (const child of descriptor.children)
        copy.append(paintNode(view, child, prepared, materialization, copies));
    return copy;
  } catch (cause) {
    discardFailedPaint(copy, cause);
  }
}
/** One cold batch admits all native line nodes/media before any copied paint is allocated. */
export function admitNativeReorderPaint(lines: readonly HTMLElement[]): readonly Node[] | null {
  const nodes = boundedNodes(lines);
  return nodes && mediaBudget(nodes) ? nodes : null;
}
/** Copy only admitted inert native paint; absence of decoded/context pixels is an explicit complete-snapshot fallback. */
export function copyNativeReorderPaint(
  view: EditorView,
  original: Node,
  prepared: PreparedReorderPaint,
  materialization: ReorderPaintMaterialization,
  copies?: WeakMap<Node, Node>,
): Node | null {
  try {
    return paintNode(view, original, prepared, materialization, copies);
  } catch (cause) {
    if (cause instanceof PaintUnavailable) return null;
    throw cause;
  }
}
/** READ observes complete native rich groups within layout/4096-fragment bounds independently of the animated-group budget. */
export function nativeReorderViewportFootprint(
  view: EditorView,
  held?: number,
): readonly import('./reorder-geometry.js').ReorderRect[] | null {
  const targets = currentReorderProjectionTargets(view);
  let byRoot: ReadonlyMap<HTMLElement, readonly Node[]>;
  if (targets) {
    const selected =
      held === undefined
        ? targets
        : targets.filter((target) => target.kind === 'chip' && target.index === held);
    const admitted = admitReorderTargetLayout(selected);
    if (!admitted) return null;
    byRoot = new Map(
      selected
        .filter((target) => target.kind === 'chip')
        .map((target) => [
          target.node,
          requireValue(admitted.get(target.node), 'reorder-native-paint'),
        ]),
    );
  } else {
    // Initial outer decorations retain native lines/widgets before the first changed-order replacement exists.
    const publication = view.state.field(reorderPreview, false),
      preview = publication?.paint;
    if (!publication?.native || !preview) return null;
    const recipe = nativeReorderRecipe(
      preview.capture.context.values,
      preview.capture.target.entry,
    );
    if (!recipe) return null;
    const lines = Array.from(view.contentDOM.querySelectorAll<HTMLElement>('.cm-line'));
    if (!lines.length || lines.length > 16) return null;
    const selectedRoots =
      held === undefined
        ? null
        : Array.from(
            view.contentDOM.querySelectorAll<HTMLElement>(
              `[data-reorder-native-value="${recipe.key}"][data-reorder-chip="${held}"]`,
            ),
          );
    const nodes = admitReorderLineLayout(selectedRoots ?? lines);
    if (!nodes) return null;
    const nativeRoots = new Map<HTMLElement, readonly Node[]>();
    for (const node of nodes) {
      if (
        node.nodeType !== 1 ||
        !(node as HTMLElement).hasAttribute('data-reorder-chip') ||
        !nativeReorderElement(node as HTMLElement, recipe.key)
      )
        continue;
      const root = node as HTMLElement,
        descendants = admitReorderLineLayout([root]);
      if (!descendants) return null;
      nativeRoots.set(root, descendants);
    }
    byRoot = nativeRoots;
  }
  const viewport = view.scrollDOM.getBoundingClientRect(),
    groups = new Map<number, readonly import('./reorder-geometry.js').ReorderRect[]>(),
    incomplete = new Set<number>();
  let count = 0;
  for (const [root, nodes] of byRoot) {
    const id = Number(root.dataset.reorderChip);
    if (!Number.isSafeInteger(id) || id < 0 || (held !== undefined && id !== held)) continue;
    const fragments = nativeReorderFootprint(
      root,
      nodes,
      id,
      held === undefined ? viewport : undefined,
    );
    if (
      !fragments ||
      !fragments.length ||
      (held === undefined &&
        fragments.some(
          (rect) =>
            rect.left < viewport.left ||
            rect.right > viewport.right ||
            rect.top < viewport.top ||
            rect.bottom > viewport.bottom,
        ))
    ) {
      incomplete.add(id);
      continue;
    }
    count += fragments.length;
    if (count > 4096) return null;
    groups.set(id, [...(groups.get(id) ?? []), ...fragments]);
  }
  for (const id of incomplete) groups.delete(id);
  return Object.freeze(Array.from(groups.values()).flat());
}
