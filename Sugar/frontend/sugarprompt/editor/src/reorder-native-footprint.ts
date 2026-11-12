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
/** Complete native rich fragments share one geometry observer without importing paint materialization or publication owners. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderRect, ReorderMarkerRect } from './reorder-geometry.js';
/** Native numeric operands are copied before they can participate in callback-free reduction. */
export type NativeReorderFragment = Pick<
  DOMRectReadOnly,
  'left' | 'right' | 'top' | 'bottom' | 'width' | 'height'
>;
/** The sole numeric reducer admits positive fragments without consulting native nodes or external callbacks. */
function appendNumericFragment(
  rects: ReorderRect[],
  id: number,
  left: number,
  right: number,
  top: number,
  bottom: number,
  width: number,
  height: number,
  completeViewport?: ReorderMarkerRect,
): boolean {
  if (width <= 0 || height <= 0) return true;
  if (![left, right, top, bottom].every(Number.isFinite)) return false;
  if (
    completeViewport &&
    (left < completeViewport.left ||
      right > completeViewport.right ||
      top < completeViewport.top ||
      bottom > completeViewport.bottom)
  )
    return false;
  rects.push(Object.freeze({ kind: 'chip', index: id, left, right, top, bottom }));
  return true;
}
/** Module-private construction authority prevents reflective callers from issuing samples over arbitrary arrays. */
const rootSampleAuthority = Symbol('reorder-root-sample');
/** Only this native-copy factory can issue the nominal, privately held immutable root sample; it owns no cross-request cache. */
export class NativeReorderRootSample {
  readonly #fragments: readonly NativeReorderFragment[];
  /** Construction follows successful guarded native extraction, never an arbitrary supplied numeric array. */
  private constructor(
    authority: typeof rootSampleAuthority,
    fragments: readonly NativeReorderFragment[],
  ) {
    if (authority !== rootSampleAuthority)
      throw new Error('reorder-root-sample-authority-required');
    this.#fragments = Object.freeze(fragments);
    Object.freeze(this);
  }
  /** Own the native list count and every before/index/field-copy guard under the original mounted target limit. */
  static capture(
    native: DOMRectList,
    current: () => boolean,
    maximum: 4096 | null,
  ): NativeReorderRootSample | null {
    if (!current()) return null;
    const count = native.length;
    if (
      !current() ||
      !Number.isSafeInteger(count) ||
      count < 0 ||
      (maximum !== null && count > maximum)
    )
      return null;
    const fragments: NativeReorderFragment[] = [];
    for (let index = 0; index < count; index++) {
      if (!current()) return null;
      const rect = requireValue(native[index], 'reorder-native-footprint');
      if (!current()) return null;
      const { left, right, top, bottom, width, height } = rect;
      fragments.push(Object.freeze({ left, right, top, bottom, width, height }));
      if (!current()) return null;
    }
    return new NativeReorderRootSample(rootSampleAuthority, fragments);
  }
  /** The actual immutable copied operands also supply the existing target layout arithmetic and validation. */
  get fragments(): readonly NativeReorderFragment[] {
    return this.#fragments;
  }
  /** Reduce only private plain-number records under the caller's enclosing pre/post freshness fence. */
  static append(sample: NativeReorderRootSample, rects: ReorderRect[], id: number): boolean {
    if (sample.#fragments.length + rects.length > 4096) return false;
    for (const { left, right, top, bottom, width, height } of sample.#fragments)
      if (!appendNumericFragment(rects, id, left, right, top, bottom, width, height)) return false;
    return true;
  }
}
/** The mounted measurement owner may supply its genuinely captured root under the same exact READ authority. */
export interface NativeReorderRootObservation {
  readonly sample?: NativeReorderRootSample;
  /** Recheck the exact request and mounted issue around every delegated native read. */
  current(): boolean;
}
/** Observe one actual native element fragment group within the existing whole-footprint budget. */
function appendNativeReorderFragments(
  element: HTMLElement,
  rects: ReorderRect[],
  id: number,
  current: () => boolean,
  completeViewport?: ReorderMarkerRect,
  observed?: NativeReorderRootSample,
): boolean {
  if (!current()) return false;
  if (observed && !completeViewport)
    return NativeReorderRootSample.append(observed, rects, id) && current();
  const fragments = observed?.fragments ?? element.getClientRects();
  if (!current()) return false;
  const count = fragments.length;
  if (!current() || count + rects.length > 4096) return false;
  for (const rect of fragments) {
    if (!current()) return false;
    const { left, right, top, bottom, width, height } = rect;
    if (!current()) return false;
    if (
      !appendNumericFragment(rects, id, left, right, top, bottom, width, height, completeViewport)
    )
      return false;
  }
  return current();
}
/** Measure actual viewport-CSS-pixel rich paint; complete-peer queries reject an outside root before descendant reads. */
export function nativeReorderFootprint(
  root: HTMLElement,
  nodes: readonly Node[],
  id: number,
  completeViewport?: ReorderMarkerRect,
  observation?: NativeReorderRootObservation,
): readonly ReorderRect[] | null {
  const rects: ReorderRect[] = [],
    current = observation?.current ?? (() => true);
  if (
    completeViewport &&
    !appendNativeReorderFragments(root, rects, id, current, completeViewport, observation?.sample)
  )
    return null;
  for (const node of nodes) {
    if (!current()) return null;
    if (node.nodeType !== 1) continue;
    const contained = root.contains(node);
    if (!current()) return null;
    if (!contained || (completeViewport && node === root)) continue;
    if (
      !appendNativeReorderFragments(
        node as HTMLElement,
        rects,
        id,
        current,
        undefined,
        node === root ? observation?.sample : undefined,
      )
    )
      return null;
  }
  return current() ? Object.freeze(rects) : null;
}
