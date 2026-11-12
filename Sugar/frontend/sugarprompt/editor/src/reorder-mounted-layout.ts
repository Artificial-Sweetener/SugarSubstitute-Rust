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
/** Native mounted-line measurement owns finite viewport layout reuse, independently of literal widget rendering. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderLayoutReadAuthority } from './reorder-slot-layout-read.js';
import {
  readReorderPhysicalFragments,
  reorderPhysicalTargetNodes,
} from './reorder-placement-measurement.js';
import type { ReorderPhysicalFragment } from './reorder-placement-topology.js';
import type { EditorView } from '@codemirror/view';
import {
  selectReorderMarkerTargets,
  type ReorderMarkerWitnesses,
  type ReorderRect,
} from './reorder-geometry.js';
import type {
  MountedReorderTarget,
  ReorderMountedObservation,
  ReorderMountedCoverage,
  ReorderCompleteMountedObservation,
} from './reorder-mounted-target.js';
import type { admitReorderTargetLayout } from './reorder-layout-admission.js';
import {
  nativeReorderFootprint,
  NativeReorderRootSample,
  type NativeReorderFragment,
} from './reorder-native-footprint.js';
/** Copy native numeric operands immediately; subsequent DOM callbacks cannot mutate a retained live rectangle. */
function copyNativeFragment(rect: NativeReorderFragment): NativeReorderFragment {
  const { left, right, top, bottom, width, height } = rect;
  return Object.freeze({ left, right, top, bottom, width, height });
}
/** Cached and fresh layout share chip/marker filtering and their original root-local arithmetic. */
function localTargetFragments(
  target: MountedReorderTarget,
  bounds: NativeReorderFragment,
  physical: Iterable<NativeReorderFragment>,
): ReorderRect[] {
  const rects: ReorderRect[] = [];
  for (const rect of physical)
    if (target.kind === 'chip' ? rect.width > 0 && rect.height > 0 : rect.width || rect.height)
      rects.push({
        kind: target.kind,
        index: target.index,
        ...(target.blank !== undefined ? { blank: target.blank } : {}),
        left: rect.left - bounds.left,
        right: rect.right - bounds.left,
        top: rect.top - bounds.top,
        bottom: rect.bottom - bounds.top,
      });
  return rects;
}
/** Preserve the existing local-to-viewport normalization for both measurement paths. */
function positionedTargetFragment(rect: ReorderRect, bounds: NativeReorderFragment): ReorderRect {
  return Object.freeze({
    ...rect,
    left: rect.left + bounds.left,
    right: rect.right + bounds.left,
    top: rect.top + bounds.top,
    bottom: rect.bottom + bounds.top,
  });
}
/** Immutable containing-line operands belong only to one native measurement invocation. */
interface ContainingLineObservation {
  readonly width: string;
  readonly paddingLeft: string;
  readonly paddingRight: string;
  readonly textIndent: string;
  readonly direction: string;
  readonly unicodeBidi: string;
  readonly left: number;
  readonly boundsWidth: number;
}
/** One mounted widget retains at most 64 actual layout outputs under measured native appearance and edge context. */
export class ReorderMountedLayout {
  private readonly view: EditorView;
  private readonly measured = new Map<string, readonly ReorderRect[]>();
  private disposed = false;
  private placement: {
    readonly rects: readonly ReorderRect[];
    readonly nodes: readonly Node[];
    readonly fragments: readonly ReorderPhysicalFragment[];
  } | null = null;
  /** Native view and currentness remain owned by the caller's one supported mounted widget lifetime. */
  constructor(view: EditorView) {
    this.view = view;
  }
  /** Marker paint observes only held/vacancy witnesses before native styles, bounds or layout-cache discovery. */
  marker(
    targets: readonly MountedReorderTarget[],
    witnesses: ReorderMarkerWitnesses,
  ): readonly ReorderRect[] | null {
    return this.measure(selectReorderMarkerTargets(targets, witnesses));
  }
  /** Every whole-paint READ observes complete root layout; only unpublished carrier preparation may omit provably ineligible rich groups. */
  observe(
    targets: readonly MountedReorderTarget[],
    authority: ReorderLayoutReadAuthority,
    readTargets: typeof admitReorderTargetLayout,
    coverage: ReorderMountedCoverage,
  ): ReorderMountedObservation | null {
    /** Native queue delegation may synchronously retire this exact layout after its predicate. */
    const checkpoint = (predicate: () => boolean): boolean =>
      authority.guard(predicate) && !this.disposed;
    /** The delegated predicate stays bare until this layout's complete checkpoint is composed. */
    const current = (): boolean => checkpoint(authority.available);
    if (this.disposed || !current()) return null;
    const retained = [...targets],
      nodes = readTargets(retained);
    if (this.disposed || !current() || !nodes || !this.view.dom.ownerDocument.defaultView)
      return null;
    const observed = new Map<
        MountedReorderTarget,
        {
          readonly bounds: NativeReorderFragment;
          readonly physical: NativeReorderRootSample;
        }
      >(),
      lines = new Map<HTMLElement, number>(),
      footprints = new Map<HTMLElement, readonly ReorderRect[]>();
    let fragments = 0;
    /** Exact request authority fences every delegated DOM read, including same-preview mounted issue replacement. */
    const owned = (): boolean => !this.disposed && authority.available() && !this.disposed;
    /** Native checkpoints share the original Slot fence instead of nesting observer drains. */
    const available = (): boolean => checkpoint(owned);
    /** Root BCR and physical fragments remain distinct native operands; neither is rediscovered for rich paint. */
    const readTarget = (target: MountedReorderTarget): boolean => {
      if (!available()) return false;
      const line = target.node.closest<HTMLElement>('.cm-line');
      if (!available() || !line || !target.node.isConnected || !available()) return false;
      let width = lines.get(line);
      if (width === undefined) {
        const bounds = line.getBoundingClientRect();
        if (!available()) return false;
        width = bounds.width;
        if (!available() || !Number.isFinite(width) || width <= 0) return false;
        lines.set(line, width);
      }
      const nativeBounds = target.node.getBoundingClientRect();
      if (!available()) return false;
      const bounds = copyNativeFragment(nativeBounds);
      if (!available() || !target.node.isConnected || !available()) return false;
      const native = target.node.getClientRects();
      if (!available()) return false;
      const physical = NativeReorderRootSample.capture(
        native,
        available,
        target.kind === 'chip' ? 4096 : null,
      );
      if (!physical) return false;
      if (!available() || !target.node.isConnected || !available()) return false;
      if (
        ![bounds.left, bounds.top, bounds.width, bounds.height].every(Number.isFinite) ||
        bounds.height <= 0 ||
        physical.fragments.some(
          (rect) =>
            ![rect.left, rect.right, rect.top, rect.bottom, rect.width, rect.height].every(
              Number.isFinite,
            ) ||
            rect.width < 0 ||
            rect.height < 0,
        )
      )
        return false;
      observed.set(target, { bounds, physical });
      return true;
    };
    const omitted = new Set<number>();
    let rootFragments = 0;
    if (coverage.kind === 'carrier-candidates')
      for (const target of retained) {
        if (!readTarget(target)) return null;
        if (target.kind !== 'chip') continue;
        rootFragments += requireValue(
          observed.get(target),
          'reorder-mounted-layout',
        ).physical.fragments.filter((rect) => rect.width > 0 && rect.height > 0).length;
        // Root fragments are already a lower bound on the complete rich footprint budget.
        if (rootFragments > 4096) return null;
        const outside = requireValue(
          observed.get(target),
          'reorder-mounted-layout',
        ).physical.fragments.some(
          (rect) =>
            rect.width > 0 &&
            rect.height > 0 &&
            (rect.left < coverage.viewport.left ||
              rect.right > coverage.viewport.right ||
              rect.top < coverage.viewport.top ||
              rect.bottom > coverage.viewport.bottom),
        );
        if (!available()) return null;
        if (outside) omitted.add(target.index);
      }
    const completed = new Set<number>();
    /** Each target occurrence is charged once; repeated node identities retain the original complete-path reads and budgets. */
    const readFootprint = (target: MountedReorderTarget, occurrence: number): boolean => {
      if (completed.has(occurrence)) return true;
      /** Root connection and both complete owner checks share one native mutation fence. */
      const rootCurrent = (): boolean =>
        checkpoint(() => owned() && target.node.isConnected && owned());
      const rects = nativeReorderFootprint(
        target.node,
        requireValue(nodes.get(target.node), 'reorder-mounted-layout'),
        target.index,
        undefined,
        {
          sample: requireValue(observed.get(target), 'reorder-mounted-layout').physical,
          current: rootCurrent,
        },
      );
      if (!rootCurrent() || !rects || !rects.length || rects.length > 256) return false;
      fragments += rects.length;
      if (fragments > 4096) return false;
      footprints.set(target.node, rects);
      completed.add(occurrence);
      return true;
    };
    for (const [occurrence, target] of retained.entries()) {
      if (target.kind !== 'chip' || omitted.has(target.index)) continue;
      if (coverage.kind === 'complete' && !readTarget(target)) return null;
      if (!readFootprint(target, occurrence)) return null;
    }
    for (const target of retained)
      if (coverage.kind === 'complete' && target.kind !== 'chip' && !readTarget(target))
        return null;
    const layout: ReorderRect[] = [];
    for (const target of retained) {
      if (!available()) return null;
      const root = requireValue(observed.get(target), 'reorder-mounted-layout');
      for (const rect of localTargetFragments(target, root.bounds, root.physical.fragments))
        layout.push(positionedTargetFragment(rect, root.bounds));
    }
    const rootLayout = Object.freeze(layout);
    /** Slot retirement revokes this borrowed continuation; no sample or callback crosses a WRITE boundary. */
    const complete = (): ReorderCompleteMountedObservation | null => {
      if (!available()) return null;
      for (const [occurrence, target] of retained.entries())
        if (target.kind === 'chip' && !readFootprint(target, occurrence)) return null;
      return available()
        ? Object.freeze({
            coverage: 'complete' as const,
            layout: rootLayout,
            footprints: new Map(
              retained
                .filter((target) => target.kind === 'chip')
                .map((target) => [
                  target.node,
                  requireValue(footprints.get(target.node), 'reorder-mounted-layout'),
                ]),
            ),
          })
        : null;
    };
    if (!available()) return null;
    return coverage.kind === 'complete'
      ? Object.freeze({
          coverage: 'complete' as const,
          layout: rootLayout,
          footprints,
        })
      : Object.freeze({
          coverage: 'carrier-candidates' as const,
          layout: rootLayout,
          footprints: new Map(footprints),
          omitted,
          complete,
        });
  }
  /** Called only from the canonical CodeMirror read phase; stale or detached output rejects before admission. */
  measure(targets: readonly MountedReorderTarget[]): readonly ReorderRect[] | null {
    if (this.disposed) return null;
    const window = this.view.dom.ownerDocument.defaultView;
    if (!window) return null;
    const content = window.getComputedStyle(this.view.contentDOM);
    const appearance = [
      this.view.themeClasses,
      content.font,
      content.lineHeight,
      content.letterSpacing,
      content.wordSpacing,
      content.fontFeatureSettings,
      content.fontVariationSettings,
      content.fontKerning,
      content.fontOpticalSizing,
      content.fontVariantLigatures,
      content.fontVariantNumeric,
      content.fontVariantEastAsian,
      content.whiteSpace,
      content.wordBreak,
      content.overflowWrap,
      content.tabSize,
      content.direction,
      content.unicodeBidi,
      content.textTransform,
      content.textRendering,
      window.innerWidth,
      window.innerHeight,
      window.devicePixelRatio,
    ];
    if (this.disposed) return null;
    const lines = new Map<HTMLElement, ContainingLineObservation>();
    const observed: {
        readonly target: MountedReorderTarget;
        readonly context: string;
        readonly bounds: DOMRect;
        readonly physical: DOMRectList;
      }[] = [],
      identities: string[] = [];
    for (const target of targets) {
      if (this.disposed) return null;
      const line = target.node.closest<HTMLElement>('.cm-line');
      if (!line || !target.node.isConnected || this.disposed) return null;
      let containing = lines.get(line);
      if (!containing) {
        const style = window.getComputedStyle(line);
        if (this.disposed) return null;
        const bounds = line.getBoundingClientRect();
        if (this.disposed) return null;
        containing = Object.freeze({
          width: style.width,
          paddingLeft: style.paddingLeft,
          paddingRight: style.paddingRight,
          textIndent: style.textIndent,
          direction: style.direction,
          unicodeBidi: style.unicodeBidi,
          left: bounds.left,
          boundsWidth: bounds.width,
        });
        if (this.disposed) return null;
        lines.set(line, containing);
      }
      const targetStyle = window.getComputedStyle(target.node);
      if (this.disposed) return null;
      const bounds = target.node.getBoundingClientRect();
      if (this.disposed) return null;
      const physical = target.node.getClientRects();
      if (this.disposed) return null;
      if (
        Array.from(physical).some(
          (rect) =>
            ![rect.left, rect.right, rect.top, rect.bottom, rect.width, rect.height].every(
              Number.isFinite,
            ) ||
            rect.width < 0 ||
            rect.height < 0,
        )
      )
        return null;
      if (
        ![bounds.left, bounds.top, bounds.width, bounds.height, containing.boundsWidth].every(
          Number.isFinite,
        ) ||
        bounds.height <= 0 ||
        containing.boundsWidth <= 0
      )
        return null;
      const context = JSON.stringify([
        appearance,
        containing.width,
        containing.paddingLeft,
        containing.paddingRight,
        containing.textIndent,
        containing.direction,
        containing.unicodeBidi,
        containing.boundsWidth,
        targetStyle.font,
        targetStyle.lineHeight,
        targetStyle.letterSpacing,
        targetStyle.wordSpacing,
        targetStyle.fontFeatureSettings,
        targetStyle.fontVariationSettings,
        targetStyle.fontKerning,
        targetStyle.fontOpticalSizing,
        targetStyle.fontVariantLigatures,
        targetStyle.fontVariantNumeric,
        targetStyle.fontVariantEastAsian,
        targetStyle.textTransform,
        targetStyle.textRendering,
        targetStyle.whiteSpace,
        targetStyle.wordBreak,
        targetStyle.overflowWrap,
        targetStyle.tabSize,
        targetStyle.direction,
        targetStyle.unicodeBidi,
        targetStyle.verticalAlign,
        targetStyle.writingMode,
        bounds.left - containing.left,
        bounds.width,
        bounds.height,
        target.kind,
        target.index,
        target.blank,
        Array.from(physical, (rect) => [
          rect.left - containing.left,
          rect.top - bounds.top,
          rect.right - containing.left,
          rect.bottom - bounds.top,
          rect.width,
          rect.height,
        ]),
        target.origins,
        target.leases.map((lease) => lease.key),
      ]);
      observed.push({ target, context, bounds, physical });
      identities.push(JSON.stringify([context, bounds.left, bounds.top]));
    }
    if (this.disposed) return null;
    const identity = identities.join('|'),
      previous = this.measured.get(identity);
    if (previous) return previous;
    const result: ReorderRect[] = [];
    for (const { target, context, bounds, physical } of observed) {
      const lease = target.leases[0];
      let geometry = lease?.geometry(context) ?? null;
      if (this.disposed) return null;
      if (!geometry) {
        const rects = localTargetFragments(target, bounds, physical);
        const height = bounds.height;
        if (this.disposed) return null;
        if (lease && height > 0) {
          lease.measure(context, { height, rects });
          if (this.disposed) return null;
          geometry = lease.geometry(context);
        } else geometry = { height: bounds.height, rects };
      }
      if (this.disposed) return null;
      for (const rect of requireValue(geometry, 'reorder-mounted-layout').rects)
        result.push(positionedTargetFragment(rect, bounds));
    }
    const rects = Object.freeze(result);
    if (this.disposed) return null;
    if (this.measured.size === 64)
      this.measured.delete(
        requireValue(this.measured.keys().next().value, 'reorder-mounted-layout'),
      );
    this.measured.set(identity, rects);
    return rects;
  }
  /** One cold physical-slot observation reuses the genuine current measured output and exact target nodes. */
  physical(
    targets: readonly MountedReorderTarget[],
    rects: readonly ReorderRect[],
  ): readonly ReorderPhysicalFragment[] | null {
    if (this.disposed) return null;
    const nodes = reorderPhysicalTargetNodes(targets);
    if (!nodes) {
      this.placement = null;
      return null;
    }
    const cached = this.placement;
    if (
      cached?.rects === rects &&
      cached.nodes.length === nodes.length &&
      nodes.every((node, at) => node === cached.nodes[at] && node.isConnected)
    )
      return cached.fragments;
    const fragments = readReorderPhysicalFragments(targets, rects);
    this.placement = fragments ? { rects, nodes, fragments } : null;
    return fragments;
  }
  /** Real native font/resource completion invalidates derived geometry even when outer box dimensions happen to match. */
  invalidate(): void {
    this.measured.clear();
    this.placement = null;
  }
  /** Retirement prevents old target callbacks from borrowing retained geometry and releases every cached output. */
  destroy(): void {
    this.disposed = true;
    this.invalidate();
  }
}
