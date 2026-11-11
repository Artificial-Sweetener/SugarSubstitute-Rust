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
/** One whole Scene owns complete native atomic copies; canonical inline output remains its flow and geometry authority. */
import { requireValue } from '../../core/src/required.js';
import {
  withReorderPaintMaterialization,
  type ReorderPaintMaterialization,
} from './reorder-paint-materialization.js';
import type { EditorView } from '@codemirror/view';
import { materializeReorderGrab } from './reorder-grab-materialization.js';
import type { ReorderGrabPaint } from './reorder-grab-contracts.js';
import type { PreparedReorderPaint } from './reorder-paint-preparation.js';
import { admitNativeReorderPaint } from './reorder-native-paint.js';
import type { admitReorderTargetLayout } from './reorder-layout-admission.js';
import { nativeReorderFootprint } from './reorder-native-footprint.js';
import {
  prepareReorderPeerCarriers,
  type ReorderCarrierSource as PeerSource,
  type ReorderPeerCopyPlan,
} from './reorder-peer-carrier-plan.js';
/** A candidate borrows the original prepared Scene; no second target or semantic index is introduced. */
/** Exact original own declarations are restored independently after each attempted publication. */
interface FlowPaint {
  readonly style: CSSStyleDeclaration;
  readonly opacity: string;
  readonly priority: string;
}
/** One complete single-root atomic peer uses the same native copy recipe as an admitted press fragment. */
interface PeerPlan extends ReorderPeerCopyPlan {
  paint: ReorderGrabPaint | null;
  flow: FlowPaint | null;
  attempted: boolean;
}
/** Complete atomic copies share one aggregate native admission and one finite Scene publication lifetime. */
export class ReorderMotionPeerCarriers {
  private readonly view: EditorView;
  private phase:
    | 'prepared'
    | 'instantiated'
    | 'validated'
    | 'published'
    | 'refused'
    | 'retired-carriers'
    | 'retired-canonical';
  private readonly peers: readonly PeerPlan[];
  private readonly prepared: PreparedReorderPaint | null;
  private readonly parent: HTMLElement;
  private readonly left: number;
  private readonly top: number;
  private readonly scaleX: number;
  private readonly scaleY: number;
  /** Native preparation is complete before this owner can allocate or suppress any original output. */
  private constructor(
    view: EditorView,
    parent: HTMLElement,
    prepared: PreparedReorderPaint | null,
    peers: readonly PeerPlan[],
    placement: {
      readonly left: number;
      readonly top: number;
      readonly scaleX: number;
      readonly scaleY: number;
    },
  ) {
    this.phase = prepared ? 'prepared' : 'refused';
    this.view = view;
    this.parent = parent;
    this.prepared = prepared;
    this.peers = peers;
    this.left = placement.left;
    this.top = placement.top;
    this.scaleX = placement.scaleX;
    this.scaleY = placement.scaleY;
  }
  /** READ delegates one immutable native plan; this owner alone retains create/validate/refusal/publication phases. */
  static prepare(
    view: EditorView,
    parent: HTMLElement,
    chips: readonly PeerSource[],
    scaleX: number,
    scaleY: number,
    current: () => boolean,
  ): ReorderMotionPeerCarriers | null {
    const plan = prepareReorderPeerCarriers(view, parent, chips, scaleX, scaleY, current);
    if (plan.kind === 'unavailable') return null;
    return plan.kind === 'refused'
      ? new ReorderMotionPeerCarriers(view, parent, null, [], {
          left: 0,
          top: 0,
          scaleX,
          scaleY,
        })
      : new ReorderMotionPeerCarriers(
          view,
          parent,
          plan.prepared,
          plan.peers.map((peer) => ({
            ...peer,
            paint: null,
            flow: null,
            attempted: false,
          })),
          plan.placement,
        );
  }
  /** Phase authority remains in this finite owner; target admission is borrowed only for this invocation, never retained. */
  read(
    chips: readonly PeerSource[],
    current: () => boolean,
    readTargets: typeof admitReorderTargetLayout,
  ): 'ready' | 'create' | 'discard' | 'unavailable' {
    if (!this.live(current)) return 'unavailable';
    if (this.phase === 'prepared') return 'create';
    if (this.phase === 'instantiated') {
      if (!this.matches(chips) || !this.validate(current, readTargets))
        return this.live(current) ? 'discard' : 'unavailable';
      this.phase = 'validated';
    }
    return 'ready';
  }
  /** An extra copied-shell budget refusal discards the optimization before retrying unchanged canonical output. */
  shellRefused(): boolean {
    return this.phase === 'instantiated';
  }
  /** The caller reserves its existing Slot while this owner creates or independently discards every copy. */
  configure(operation: 'create' | 'discard', current: () => boolean): void {
    if (operation === 'create' && this.instantiate(current)) return;
    const causes = [...this.restore(), ...this.removeCopies()];
    if (!this.retired) this.phase = 'refused';
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'reorder-peer-carrier-refusal-failed');
  }
  /** Retirement is independent of whether this request ever produced visible carrier paint. */
  private get retired(): boolean {
    return this.phase === 'retired-carriers' || this.phase === 'retired-canonical';
  }
  /** Delegated admission cannot revive this finite owner after synchronous retirement; raw callback failures remain observable. */
  private live(current: () => boolean): boolean {
    return !this.retired && current() && !this.retired;
  }
  /** WRITE creates hidden complete copies only; original flow and visible paint remain untouched until revalidation. */
  instantiate(current: () => boolean): boolean {
    let complete = false;
    const causes: unknown[] = [];
    try {
      const created = withReorderPaintMaterialization(
        this.view.dom.ownerDocument,
        (materialization) => this.create(current, materialization),
        () => {
          const failures = this.removeCopies();
          if (failures.length === 1) throw failures[0];
          if (failures.length)
            throw new AggregateError(failures, 'reorder-peer-carrier-transfer-failed');
        },
      );
      complete =
        created && this.phase === 'prepared' && this.live(current) && this.phase === 'prepared';
      if (complete) this.phase = 'instantiated';
    } catch (cause) {
      causes.push(cause);
    }
    if (!complete) causes.push(...this.removeCopies());
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'reorder-peer-carrier-creation-failed');
    return complete;
  }
  /** Captured outputs transfer before native attachment; the outer creation boundary retries removal after late append delegates. */
  private create(current: () => boolean, materialization: ReorderPaintMaterialization): boolean {
    if (this.phase !== 'prepared' || !this.live(current)) return false;
    for (const peer of this.peers) {
      const paint = materializeReorderGrab(
        this.view,
        materialization,
        peer.geometry,
        requireValue(this.prepared, 'reorder-motion-peer-carriers'),
        [peer.copy],
        [peer.clip],
        {
          kind: 'stage',
          left: this.left,
          top: this.top,
          scaleX: this.scaleX,
          scaleY: this.scaleY,
        },
      );
      let admitted: boolean;
      try {
        admitted = !!paint && this.live(current);
      } catch (primary) {
        try {
          paint?.node.remove();
        } catch (cleanup) {
          throw new AggregateError(
            [primary, cleanup],
            'reorder-peer-carrier-late-acquisition-failed',
          );
        }
        throw primary;
      }
      if (!admitted) {
        paint?.node.remove();
        return false;
      }
      peer.paint = paint;
      if (!paint) return false;
      paint.node.style.setProperty('opacity', '0', 'important');
      if (!this.live(current)) return false;
      paint.node.dataset.reorderMotionCarrier = String(peer.source.id);
      if (!this.live(current)) return false;
      const copy = paint.node.firstElementChild as HTMLElement | null;
      if (!copy || !this.live(current)) return false;
      copy.dataset.reorderChip = String(peer.source.id);
      if (!this.live(current)) return false;
      this.parent.append(paint.node);
      if (!this.live(current)) return false;
    }
    return true;
  }
  /** READ accepts only actual complete copied fragments identical to the original endpoint; missing or expired request topology refuses without fresh discovery. */
  validate(current: () => boolean, readTargets: typeof admitReorderTargetLayout): boolean {
    if (this.phase !== 'instantiated' || !this.live(current)) return false;
    for (const peer of this.peers) {
      const root = peer.paint?.node.firstElementChild as HTMLElement | null;
      if (!root || !this.live(current)) return false;
      const painted = admitNativeReorderPaint([root]);
      if (!painted || !this.live(current)) return false;
      const nodes = readTargets([
        {
          node: root,
          kind: 'chip',
          index: peer.source.id,
          leases: [],
          origins: [],
        },
      ])?.get(root);
      if (!nodes || !this.live(current)) return false;
      const actual = nativeReorderFootprint(root, nodes, peer.source.id, undefined, {
        current: () => this.live(current),
      });
      if (!actual || !this.live(current) || actual.length !== peer.source.rects.length)
        return false;
      for (let index = 0; index < actual.length; index++) {
        const expected = requireValue(peer.source.rects[index], 'reorder-motion-peer-carriers'),
          found = requireValue(actual[index], 'reorder-motion-peer-carriers');
        if (
          found.left !== expected.left ||
          found.right !== expected.right ||
          found.top !== expected.top ||
          found.bottom !== expected.bottom
        )
          return false;
      }
    }
    return this.live(current);
  }
  /** WRITE runs within the Slot reservation; failed or late native suppression is restored before that reservation can lend its physical roots. */
  reveal(current: () => boolean): boolean {
    if (this.phase === 'refused') return this.live(current);
    if (this.phase !== 'validated' || !this.live(current)) return false;
    let complete = false;
    const causes: unknown[] = [];
    try {
      complete = this.publish(current);
    } catch (cause) {
      causes.push(cause);
    }
    if (!complete) causes.push(...this.restore());
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'reorder-peer-carrier-publication-failed');
    if (complete) this.phase = 'published';
    return complete;
  }
  /** Each attempted root is registered before its first native write, including setters that retire then resume. */
  private publish(current: () => boolean): boolean {
    for (const peer of this.peers) {
      const style = peer.root.style;
      if (!this.live(current)) return false;
      const opacity = style.getPropertyValue('opacity');
      if (!this.live(current)) return false;
      const priority = style.getPropertyPriority('opacity');
      if (!this.live(current)) return false;
      if (peer.root.getAttribute('data-reorder-motion-flow') !== null || !this.live(current))
        return false;
      peer.flow = { style, opacity, priority };
      peer.attempted = true;
      style.setProperty('opacity', '0', 'important');
      if (!this.live(current)) return false;
      peer.root.dataset.reorderMotionFlow = String(peer.source.id);
      if (!this.live(current)) return false;
      const copy = requireValue(peer.paint, 'reorder-motion-peer-carriers').node
        .firstElementChild as HTMLElement | null;
      if (!copy || !this.live(current)) return false;
      copy.dataset.reorderMotionChip = String(peer.source.id);
      if (!this.live(current)) return false;
      requireValue(peer.paint, 'reorder-motion-peer-carriers').node.style.setProperty(
        'opacity',
        '1',
        'important',
      );
      if (!this.live(current)) return false;
    }
    return true;
  }
  /** Independently retry every attempted restoration, including after a delegated retirement has already returned. */
  private restore(): readonly unknown[] {
    const causes: unknown[] = [];
    for (const peer of this.peers)
      for (const restore of [
        () => peer.paint?.node.style.setProperty('opacity', '0', 'important'),
        () => {
          if (peer.attempted && peer.flow)
            peer.flow.style.setProperty('opacity', peer.flow.opacity, peer.flow.priority);
        },
        () => {
          if (peer.attempted) delete peer.root.dataset.reorderMotionFlow;
        },
      ])
        try {
          restore();
        } catch (cause) {
          causes.push(cause);
        }
    return causes;
  }
  /** Only exact admitted peers consume transform motion; all unsupported groups retain the existing canonical offset writer. */
  paint(id: number, left: number, top: number, current: () => boolean): boolean {
    if (!this.handles(id)) return false;
    const peer = this.peers.find((entry) => entry.source.id === id);
    if (!peer) return false;
    if (this.phase !== 'published' || !this.live(current)) return true;
    requireValue(peer.paint, 'reorder-motion-peer-carriers').node.style.transform =
      `translate(${(left - peer.source.left) / this.scaleX}px, ${(top - peer.source.top) / this.scaleY}px)`;
    this.live(current);
    return true;
  }
  /** The next canonical READ must reproduce the exact original source identities and complete endpoint geometry. */
  matches(chips: readonly PeerSource[]): boolean {
    return this.peers.every((peer) => {
      const chip = chips.find((candidate) => candidate.id === peer.source.id);
      return (
        chip?.roots.length === 1 &&
        requireValue(chip.roots[0], 'reorder-motion-peer-carriers').node === peer.root &&
        chip.rects.length === peer.source.rects.length &&
        chip.rects.every((rect, index) => {
          const expected = requireValue(peer.source.rects[index], 'reorder-motion-peer-carriers');
          return (
            rect.left === expected.left &&
            rect.right === expected.right &&
            rect.top === expected.top &&
            rect.bottom === expected.bottom
          );
        })
      );
    });
  }
  /** Capture still-owned peer identity without introducing another target roster or geometry authority. */
  handles(id: number): boolean {
    return (
      (this.phase === 'validated' ||
        this.phase === 'published' ||
        this.phase === 'retired-carriers') &&
      this.peers.some((peer) => peer.source.id === id)
    );
  }
  /** Every captured copy remains removable after retirement so resumed native append delegates cannot resurrect it. */
  private removeCopies(): readonly unknown[] {
    const causes: unknown[] = [];
    for (const peer of this.peers)
      try {
        peer.paint?.node.remove();
      } catch (cause) {
        causes.push(cause);
      }
    return causes;
  }
  /** Close every copy before restoring every attempted original declaration; failures retain order and never skip later cleanup. */
  destroy(): void {
    if (this.retired) return;
    this.phase =
      this.phase === 'validated' || this.phase === 'published'
        ? 'retired-carriers'
        : 'retired-canonical';
    const causes: unknown[] = [...this.restore(), ...this.removeCopies()];
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'reorder-peer-carrier-retirement-failed');
  }
}
