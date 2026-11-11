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
/** One real external coordinate resource reuses native calibration across same-context readonly publications. */
import { attachReorderMotionStage, type ReorderMotionLayerLease } from './reorder-motion-layer.js';
import type { EditorView } from '@codemirror/view';
import type {
  PreparedPassiveMotion,
  PassiveMotionRead,
  PassiveMotionStage,
  PassiveMotionChild,
} from './reorder-motion-contracts.js';
import { ReorderPassivePaintSlot } from './reorder-passive-paint-slot.js';
import { reorderPreview } from './reorder-preview.js';
import { currentNativeReorder } from './reorder-command.js';
import { reorderPaintIssueName } from './reorder-paint-identity.js';
import { PendingPassiveMotion } from './reorder-motion-passive.js';
import { matchesMotionViewAppearance } from './reorder-passive-context.js';

/** One actual calibrated coordinate root is reused across the same gesture/context; proposals never allocate a new calibration root. */
export class ReorderPassiveStage implements PassiveMotionStage {
  readonly node: HTMLElement;
  private readonly style: HTMLElement;
  private layerLease: ReorderMotionLayerLease | null = null;
  private readonly view: EditorView;
  private readonly capture: PreparedPassiveMotion['preview']['capture'];
  private readonly token: PreparedPassiveMotion['preview']['token'];
  private readonly attributes: readonly (readonly [string, string])[];
  private readonly positionKey: string;
  private readonly appearanceCurrent: () => boolean;
  private readonly pending = new Map<PassiveMotionChild, ReorderPassivePaintSlot>();
  private readonly slots = new Set<ReorderPassivePaintSlot>();
  private disposed = false;
  private calibration: {
    readonly scaleX: number;
    readonly scaleY: number;
  } | null = null;
  /** One finite native root supplies actual scale/translation output before any proposal can mask source paint. */
  constructor(view: EditorView, prepared: PreparedPassiveMotion) {
    this.view = view;
    this.capture = prepared.preview.capture;
    this.token = prepared.preview.token;
    this.attributes = prepared.appearance;
    this.appearanceCurrent = prepared.context.appearanceCurrent;
    this.positionKey = prepared.context.key;
    const node = view.dom.ownerDocument.createElement('span');
    this.node = node;
    let style: HTMLElement | null = null;
    try {
      const issue = reorderPaintIssueName(this);
      style = view.dom.ownerDocument.createElement('style');
      this.style = style;
      node.setAttribute('data-reorder-motion-stage', issue);
      style.textContent = `[data-reorder-motion-stage="${issue}"],[data-reorder-motion-stage="${issue}"] *{animation:none!important;transition:none!important;pointer-events:none!important;-webkit-user-modify:read-only!important;user-select:none!important}`;
      // CodeMirror exposes the shared root interface; its supported native roots are Document and ShadowRoot.
      const root = view.root as Document | ShadowRoot;
      if ('head' in root) root.head.append(style);
      else root.append(style);
      node.className = 'sp-reorder-motion-stage';
      node.inert = true;
      node.setAttribute('aria-hidden', 'true');
      Object.assign(node.style, {
        position: 'absolute',
        left: '0px',
        top: '0px',
        width: `${prepared.context.width}px`,
        height: '256px',
        pointerEvents: 'none',
        zIndex: '99',
        visibility: 'hidden',
        // The fixed calibration box isolates descendant layout while complete inline paint may overflow its height.
        contain: 'layout size',
        overflow: 'visible',
        margin: '0',
        padding: '0',
        border: '0',
        transform: 'none',
        transformOrigin: '0 0',
        zoom: '1',
      });
      this.layerLease = attachReorderMotionStage(view, node, prepared.context.width, () =>
        this.destroy(),
      );
      if (!this.current()) throw new Error('Passive stage retired during creation');
    } catch (primary) {
      const causes: unknown[] = [primary];
      for (const release of [
        () => {
          const lease = this.layerLease;
          this.layerLease = null;
          lease?.release();
        },
        () => style?.remove(),
        () => node.remove(),
      ])
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      if (causes.length === 1) throw primary;
      throw new AggregateError(causes, 'Passive stage construction/retirement failed');
    }
  }
  /** Root/source appearance and the exact native clip lease survive later readonly publications. */
  current(): boolean {
    const preview = this.view.state.field(reorderPreview, false)?.value;
    if (
      this.disposed ||
      !this.appearanceCurrent() ||
      !this.node.isConnected ||
      preview?.capture !== this.capture ||
      preview.token !== this.token ||
      !currentNativeReorder(this.view.state, this.capture)
    )
      return false;
    const lease = this.layerLease;
    if (!matchesMotionViewAppearance(this.view.dom, this.attributes)) return false;
    return (
      !this.disposed &&
      lease !== null &&
      this.layerLease === lease &&
      lease.current() &&
      !this.disposed &&
      this.layerLease === lease
    );
  }
  /** Native clip draw must be followed by exact original-extent admission before calibration or reveal. */
  readLayer(): 'waiting' | 'ready' | 'unavailable' {
    return this.layerLease?.read() ?? 'unavailable';
  }
  /** Exact actual wrapper coordinate context, not a class-only key, admits calibration reuse. */
  matches(prepared: PreparedPassiveMotion): boolean {
    return (
      this.current() &&
      prepared.preview.capture === this.capture &&
      prepared.preview.token === this.token &&
      prepared.context.key === this.positionKey
    );
  }
  /** The gesture coordinate resource can calibrate independently of a superseded proposal child, in the existing native READ phase. */
  readPlacement(
    prepared: PreparedPassiveMotion,
  ): Extract<PassiveMotionRead, { kind: 'placement' }> | null {
    if (this.calibration || !this.matches(prepared) || this.readLayer() !== 'ready') return null;
    const box = this.node.getBoundingClientRect(),
      scaleX = box.width / prepared.context.width,
      scaleY = box.height / 256;
    return [box.left, box.top, scaleX, scaleY].every(Number.isFinite) && scaleX > 0 && scaleY > 0
      ? { kind: 'placement', left: box.left, top: box.top, scaleX, scaleY }
      : null;
  }
  /** Borrow the exact native scale output; no new calibration is needed for another same-context order. */
  measured(): { readonly scaleX: number; readonly scaleY: number } | null {
    return this.calibration;
  }
  /** Register complete canonical paint only while the stage remains current; expected retirement returns null after independently owned child cleanup. */
  mount(prepared: PreparedPassiveMotion): PendingPassiveMotion | null {
    if (!this.matches(prepared)) return null;
    const idle = Array.from(this.slots).find((slot) => slot.idle);
    if (!idle && this.slots.size >= 2) return null;
    const slot = idle ?? new ReorderPassivePaintSlot(this.view, this);
    // Reservation counts before the first external construction callback, including all in-flight mounts.
    this.slots.add(slot);
    const pending = new PendingPassiveMotion(this.view, prepared, this, slot);
    this.pending.set(pending, slot);
    slot.reserve(pending);
    try {
      const outcome = idle ? slot.reconcile(prepared) : slot.mount(prepared) ? 'ready' : 'retired';
      if (outcome !== 'ready') {
        const retry =
          outcome === 'miss' && this.pending.get(pending) === slot && pending.publicationCurrent();
        this.discard(pending, slot);
        return retry &&
          this.matches(prepared) &&
          this.view.state.field(reorderPreview, false)?.value === prepared.preview
          ? this.mount(prepared)
          : null;
      }
      const admitted = this.current() && pending.initialize(!!idle);
      if (!admitted) {
        this.discard(pending, slot);
        return null;
      }
      return pending;
    } catch (primary) {
      try {
        this.discard(pending, slot);
      } catch (cleanup) {
        throw new AggregateError([primary, cleanup], 'Passive child admission/retirement failed');
      }
      throw primary;
    }
  }
  /** Retire the exact proposal before cleanup while its buffer reservation remains counted until successful destruction. */
  private discard(pending: PendingPassiveMotion, slot: ReorderPassivePaintSlot): void {
    if (this.pending.get(pending) !== slot) return;
    this.pending.delete(pending);
    const causes: unknown[] = [];
    for (const release of [() => pending.destroy(), () => this.retireSlot(slot)])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'Passive slot discard failed');
  }
  /** Return only the exact retired generation; both acquired slots retain complete idle output until context retirement within the unchanged two-slot cap. */
  release(pending: PassiveMotionChild): void {
    const slot = this.pending.get(pending);
    this.pending.delete(pending);
    if (!slot) return;
    let retained = false;
    try {
      retained = !this.disposed && this.current() && slot.release(pending);
    } catch (primary) {
      try {
        this.retireSlot(slot);
      } catch (cleanup) {
        throw new AggregateError([primary, cleanup], 'Passive slot return failed');
      }
      throw primary;
    }
    if (!retained) {
      this.retireSlot(slot);
      return;
    }
  }
  /** Retiring buffers remain counted through all external cleanup callbacks; failed retirement permanently occupies its reservation. */
  private retireSlot(slot: ReorderPassivePaintSlot): void {
    slot.destroy();
    this.slots.delete(slot);
  }
  /** WRITE applies one actual native placement result; every later order reuses this same connected root/output. */
  place(
    prepared: PreparedPassiveMotion,
    read: Extract<PassiveMotionRead, { kind: 'placement' }>,
  ): void {
    if (!this.matches(prepared)) return;
    this.node.style.left = `${(prepared.context.left - read.left) / read.scaleX}px`;
    this.node.style.top = `${(prepared.context.top - read.top) / read.scaleY}px`;
    this.node.style.setProperty(
      'transform',
      prepared.context.scaleX === read.scaleX && prepared.context.scaleY === read.scaleY
        ? 'none'
        : `scale(${prepared.context.scaleX / read.scaleX},${prepared.context.scaleY / read.scaleY})`,
      'important',
    );
    this.calibration = Object.freeze({
      scaleX: prepared.context.scaleX,
      scaleY: prepared.context.scaleY,
    });
  }
  /** Context/session retirement releases all proposal resources and root, preserving raw ordered causes. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    const layerLease = this.layerLease;
    this.layerLease = null;
    this.calibration = null;
    const children = [...this.pending.keys()],
      slots = [...this.slots];
    this.pending.clear();
    this.slots.clear();
    const causes: unknown[] = [];
    for (const release of [
      () => layerLease?.release(),
      ...children.map((child) => () => child.destroy()),
      ...slots.map((slot) => () => slot.destroy()),
      () => this.style.remove(),
      () => this.node.remove(),
    ])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'Passive stage retirement failed');
  }
}
