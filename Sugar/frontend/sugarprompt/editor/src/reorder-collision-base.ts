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
/** One finite keyboard intent or pointer hold measures its immutable Core collision operand outside live CM paint. */
import { requireValue } from '../../core/src/required.js';
import { ReadonlyWidgetMedia } from './readonly-widget-media.js';
import { Decoration, type EditorView } from '@codemirror/view';
import {
  valueReorderLayout,
  valueReorderVacancy,
  type ReorderState,
  type ReorderDropTarget,
  type ReorderLayout,
} from '../../core/src/index.js';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import type { ReorderMarkerRect, ReorderRect } from './reorder-geometry.js';
import {
  prepareReorderPlacementTopology,
  resolveReorderPlacement,
  sameReorderTarget,
  type ReorderPlacementTopology,
  type ReorderPlacementSlot,
} from './reorder-placement-topology.js';
import { prepareReorderPreview } from './reorder-preview-preparation.js';
import { ReorderWidget, type PassiveReorderPaint } from './reorder-widget.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import {
  canonicalReorderFragment,
  renderReorderFragment,
  releaseReorderFragment,
} from './reorder-projection.js';
import {
  createReorderBaseScene,
  readReorderBaseContext,
  resetReorderBasePlacement,
  type ReorderBaseContext,
} from './reorder-base-context.js';
/** Prepared clipping is borrowed only under the exact viewport issued by the native read phase. */
function sameViewport(a: ReorderMarkerRect | undefined, b: ReorderMarkerRect): boolean {
  return (
    !!a && a.left === b.left && a.right === b.right && a.top === b.top && a.bottom === b.bottom
  );
}
/** The first native read calibrates external CSS space; the next observes canonical collision geometry. */
export type ReorderBaseRead =
  | {
      readonly kind: 'calibration';
      readonly root: HTMLElement;
      readonly context: ReorderBaseContext;
      readonly left: number;
      readonly top: number;
      readonly scaleX: number;
      readonly scaleY: number;
    }
  | {
      readonly kind: 'ready';
      readonly rects: readonly ReorderRect[];
    }
  | {
      readonly kind: 'unavailable';
    };
