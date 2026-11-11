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
/** Canonical passive motion paint owns external native layout and finite calibration, without another renderer or editable view. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderLayoutReadAuthority } from './reorder-slot-layout-read.js';
import { ReorderScenePaint } from './reorder-scene-motion-paint.js';
import { ReorderMotionPeerCarriers } from './reorder-motion-peer-carriers.js';
import {
  type ReorderPassiveGeometry,
  readReorderPassiveGeometry,
} from './reorder-passive-geometry-read.js';
import { type ReadonlyMediaPlan } from './readonly-widget-media.js';
import { admitReorderLineLayout, admitReorderTargetLayout } from './reorder-layout-admission.js';
import type { EditorView } from '@codemirror/view';
import { reorderPreview } from './reorder-preview.js';
import type { PassiveReorderPaint } from './reorder-widget.js';
import type { ReorderPassivePaintSlot } from './reorder-passive-paint-slot.js';
import { nativeReorderLineSize } from './reorder-native-paint.js';
import type { ReorderMotionScene, ReorderMotionSelection } from './reorder-motion-scene.js';
import type { ReorderMarkerRect } from './reorder-geometry.js';
import type { ReorderMotionFrame } from './reorder-motion-clock.js';
import type {
  PreparedPassiveMotion,
  PassiveMotionRead,
  PassiveMotionStage,
} from './reorder-motion-contracts.js';
/** One external mount owns canonical leases until transfer or retirement; calibration never estimates source shaping. */
export class PendingPassiveMotion {
  /** The slot is initialized before any caller observes the lease node. */
  get node(): HTMLElement {
    return this.slot.node;
  }
  private readonly view: EditorView;
  private readonly prepared: PreparedPassiveMotion;
  private readonly slot: ReorderPassivePaintSlot;
  /** Geometry and targets always belong to this complete original canonical slot. */
  private get paint(): PassiveReorderPaint {
    return this.slot.paint;
  }
  private readonly stage: PassiveMotionStage;
  private disposed = false;
  private scaleX = 1;
  private scaleY = 1;
  private scene: ReorderMotionScene | null = null;
  private carriers: ReorderMotionPeerCarriers | null = null;
  private scenePaint: ReorderScenePaint | null = null;
  private clip: ReorderMarkerRect | null = null;
  /** The partial viewport plan retains the original slot media owner. */
  private get media() {
    return this.slot.viewportMedia;
  }
  private mediaRoots: readonly HTMLElement[] | null = null;
  private hydrated = false;
  private wholeMedia = false;
  private unavailable = false;
  private layerWaits = 0;
  /** Reserve a fresh finite generation before the Stage performs any external construction, reconciliation or hydration. */
  constructor(
    view: EditorView,
    prepared: PreparedPassiveMotion,
    stage: PassiveMotionStage,
    slot: ReorderPassivePaintSlot,
  ) {
    this.view = view;
    this.prepared = prepared;
    this.stage = stage;
    this.slot = slot;
  }
  /** Stage calls this only after complete resources exist under this exact reserved lease. */
  initialize(reused: boolean): boolean {
    const preflight = reused ? 'whole' : this.slot.preflight(this.prepared);
    this.wholeMedia = preflight === 'whole';
    this.hydrated = this.wholeMedia;
    this.unavailable = preflight === 'retired';
    return this.current();
  }
  /** A current mount and exact original context are required before any native geometry inspection. */
  current(): boolean {
    return this.publicationCurrent() && this.slot.current();
  }
  /** Source, context and exact publication identity are checkable even while a reserved slot remains detached. */
  publicationCurrent(): boolean {
    return (
      !this.disposed &&
      !this.unavailable &&
      this.stage.current() &&
      this.prepared.context.current() &&
      this.view.state.field(reorderPreview, false)?.value === this.prepared.preview &&
      !this.disposed
    );
  }
  /** Retained display borrows the same gesture; every native continuation rechecks the captured child lease. */
  retained(): boolean {
    return (
      !this.disposed &&
      this.stage.current() &&
      !this.disposed &&
      this.slot.owns(this) &&
      this.node.isConnected &&
      !this.disposed &&
      this.slot.owns(this) &&
      this.paint.current() &&
      !this.disposed &&
      this.slot.owns(this)
    );
  }
  /** READ selects at most 64 eligible groups; whole-fit carrier preparation may defer excluded rich reads until complete pre-publication observation. */
  read(select: ReorderMotionSelection): PassiveMotionRead {
    if (!this.current() || !this.prepared.readViewportCurrent()) return { kind: 'unavailable' };
    const layer = this.stage.readLayer();
    if (layer === 'unavailable') return { kind: 'unavailable' };
    if (layer === 'waiting') return { kind: ++this.layerWaits <= 2 ? 'layer' : 'unavailable' };
    const calibration = this.stage.measured();
    if (!calibration) {
      return this.stage.readPlacement(this.prepared) ?? { kind: 'unavailable' };
    }
    this.scaleX = calibration.scaleX;
    this.scaleY = calibration.scaleY;
    if (this.scene) return { kind: 'ready', scene: this.scene };
    const result = this.prepared.whole
      ? this.slot.readLayout(this, (topology, authority) =>
          this.readLayout(select, topology.targets, authority),
        )
      : // Partial viewport reads retain their original unobserved current predicate.
        admitReorderLineLayout([this.node])
        ? this.readLayout(select, admitReorderTargetLayout, {
            available: () => true,
            guard: (predicate) => predicate(),
          })
        : null;
    if (!result) {
      if (this.carriers?.shellRefused() && this.current())
        return { kind: 'carriers', operation: 'discard' };
      return { kind: 'unavailable' };
    }
    if (result.kind === 'ready') this.scene = result.scene;
    return result;
  }
  /** Consume only this READ's admitted topology while retaining original geometry, footprint and publication policy. */
  private readLayout(
    select: ReorderMotionSelection,
    readTargets: typeof admitReorderTargetLayout,
    authority: ReorderLayoutReadAuthority,
  ): PassiveMotionRead {
    const viewport = this.view.scrollDOM.getBoundingClientRect();
    if (this.prepared.whole) {
      const extent = nativeReorderLineSize(this.view, this.slot.line);
      if (
        !extent ||
        extent.width !== this.prepared.context.width ||
        extent.height !== this.prepared.context.height ||
        !this.current()
      )
        return { kind: 'unavailable' };
    }
    const state = this.view.state;
    /** Immutable state and exact reservation fence synchronous reentry; full environment admission follows the completed observation. */
    const requestAvailable = (): boolean =>
      authority.available() &&
      !this.disposed &&
      !this.unavailable &&
      this.view.state === state &&
      state.field(reorderPreview, false)?.value === this.prepared.preview &&
      this.slot.owns(this) &&
      authority.available();
    /** One Slot fence encloses every exact request predicate at each native checkpoint. */
    const requestCurrent = (): boolean => authority.guard(requestAvailable) && requestAvailable();
    const candidates = this.prepared.whole && this.wholeMedia && !this.carriers;
    const measured = readReorderPassiveGeometry(this.paint, {
      viewport,
      held: this.prepared.held,
      whole: this.prepared.whole
        ? {
            authority: { available: requestAvailable, guard: authority.guard },
            readTargets,
            coverage: candidates ? { kind: 'carrier-candidates', viewport } : { kind: 'complete' },
          }
        : null,
      current: () =>
        this.current() && (!this.prepared.whole || this.prepared.readViewportCurrent()),
    });
    if (!measured) return { kind: 'unavailable' };
    /** Only complete geometry may publish; same-READ completion retains the exact selection over unchanged eligible groups. */
    const consume = (
      measured: ReorderPassiveGeometry,
      issuedPlan?: ReturnType<ReorderMotionSelection>,
    ): PassiveMotionRead => {
      if (measured.coverage === 'carrier-candidates' && !measured.admitted.size) {
        const full = measured.complete();
        return full ? consume(full) : { kind: 'unavailable' };
      }
      const { admitted, geometry, layout } = measured;
      const targetPlan = issuedPlan === undefined ? select(measured.eligible) : issuedPlan;
      const selected = targetPlan?.map((target) => target.id);
      if (
        !this.current() ||
        !selected ||
        selected.length > 64 ||
        new Set(selected).size !== selected.length ||
        selected.some((id) => !admitted.has(id))
      )
        return { kind: 'unavailable' };
      const animated = new Map(
        selected.map((id) => [id, requireValue(admitted.get(id), 'reorder-motion-passive')]),
      );
      const mediaRoots = [
        ...(this.prepared.whole ? this.slot.edges : []),
        ...Array.from((this.prepared.whole ? admitted : animated).values()).flatMap((entry) =>
          !this.prepared.whole ||
          entry.rects.some(
            (rect) =>
              rect.right > viewport.left &&
              rect.left < viewport.right &&
              rect.bottom > viewport.top &&
              rect.top < viewport.bottom,
          )
            ? entry.roots
            : [],
        ),
      ];
      if (!this.hydrated) {
        const plan = this.media.prepare(mediaRoots);
        if (!plan) return { kind: 'unavailable' };
        this.mediaRoots = mediaRoots;
        if (!plan.empty) return { kind: 'media', plan };
        this.hydrated = true;
      }
      if (
        !this.wholeMedia &&
        (!this.mediaRoots ||
          this.mediaRoots.length !== mediaRoots.length ||
          mediaRoots.some(
            (root, index) =>
              root !== requireValue(this.mediaRoots, 'reorder-motion-passive')[index],
          ))
      )
        return { kind: 'unavailable' };
      if (this.prepared.whole && (!layout || !this.current())) return { kind: 'unavailable' };
      const chips = this.prepared.preview.order.order.flatMap((id) => {
        if (id === this.prepared.held) return [];
        const entry = animated.get(id);
        return entry
          ? [
              Object.freeze({
                id,
                rects: Object.freeze(entry.rects),
                left: Math.min(...entry.rects.map((rect) => rect.left)),
                top: Math.min(...entry.rects.map((rect) => rect.top)),
                roots: Object.freeze(
                  entry.roots.map((node) =>
                    Object.freeze({
                      node,
                      scaleX: this.scaleX,
                      scaleY: this.scaleY,
                    }),
                  ),
                ),
              }),
            ]
          : [];
      });
      if (this.prepared.whole && !this.carriers) {
        this.carriers = ReorderMotionPeerCarriers.prepare(
          this.view,
          this.node,
          chips,
          this.scaleX,
          this.scaleY,
          requestCurrent,
        );
        if (!this.carriers || !requestCurrent()) return { kind: 'unavailable' };
      }
      const carrierAction = this.carriers?.read(chips, requestCurrent, readTargets);
      if (carrierAction === 'unavailable') return { kind: 'unavailable' };
      if (carrierAction === 'create' || carrierAction === 'discard')
        return { kind: 'carriers', operation: carrierAction };
      // Hidden carrier creation may precede full rich-budget certification; a Scene always waits for complete observation.
      if (measured.coverage === 'carrier-candidates') {
        const full = measured.complete();
        return full ? consume(full, targetPlan) : { kind: 'unavailable' };
      }
      const pointer = this.view.state.field(reorderPreview, false)?.pointer,
        heldRoot = admitted.get(this.prepared.preview.activeChip)?.roots[0];
      if (
        this.prepared.whole &&
        (!requestCurrent() || !pointer || pointer.mode !== 'stage' || !heldRoot)
      )
        return { kind: 'unavailable' };
      const endpoint = this.prepared.whole
        ? Object.freeze({
            order: this.prepared.preview.order,
            held: this.prepared.preview.activeChip,
            hold: requireValue(pointer, 'reorder-motion-passive').hold,
            viewport: Object.freeze({
              left: viewport.left,
              right: viewport.right,
              top: viewport.top,
              bottom: viewport.bottom,
            }),
            color: requireValue(
              this.view.dom.ownerDocument.defaultView,
              'reorder-motion-passive',
            ).getComputedStyle(requireValue(heldRoot, 'reorder-motion-passive')).outlineColor,
          })
        : null;
      if (this.prepared.whole && !requestCurrent()) return { kind: 'unavailable' };
      const scenePaint = new ReorderScenePaint(chips, this.carriers);
      this.scenePaint = scenePaint;
      const scene: ReorderMotionScene = Object.freeze({
        endpoint,
        ...(this.prepared.whole
          ? {
              whole: true as const,
              layout: requireValue(layout, 'reorder-motion-passive'),
              reveal: () => this.reveal(),
              publishEndpoint: (issue: string) =>
                this.slot.publish(
                  this,
                  () => this.retained(),
                  (current) => {
                    if (current()) this.node.dataset.reorderMotionEndpoint = issue;
                  },
                ),
            }
          : {}),
        node: this.node,
        geometry,
        targets: requireValue(targetPlan, 'reorder-motion-passive'),
        chips: Object.freeze(chips),
        paint: (frame: ReorderMotionFrame) => {
          this.slot.publish(
            this,
            () => this.retained(),
            (current) => {
              scenePaint.paint(frame, current);
              if (!current()) return;
              this.node.dataset.reorderMotionProgress = String(frame.progress);
            },
          );
        },
        destroy: () => this.destroy(),
      });
      if (!this.current() || (this.prepared.whole && !this.prepared.readViewportCurrent()))
        return { kind: 'unavailable' };
      if (!this.prepared.whole) {
        const physical = this.stage.node.getBoundingClientRect();
        this.clip = Object.freeze({
          left: (viewport.left - physical.left) / this.scaleX,
          right: (viewport.right - physical.left) / this.scaleX,
          top: (viewport.top - physical.top) / this.scaleY,
          bottom: (viewport.bottom - physical.top) / this.scaleY,
        });
      }
      if (!this.current() || (this.prepared.whole && !this.prepared.readViewportCurrent()))
        return { kind: 'unavailable' };
      return authority.guard(authority.available)
        ? { kind: 'ready', scene }
        : { kind: 'unavailable' };
    };
    return consume(measured);
  }
  /** WRITE creates or discards prepared carriers under the same physical Slot reservation used by all native publication and retirement. */
  configureCarriers(operation: 'create' | 'discard'): boolean {
    if (!this.current()) return false;
    const carriers = this.carriers;
    if (!carriers) return false;
    this.slot.publish(
      this,
      () => this.publicationCurrent() && this.slot.owns(this),
      (current) => carriers.configure(operation, current),
    );
    return this.current();
  }
  /** WRITE hydrates exactly the viewport-admitted prepared batch; a following READ certifies unchanged complete group geometry. */
  hydrate(plan: ReadonlyMediaPlan): boolean {
    if (!this.current()) return false;
    this.hydrated = plan.hydrate();
    return this.hydrated && this.current();
  }
  /** WRITE applies actual native calibration; a following READ must observe final positioned footprints before admission. */
  place(
    read: Extract<
      PassiveMotionRead,
      {
        kind: 'placement';
      }
    >,
  ): void {
    if (!this.current()) return;
    this.stage.place(this.prepared, read);
  }
  /** WRITE reveals Resource-admitted whole retained paint or exact-current partial paint; carriers retain native shaping. */
  reveal(): void {
    const scene = this.scene;
    if (
      !scene ||
      !(scene.whole ? this.retained() : this.current()) ||
      this.scene !== scene ||
      !this.slot.owns(this)
    )
      throw new Error('Passive motion reveal unavailable');
    this.slot.publish(
      this,
      () => this.scene === scene && this.retained() && (scene.whole || this.publicationCurrent()),
      (current) => {
        if (!this.scenePaint) throw new Error('reorder-scene-paint-unavailable');
        this.scenePaint.reveal(current);
      },
      scene.whole ? null : requireValue(this.clip, 'reorder-motion-passive'),
    );
  }
  /** Retirement clears publication before independently retiring canonical resources and the external tree. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.scene = null;
    const causes: unknown[] = [
      ...this.slot.close(this, !this.prepared.whole, () => {
        const failures: unknown[] = [];
        if (this.scenePaint) failures.push(...this.scenePaint.close(() => this.slot.owns(this)));
        else
          try {
            this.carriers?.destroy();
          } catch (cause) {
            failures.push(cause);
          }
        return failures;
      }),
    ];
    if (causes.length) this.slot.rejectReuse(this);
    try {
      this.stage.release(this);
    } catch (cause) {
      causes.push(cause);
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'Passive motion output retirement failed');
  }
}
