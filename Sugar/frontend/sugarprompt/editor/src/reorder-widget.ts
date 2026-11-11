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
/** One native replacement widget owns changed reorder paint within the affected CM source interval. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderLayoutReadAuthority } from './reorder-slot-layout-read.js';
import type { admitReorderTargetLayout } from './reorder-layout-admission.js';
import type { ReorderMotionMask } from './reorder-motion-mask.js';
import {
  sameReorderVisibility,
  updateReorderChipPresentation,
} from './reorder-chip-presentation.js';
import { ReorderPaintAdmission } from './reorder-paint-admission.js';
import type { ReadonlyWidgetMedia } from './readonly-widget-media.js';
import { WidgetType, type EditorView } from '@codemirror/view';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { samePreparedReorderPaint } from './reorder-preview-preparation.js';
import { preparedReorderSourceLines } from './source-line-origins.js';
import { reconcileReorderBlocks } from './reorder-widget-blocks.js';
import { ReorderLineLayout } from './reorder-line-layout.js';
import { prepareReorderWidgetPlan, type ReorderWidgetPlan } from './reorder-widget-plan.js';
import type { ReorderPhysicalFragment } from './reorder-placement-topology.js';
import type { ReorderRect, ReorderMarkerWitnesses } from './reorder-geometry.js';
import { ReorderMountedLayout } from './reorder-mounted-layout.js';
import type {
  MountedReorderTarget,
  ReorderMountedObservation,
  ReorderMountedCoverage,
} from './reorder-mounted-target.js';
import { ReorderPaintRetired, ReorderMountedBlock } from './reorder-widget-block.js';
/** CodeMirror alone mounts or updates this root; one finite owner retains its canonical line paint. */
interface MountedPaint {
  readonly root: HTMLElement;
  readonly media: ReadonlyWidgetMedia | undefined;
  readonly view: EditorView;
  readonly lines: ReorderLineLayout;
  readonly blocks: Set<ReorderMountedBlock>;
  readonly targets: MountedReorderTarget[];
  preview: ReorderPreview;
  issue: object;
  completeIssue: object | null;
  mask: ReorderMotionMask | null;
  readonly layout: ReorderMountedLayout;
}
const mountedPaint = new WeakMap<HTMLElement, MountedPaint>();
const mountedViews = new WeakMap<EditorView, MountedPaint>();
/** Capture a constant-time mounted paint stamp after canonical equivalence admission; frames never traverse Core order. */
export function currentReorderPaintLease(
  view: EditorView,
  preview: ReorderPreview,
): (() => boolean) | null {
  const mounted = mountedViews.get(view);
  if (!mounted || !samePreparedReorderPaint(mounted.preview, preview)) return null;
  if (mounted.completeIssue !== mounted.issue) return null;
  return mountedReorderLease(view, mounted);
}
/** The actual canonical issue remains borrowable until its mounted painter changes. */
function mountedReorderLease(view: EditorView, mounted: MountedPaint): () => boolean {
  const stamp = mounted.preview,
    issue = mounted.issue;
  return () =>
    mountedViews.get(view) === mounted &&
    mounted.preview === stamp &&
    mounted.issue === issue &&
    mounted.completeIssue === issue &&
    mounted.root.isConnected;
}
/** Current mounted semantic targets belong to the exact native publication lease, never a document-wide selector. */
export function currentReorderPaintTargets(
  view: EditorView,
): readonly MountedReorderTarget[] | null {
  const mounted = mountedViews.get(view);
  return mounted?.completeIssue === mounted?.issue && mounted?.root.isConnected
    ? mounted.targets
    : null;
}
/** Canonical warm admission distinguishes an untouched key miss from synchronous owner retirement. */
export type PassiveReorderUpdate = 'ready' | 'miss' | 'retired';
/** A passive root owns independent paint/measurement leases and never registers live editor geometry. */
export interface PassiveReorderPaint {
  readonly node: HTMLElement;
  /** The exact still-mounted resource lease owns this connected root. */
  current(): boolean;
  /** The canonical painter owns these exact semantic target roots for this finite mount. */
  targets(): readonly MountedReorderTarget[];
  /** Admit the complete exact key multiset before supported canonical update; misses leave this mount untouched. */
  reconcile(preview: ReorderPreview): PassiveReorderUpdate;
  measure(): readonly ReorderRect[] | null;
  /** A fresh whole-paint READ shares actual root fragments with complete rich footprints under exact request authority. */
  observe(
    authority: ReorderLayoutReadAuthority,
    readTargets: typeof admitReorderTargetLayout,
    coverage: ReorderMountedCoverage,
  ): ReorderMountedObservation | null;
  /** Exact native physical-slot output is prepared once for an admitted rectangle identity. */
  physical(rects: readonly ReorderRect[]): readonly ReorderPhysicalFragment[] | null;
  /** Native font/resource invalidation retires both rectangle and caret measurement memoization. */
  invalidateMeasurement(): void;
  destroy(): void;
}
/** Only a current mounted replacement exposes geometry through its finite native read owner. */
export function measuredReorderWidget(
  view: EditorView,
  preview: ReorderPreview,
): readonly ReorderRect[] | null {
  const mounted = mountedViews.get(view);
  return mounted &&
    mounted.completeIssue === mounted.issue &&
    samePreparedReorderPaint(mounted.preview, preview)
    ? mounted.layout.measure(mounted.targets)
    : null;
}
/** Marker witnesses borrow only current canonical targets; the mounted layout owner selects them before observation. */
export function measuredReorderMarker(
  view: EditorView,
  preview: ReorderPreview,
  witnesses: ReorderMarkerWitnesses,
): readonly ReorderRect[] | null {
  const mounted = mountedViews.get(view);
  return mounted &&
    mounted.completeIssue === mounted.issue &&
    samePreparedReorderPaint(mounted.preview, preview)
    ? mounted.layout.marker(mounted.targets, witnesses)
    : null;
}
/** The widget owns only literal display nodes and typed chip/gap targets. */
export class ReorderWidget extends WidgetType {
  readonly preview: ReorderPreview;
  /** Exact StateField visibility descriptor advances independently of canonical geometry. */
  readonly mask: ReorderMotionMask | null;
  /** Keep the genuine immutable publication for measurement identity checks. */
  constructor(preview: ReorderPreview, mask: ReorderMotionMask | null) {
    super();
    this.preview = preview;
    this.mask = mask;
  }
  /** CodeMirror's native height map consumes actual projected breaks, including canonical replacement coverage. */
  override get lineBreaks(): number {
    const lines = preparedReorderSourceLines(this.preview.capture, this.preview.output);
    return lines.length - 1 + lines.reduce((total, line) => total + line.widgetBreaks, 0);
  }
  /** Retain identical rendered targets within one genuine session; the latest decoration still carries its new publication. */
  override eq(other: ReorderWidget): boolean {
    return (
      samePreparedReorderPaint(this.preview, other.preview) &&
      sameReorderVisibility(this.mask, other.mask)
    );
  }
  /** Render exact current serialized carriers and chip ranges literally. */
  toDOM(view: EditorView): HTMLElement {
    return this.mount(view, true);
  }
  /** The canonical painter may materialize a separate collision scene without stealing the live CM mount. */
  mountPassive(view: EditorView, media?: ReadonlyWidgetMedia): PassiveReorderPaint {
    const node = this.mount(view, false, media),
      mounted = requireValue(mountedPaint.get(node), 'reorder-widget');
    return Object.freeze({
      node,
      current: () =>
        mountedPaint.get(node) === mounted &&
        mounted.completeIssue === mounted.issue &&
        node.isConnected,
      reconcile: (preview: ReorderPreview) =>
        new ReorderWidget(preview, null).updateMounted(node, view, true),
      targets: () =>
        mountedPaint.get(node) === mounted && mounted.completeIssue === mounted.issue
          ? mounted.targets
          : [],
      measure: () =>
        mountedPaint.get(node) === mounted && mounted.completeIssue === mounted.issue
          ? mounted.layout.measure(mounted.targets)
          : null,
      observe: (
        authority: ReorderLayoutReadAuthority,
        readTargets: typeof admitReorderTargetLayout,
        coverage: ReorderMountedCoverage,
      ) => {
        const issue = mounted.issue,
          preview = mounted.preview;
        /** Current publication alone cannot certify a replaced mounted issue, even for the same preview. */
        const issued = (): boolean =>
          mountedPaint.get(node) === mounted &&
          mounted.issue === issue &&
          mounted.completeIssue === issue &&
          mounted.preview === preview;
        /** A synchronous native connection getter cannot replace the captured mounted issue. */
        const owned = (): boolean => issued() && node.isConnected && issued();
        /** Recheck the bare request after the final connection getter without nesting the Slot fence. */
        const available = (): boolean =>
          owned() && authority.available() && owned() && authority.available();
        /** Retain the original post-request connection fence before final pure issued/request admission. */
        const guard = (predicate: () => boolean): boolean =>
          authority.guard(predicate) && owned() && authority.available();
        return guard(available)
          ? mounted.layout.observe(mounted.targets, { available, guard }, readTargets, coverage)
          : null;
      },
      physical: (rects: readonly ReorderRect[]) =>
        mountedPaint.get(node) === mounted &&
        mounted.completeIssue === mounted.issue &&
        node.isConnected
          ? mounted.layout.physical(mounted.targets, rects)
          : null,
      invalidateMeasurement: () => mounted.layout.invalidate(),
      destroy: () => this.destroy(node),
    });
  }
  /** Every root has independent resources; only CodeMirror's live adapter registers view geometry. */
  private mount(view: EditorView, live: boolean, media?: ReadonlyWidgetMedia): HTMLElement {
    const root = view.dom.ownerDocument.createElement('span');
    root.className = live ? 'sp-reorder-preview sp-reorder-pointer-underlay' : 'sp-reorder-preview';
    root.contentEditable = 'false';
    const mounted: MountedPaint = {
      root,
      media,
      view,
      lines: new ReorderLineLayout(),
      blocks: new Set(),
      targets: [],
      preview: this.preview,
      issue: {},
      completeIssue: null,
      mask: this.mask,
      layout: new ReorderMountedLayout(view),
    };
    mountedPaint.set(root, mounted);
    if (live) mountedViews.set(view, mounted);
    const issue = mounted.issue;
    try {
      const plan = prepareReorderWidgetPlan(this.preview);
      if (mountedPaint.get(root) !== mounted || mounted.issue !== issue)
        throw new ReorderPaintRetired();
      const admission = new ReorderPaintAdmission(
        plan,
        () => mountedPaint.get(root) === mounted && mounted.issue === issue,
      );
      return this.paint(root, view, plan, admission);
    } catch (cause) {
      try {
        if (mountedPaint.get(root) === mounted && mounted.issue === issue) this.destroy(root);
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'Reorder preview mount cleanup failed');
      }
      throw cause;
    }
  }
  /** A genuine same-capture replacement reuses mounted canonical paint through CodeMirror's supported callback. */
  override updateDOM(root: HTMLElement, view: EditorView): boolean {
    const mounted = mountedPaint.get(root);
    if (
      mounted &&
      mounted.view === view &&
      mounted.mask !== this.mask &&
      samePreparedReorderPaint(mounted.preview, this.preview)
    ) {
      const issue = mounted.issue;
      if (mounted.completeIssue !== issue) return false;
      mounted.mask = this.mask;
      const owned = (): boolean =>
        mountedPaint.get(root) === mounted &&
        mounted.issue === issue &&
        mounted.mask === this.mask &&
        mounted.completeIssue === issue;
      const current = (): boolean => owned() && root.isConnected && owned();
      return updateReorderChipPresentation(
        mounted.targets,
        this.preview,
        this.mask,
        current,
        () => {
          if (owned()) this.destroy(root);
        },
      );
    }
    return this.updateMounted(root, view, false) === 'ready';
  }
  /** Warm passive admission preflights every missing canonical fragment and retains exact original resource-bearing output. */
  private updateMounted(
    root: HTMLElement,
    view: EditorView,
    existingOnly: boolean,
  ): PassiveReorderUpdate {
    const mounted = mountedPaint.get(root);
    if (
      !mounted ||
      mounted.view !== view ||
      mounted.preview.token !== this.preview.token ||
      mounted.preview.capture !== this.preview.capture
    )
      return 'retired';
    const issue = {},
      previousIssue = mounted.issue;
    try {
      // Classification may reenter native publication before any canonical mutation.
      const plan = prepareReorderWidgetPlan(this.preview);
      if (mountedPaint.get(root) !== mounted || mounted.issue !== previousIssue)
        throw new ReorderPaintRetired();
      const admission = new ReorderPaintAdmission(
        plan,
        () => mountedPaint.get(root) === mounted && mounted.issue === previousIssue,
      );
      const update = (): void => {
        if (mountedPaint.get(root) !== mounted || mounted.issue !== previousIssue)
          throw new ReorderPaintRetired();
        mounted.issue = issue;
        mounted.completeIssue = null;
        mounted.preview = this.preview;
        mounted.mask = this.mask;
        this.paint(root, view, plan, admission);
        if (
          mountedPaint.get(root) !== mounted ||
          mounted.issue !== issue ||
          mounted.mask !== this.mask
        )
          throw new ReorderPaintRetired();
      };
      if (existingOnly) {
        const admitted = mounted.lines.reconcilePrepared(plan.keys, admission.allowMissing, update);
        if (!admitted && (mountedPaint.get(root) !== mounted || mounted.issue !== previousIssue))
          throw new ReorderPaintRetired();
        return admitted ? 'ready' : 'miss';
      }
      mounted.lines.reconcile(update);
      return 'ready';
    } catch (cause) {
      try {
        if (
          mountedPaint.get(root) === mounted &&
          mounted.issue === issue &&
          mounted.mask === this.mask
        )
          this.destroy(root);
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'Reorder preview update cleanup failed');
      }
      if (cause instanceof ReorderPaintRetired) return 'retired';
      throw cause;
    }
  }
  /** Literal target wrappers preserve native inline flow; cached inner paint is cut only at canonical projected hard lines. */
  private paint(
    root: HTMLElement,
    view: EditorView,
    plan: ReorderWidgetPlan,
    admission: ReorderPaintAdmission,
  ): HTMLElement {
    const mounted = requireValue(mountedPaint.get(root), 'reorder-widget'),
      issue = mounted.issue;
    /** Native callbacks may synchronously retire this exact mount; stop before borrowing from its retired line owner. */
    const current = (): void => {
      if (
        mountedPaint.get(root) !== mounted ||
        mounted.issue !== issue ||
        mounted.mask !== this.mask
      )
        throw new ReorderPaintRetired();
    };
    try {
      reconcileReorderBlocks(root, mounted, this.preview, this.mask, {
        view,
        plan,
        admission,
        current,
      });
      mounted.completeIssue = issue;
      return root;
    } catch (cause) {
      try {
        if (
          mountedPaint.get(root) === mounted &&
          mounted.issue === issue &&
          mounted.mask === this.mask
        )
          this.destroy(root);
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'Reorder preview paint cleanup failed');
      }
      throw cause;
    }
  }
  /** Native pointer and wheel intent reach the interaction owner while this widget retains source caret isolation. */
  override ignoreEvent(event: Event): boolean {
    return ![
      'wheel',
      'dragstart',
      'drop',
      'pointerdown',
      'pointermove',
      'pointerup',
      'pointercancel',
      'lostpointercapture',
    ].includes(event.type);
  }
  /** Retire every separately mounted shared widget when CodeMirror replaces this preview. */
  override destroy(dom: HTMLElement): void {
    const mounted = mountedPaint.get(dom);
    mountedPaint.delete(dom);
    if (!mounted) return;
    if (mountedViews.get(mounted.view) === mounted) mountedViews.delete(mounted.view);
    mounted.layout.destroy();
    const failures: unknown[] = [];
    try {
      this.release(mounted);
    } catch (cause) {
      failures.push(cause);
    }
    try {
      mounted.lines.destroy();
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Reorder preview cleanup failed');
  }
  /** Retire borrowing before any subsequent target can acquire these exact line nodes. */
  private release(mounted: MountedPaint): void {
    const blocks = Array.from(mounted.blocks),
      failures: unknown[] = [];
    mounted.blocks.clear();
    for (const block of blocks)
      try {
        block.destroy();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'Reorder line borrowing cleanup failed');
  }
}