/** Live pointer source/context owns freshness; this object owns only actual external DOM/geometry leases. */
export class ReorderCollisionBase {
  private readonly view: EditorView;
  private input: ReorderMeasureInput | null = null;
  private context: ReorderBaseContext | null = null;
  private root: HTMLElement | null = null;
  private paint: PassiveReorderPaint | null = null;
  private media: ReadonlyWidgetMedia | null = null;
  private edges: HTMLElement[] = [];
  private rects: readonly ReorderRect[] | null = null;
  private preview: ReorderPreview | null = null;
  private mountedPreview: ReorderPreview | null = null;
  private calibrated = false;
  private topology: ReorderLayout | null = null;
  private fonts: FontFaceSet | null = null;
  private placement: {
    readonly rects: readonly ReorderRect[];
    readonly topology: ReorderPlacementTopology;
    readonly viewport: ReorderMarkerRect;
  } | null = null;
  private activeOrder: ReorderState | null = null;
  private activeTarget: ReorderDropTarget | null = null;
  private activeSlot: ReorderPlacementSlot | null = null;
  /** Font readiness can change glyph layout without changing a computed font-family spelling. */
  private readonly fontChanged = (): void => {
    this.rects = null;
    this.placement = null;
    this.paint?.invalidateMeasurement();
  };
  /** The real view supplies canonical materialization and supported measure phases, never another editor. */
  constructor(view: EditorView) {
    this.view = view;
  }
  /** Prepare one genuine hidden output outside the native geometry read; targets reuse this exact operand. */
  prepare(input: ReorderMeasureInput): boolean {
    if (!input.hidden) return false;
    if (
      this.preview?.token !== input.token ||
      this.preview.order !== input.hidden ||
      this.preview.capture !== input.capture ||
      this.preview.activeChip !== input.held
    ) {
      this.preview = prepareReorderPreview(
        {
          token: input.token,
          capture: input.capture,
          activeChip: input.held,
          order: input.hidden,
        },
        'collision',
      );
      this.topology = this.preview ? valueReorderLayout(input.hidden) : null;
    }
    return this.preview !== null;
  }
  /** Context reads are independent of proposed chip fragments and resulting content/line height. */
  contextAt(input: ReorderMeasureInput): ReorderBaseContext | null {
    return this.preview?.token === input.token && this.preview.order === input.hidden
      ? readReorderBaseContext(this.view, this.preview)
      : null;
  }
  /** Only a genuinely measured current frame can resolve targets; exact same context retains actual output identity. */
  current(input: ReorderMeasureInput, context: ReorderBaseContext): readonly ReorderRect[] | null {
    return this.input === input &&
      this.preview === this.mountedPreview &&
      this.context?.key === context.key &&
      this.placement?.rects === this.rects &&
      sameViewport(this.placement?.viewport, context.viewport) &&
      this.context.current() &&
      context.current() &&
      this.root?.isConnected &&
      this.paint?.current() &&
      this.root.contains(this.paint.node)
      ? this.rects
      : null;
  }
  /** Both input modes borrow the same actual prepared physical-slot output under its exact rectangle lease. */
  placements(
    input: ReorderMeasureInput,
    context: ReorderBaseContext,
  ): ReorderPlacementTopology | null {
    const rects = this.current(input, context);
    return rects && this.placement?.rects === rects ? this.placement.topology : null;
  }
  /** Pointer containment and anchor selection consume prepared bands; only genuine order changes refresh the Core vacancy. */
  target(
    input: ReorderMeasureInput,
    context: ReorderBaseContext,
    x: number,
    y: number,
  ): ReorderDropTarget | null {
    const topology = this.placements(input, context);
    if (!topology || !input.hidden) return null;
    if (this.activeOrder !== input.order) {
      this.activeOrder = input.order;
      this.activeTarget = valueReorderVacancy(input.order, input.hidden, input.held);
    }
    const slot = resolveReorderPlacement(topology, { x, y }, this.activeTarget, this.activeSlot);
    this.activeSlot = slot;
    return slot?.target ?? null;
  }
  /** WRITE rebinds an equivalent fresh containing line under stable appearance; changed placement still requires native recalibration. */
  mount(input: ReorderMeasureInput, context: ReorderBaseContext): void {
    const preview = this.preview,
      topology = this.topology;
    if (
      this.input === input &&
      preview === this.mountedPreview &&
      this.context?.paintKey === context.paintKey &&
      this.context.lineParent === context.lineParent &&
      this.context.lineRoot === context.lineRoot &&
      this.context.appearanceCurrent() &&
      context.current() &&
      this.calibrated &&
      this.rects &&
      this.root?.isConnected &&
      this.paint?.current() &&
      this.root.contains(this.paint.node)
    ) {
      const moved = this.context.key !== context.key;
      this.context = context;
      this.placement = null;
      if (moved) {
        this.rects = null;
        this.calibrated = false;
        try {
          resetReorderBasePlacement(this.root, context);
        } catch (cause) {
          this.constructionFailed(cause);
        }
      }
      return;
    }
    this.destroy();
    if (!preview || preview.token !== input.token || preview.order !== input.hidden)
      throw new Error('reorder-base-invalid-collision');
    this.preview = preview;
    this.mountedPreview = preview;
    this.topology = topology;
    const scene = createReorderBaseScene(this.view, context);
    this.root = scene.root;
    this.input = input;
    this.context = context;
    try {
      // Collision output is geometry-only: retain canonical backing chrome without allocating duplicate raster buffers.
      const media = new ReadonlyWidgetMedia();
      this.media = media;
      const canonical = input.capture.projection?.visible ?? Decoration.none;
      for (const range of [context.prefix, context.suffix]) {
        const edge = scene.line.ownerDocument.createElement('span');
        this.edges.push(edge);
        renderReorderFragment(
          this.view,
          edge,
          (part) => this.view.state.doc.sliceString(part.from, part.to),
          range,
          canonical === Decoration.none
            ? canonical
            : canonicalReorderFragment(this.view.state, canonical, range),
          null,
          media,
        );
      }
      this.paint = new ReorderWidget(preview, null).mountPassive(this.view, media);
      scene.line.append(
        requireValue(this.edges[0], 'reorder-collision-base'),
        this.paint.node,
        requireValue(this.edges[1], 'reorder-collision-base'),
      );
      const parent =
        this.view.dom.parentElement ??
        ('body' in this.view.root
          ? (this.view.root as Document).body
          : (this.view.root as ShadowRoot));
      parent.append(scene.root);
      const fonts = this.view.dom.ownerDocument.fonts;
      if (fonts) {
        this.fonts = fonts;
        fonts.addEventListener('loadingdone', this.fontChanged);
        fonts.addEventListener('loadingerror', this.fontChanged);
      }
    } catch (cause) {
      this.constructionFailed(cause);
    }
  }
  /** Failed creation or repositioning retires the same finite leases and preserves primary/cleanup error order. */
  private constructionFailed(cause: unknown): never {
    try {
      this.destroy();
    } catch (cleanup) {
      throw new AggregateError([cause, cleanup], 'Reorder base construction cleanup failed');
    }
    throw cause;
  }
  /** A subsequent CM read observes actual passive layout; it never estimates offscreen chip sizes. */
  measure(input: ReorderMeasureInput, context: ReorderBaseContext): ReorderBaseRead {
    if (
      this.input !== input ||
      this.preview !== this.mountedPreview ||
      this.context?.key !== context.key ||
      !this.context.current() ||
      !context.current() ||
      !this.root?.isConnected ||
      !this.paint?.current() ||
      !this.root.contains(this.paint.node)
    )
      return { kind: 'unavailable' };
    if (!this.calibrated) {
      const box = this.root.getBoundingClientRect();
      return [box.width, box.height].every((value) => Number.isFinite(value) && value > 0) &&
        [box.left, box.top].every(Number.isFinite)
        ? {
            kind: 'calibration',
            root: this.root,
            context,
            left: box.left,
            top: box.top,
            scaleX: box.width / context.width,
            scaleY: box.height / 256,
          }
        : { kind: 'unavailable' };
    }
    const rects = this.paint?.measure() ?? null;
    if (!rects || !this.topology) return { kind: 'unavailable' };
    const viewport = this.view.scrollDOM.getBoundingClientRect(),
      cached = this.placement;
    if (cached?.rects === rects && sameViewport(cached.viewport, viewport)) {
      this.rects = rects;
      return { kind: 'ready', rects };
    }
    const physical = this.paint?.physical(rects),
      topology = physical && prepareReorderPlacementTopology(this.topology, physical, viewport);
    if (!topology) {
      this.rects = null;
      this.placement = null;
      return { kind: 'unavailable' };
    }
    const previous = this.activeSlot;
    this.activeSlot =
      this.rects === rects && previous
        ? (topology.slots.find(
            (slot) =>
              sameReorderTarget(slot.target, previous.target) &&
              slot.lane === previous.lane &&
              slot.anchor === previous.anchor &&
              slot.y === previous.y,
          ) ?? null)
        : null;
    this.rects = rects;
    this.placement = {
      rects,
      topology,
      viewport: {
        left: viewport.left,
        right: viewport.right,
        top: viewport.top,
        bottom: viewport.bottom,
      },
    };
    return { kind: 'ready', rects };
  }
  /** Native write adapts measured external space to the actual original containing-line viewport footprint. */
  align(
    read: Extract<
      ReorderBaseRead,
      {
        kind: 'calibration';
      }
    >,
  ): boolean {
    if (
      this.root !== read.root ||
      this.context !== read.context ||
      this.calibrated ||
      !this.root.isConnected ||
      !this.paint?.current() ||
      !this.root.contains(this.paint.node)
    )
      return false;
    const { root, context } = read;
    root.style.left = `${(context.left - read.left) / read.scaleX}px`;
    root.style.top = `${(context.top - read.top) / read.scaleY}px`;
    root.style.setProperty('width', `${context.width}px`, 'important');
    // Keep the independent 256px scale calibration box; actual descendant geometry owns collision topology.
    root.style.setProperty(
      'transform',
      `scale(${context.scaleX / read.scaleX}, ${context.scaleY / read.scaleY})`,
      'important',
    );
    this.calibrated = true;
    return true;
  }
  /** Detach and retire every actual shared widget/line lease even when another resource release fails. */
  destroy(): void {
    const root = this.root,
      paint = this.paint,
      media = this.media,
      edges = this.edges,
      fonts = this.fonts;
    this.root = null;
    this.paint = null;
    this.media = null;
    this.edges = [];
    this.input = null;
    this.context = null;
    this.rects = null;
    this.placement = null;
    this.activeOrder = null;
    this.activeTarget = null;
    this.activeSlot = null;
    this.preview = null;
    this.mountedPreview = null;
    this.fonts = null;
    this.calibrated = false;
    this.topology = null;
    const failures: unknown[] = [];
    for (const action of [
      () => fonts?.removeEventListener('loadingdone', this.fontChanged),
      () => fonts?.removeEventListener('loadingerror', this.fontChanged),
      () => root?.remove(),
      () => media?.destroy(),
      () => paint?.destroy(),
      ...edges.map((edge) => () => releaseReorderFragment(edge)),
    ]) {
      try {
        action();
      } catch (cause) {
        failures.push(cause);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'Reorder base cleanup failed');
  }
}
