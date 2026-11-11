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
/** Zero-width canonical row-edge resources borrow browser line boxes without estimating font height or retaining another layout map. */
import type { EditorView } from '@codemirror/view';
import {
  sameCanonicalRowPaint,
  type CanonicalRowPaint,
  type CanonicalRowPaintPlane,
} from './canonical-row-paint.js';
/** Measured viewport CSS pixels borrow the original native containing-line axis only when a regional role needs it. */
export interface CanonicalPaintRow {
  readonly top: number;
  readonly bottom: number;
  readonly left: number | null;
  readonly paint: CanonicalRowPaint;
}
/** The renderer already owns this root and retires its edge pair with the same readonly fragment lease. */
interface CanonicalRowEdges {
  readonly view: EditorView;
  readonly top: HTMLElement;
  readonly bottom: HTMLElement;
  readonly paint: CanonicalRowPaint;
}
const mounted = new WeakMap<HTMLElement, CanonicalRowEdges>();
const views = new WeakMap<EditorView, Set<HTMLElement>>();
/** Remove ownership before independent DOM retirement; raw ordered causes survive partial native failure. */
export function releaseCanonicalRowEdges(root: HTMLElement): void {
  const edges = mounted.get(root);
  if (edges) retireCanonicalRowEdges(root, edges);
}
/** Revoke only this exact lease, but always remove its captured pair: a delegated native insertion may finish after a replacement acquired the same root. */
function retireCanonicalRowEdges(root: HTMLElement, edges: CanonicalRowEdges): void {
  if (mounted.get(root) === edges) {
    mounted.delete(root);
    const roots = views.get(edges.view);
    roots?.delete(root);
    if (roots?.size === 0) views.delete(edges.view);
  }
  const failures: unknown[] = [];
  for (const node of [edges.top, edges.bottom])
    try {
      node.remove();
    } catch (cause) {
      failures.push(cause);
    }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'Canonical row edge retirement failed');
}
/** Add no text, width or height; top/bottom alignment asks the browser for its actual containing line-box edges. */
export function mountCanonicalRowEdges(
  view: EditorView,
  root: HTMLElement,
  paint: CanonicalRowPaint,
  current: () => boolean,
): boolean {
  if (mounted.has(root)) throw new Error('canonical-row-edge-duplicate');
  const top = root.ownerDocument.createElement('span'),
    bottom = root.ownerDocument.createElement('span');
  top.className = 'sp-canonical-row-edge sp-canonical-row-top';
  bottom.className = 'sp-canonical-row-edge sp-canonical-row-bottom';
  top.setAttribute('aria-hidden', 'true');
  bottom.setAttribute('aria-hidden', 'true');
  if (!current()) return false;
  const roots = views.get(view) ?? new Set<HTMLElement>();
  views.set(view, roots);
  roots.add(root);
  const edges = { view, top, bottom, paint };
  mounted.set(root, edges);
  const failures: unknown[] = [];
  let admitted = false;
  try {
    root.prepend(top);
    if (current() && mounted.get(root) === edges) {
      root.append(bottom);
      admitted = current() && mounted.get(root) === edges;
    }
  } catch (cause) {
    failures.push(cause);
  }
  if (!admitted)
    try {
      retireCanonicalRowEdges(root, edges);
    } catch (cause) {
      failures.push(cause);
    }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'Canonical row edge acquisition failed');
  return admitted;
}
/** SourceLines READ filters prepared roles before native access; null observes all roles, while passive/collision copies remain excluded. */
export function readCanonicalRowEdges(
  view: EditorView,
  current: () => boolean,
  plane: CanonicalRowPaintPlane | null = null,
): readonly CanonicalPaintRow[] | null {
  const roots = views.get(view),
    rows: CanonicalPaintRow[] = [],
    axes = new Map<HTMLElement, number>();
  if (!roots) return rows;
  for (const root of Array.from(roots)) {
    if (!current()) return null;
    const edges = mounted.get(root);
    if (!edges || edges.view !== view) continue;
    if (
      (plane === 'scene' && !edges.paint.sceneFill) ||
      (plane === 'region' && !edges.paint.region)
    )
      continue;
    const contained = view.contentDOM.contains(root);
    if (!current() || mounted.get(root) !== edges) return null;
    if (!contained) continue;
    const top = edges.top.getBoundingClientRect().top;
    if (!current() || mounted.get(root) !== edges) return null;
    const bottom = edges.bottom.getBoundingClientRect().bottom;
    if (!current() || mounted.get(root) !== edges) return null;
    if (!Number.isFinite(top) || !Number.isFinite(bottom) || bottom < top)
      throw new Error('canonical-row-edge-geometry');
    let left: number | null = null;
    if (edges.paint.region && plane !== 'scene') {
      const line = root.closest<HTMLElement>('.cm-line');
      if (!current() || mounted.get(root) !== edges) return null;
      if (!line) throw new Error('canonical-row-native-line-unavailable');
      left = axes.get(line) ?? null;
      if (left === null) {
        left = line.getBoundingClientRect().left;
        if (!current() || mounted.get(root) !== edges) return null;
        if (!Number.isFinite(left)) throw new Error('canonical-row-native-axis');
        axes.set(line, left);
      }
    }
    if (bottom > top) rows.push({ top, bottom, left, paint: edges.paint });
  }
  rows.sort((a, b) => a.top - b.top || a.bottom - b.bottom);
  const merged: {
    top: number;
    bottom: number;
    left: number | null;
    paint: CanonicalRowPaint;
  }[] = [];
  for (const row of rows) {
    const last = merged.at(-1);
    if (
      last &&
      row.top < last.bottom &&
      row.left === last.left &&
      sameCanonicalRowPaint(row.paint, last.paint)
    )
      last.bottom = Math.max(last.bottom, row.bottom);
    else merged.push({ ...row });
  }
  return Object.freeze(merged.map((row) => Object.freeze(row)));
}
