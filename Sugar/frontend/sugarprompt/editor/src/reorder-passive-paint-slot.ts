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
/** One Stage-owned complete canonical mount retains its original line paint and media buffers across finite proposal leases. */
import { requireValue } from '../../core/src/required.js';
import type { EditorView } from '@codemirror/view';
import { Decoration } from '@codemirror/view';
import type { ReorderMarkerRect } from './reorder-geometry.js';
import {
  observeReorderSlotLayout,
  type ReorderLayoutReadAuthority,
} from './reorder-slot-layout-read.js';
import type { ReorderLineLayoutRead } from './reorder-layout-admission.js';
import { ReorderSlotPublication } from './reorder-slot-publication.js';
import {
  preparePassiveWrapperStyles,
  type PreparedPassiveWrapperStyles,
} from './reorder-passive-wrapper-style.js';
import { ReadonlyWidgetMedia, ReadonlyMediaRetired } from './readonly-widget-media.js';
import {
  ReorderWidget,
  type PassiveReorderUpdate,
  type PassiveReorderPaint,
} from './reorder-widget.js';
import {
  canonicalReorderFragment,
  renderReorderFragment,
  releaseReorderFragment,
} from './reorder-projection.js';
import type {
  PreparedPassiveMotion,
  PassiveMotionStage,
  PassiveMotionChild,
} from './reorder-motion-contracts.js';
/** All original canonical resources retire together; media registrations never move between mounts. */
interface PaintResources {
  readonly line: HTMLElement;
  readonly node: HTMLElement;
  readonly paint: PassiveReorderPaint;
  readonly edges: readonly HTMLElement[];
  styles: PreparedPassiveWrapperStyles;
}
/** At most two of these actual mounts, including construction reservations, belong to one Stage. */
export class ReorderPassivePaintSlot {
  private readonly view: EditorView;
  private readonly stage: PassiveMotionStage;
  private readonly media: ReadonlyWidgetMedia;
  private resources: PaintResources | null = null;
  private shell: HTMLElement | null = null;
  private lease: PassiveMotionChild | null = null;
  private transition = false;
  private publication: ReorderSlotPublication | null = null;
  private disposed = false;
  private whole = false;
  private key: string | null = null;
  /** This original callback borrows the exact currently reserved lease even during initial hydration. */
  constructor(view: EditorView, stage: PassiveMotionStage) {
    this.view = view;
    this.stage = stage;
    this.media = new ReadonlyWidgetMedia(() => this.available());
  }
  /** Exact resolved styles/edge bounds supplement Stage's capture, resource and native coordinate authority. */
  private identity(prepared: PreparedPassiveMotion, styles: PreparedPassiveWrapperStyles): string {
    return JSON.stringify([
      prepared.context.key,
      prepared.context.prefix,
      prepared.context.suffix,
      styles.policy,
      styles.contentCss,
      prepared.contentClasses,
      styles.scrollerCss,
      prepared.scrollerClasses,
      styles.lineCss,
    ]);
  }
  /** Reserve publication before any external DOM or media callback can reenter the Stage. */
  reserve(lease: PassiveMotionChild): void {
    if (this.disposed || this.lease || this.transition || this.publication?.busy)
      throw new Error('passive-slot-reservation-unavailable');
    this.lease = lease;
  }
  /** Connected resources remain retained only under their original finite Stage. */
  current(): boolean {
    const resources = this.resources;
    if (!resources || this.disposed || this.publication?.busy) return false;
    const accepted =
      this.stage.current() &&
      resources.paint.current() &&
      resources.node.isConnected &&
      resources.node.parentElement === this.stage.node;
    return accepted && !this.disposed && !this.publication?.busy && this.resources === resources;
  }
  /** Media freshness remains exact to the active lease; retired generations cannot revive completed hydration. */
  private available(expected: PassiveMotionChild | null = this.lease): boolean {
    return (
      !this.disposed &&
      !!expected &&
      this.lease === expected &&
      expected.current() &&
      !this.disposed &&
      this.lease === expected
    );
  }
  /** The caller only accesses paint after a successful complete mount. */
  get paint(): PassiveReorderPaint {
    if (!this.resources) throw new Error('passive-slot-unmounted');
    return this.resources.paint;
  }
  /** The original native containing line belongs to these exact retained canonical resources. */
  get line(): HTMLElement {
    if (!this.resources) throw new Error('passive-slot-unmounted');
    return this.resources.line;
  }
  /** Copied native prefix/suffix retain the same media owner as the canonical value. */
  get edges(): readonly HTMLElement[] {
    if (!this.resources) throw new Error('passive-slot-unmounted');
    return this.resources.edges;
  }
  /** The caller only accesses the native shell after complete construction. */
  get node(): HTMLElement {
    if (!this.shell) throw new Error('passive-slot-unmounted');
    return this.shell;
  }
  /** The exact reserved Slot lends one copied shell for a synchronous mutation-fenced READ, never a cross-request snapshot. */
  readLayout<T>(
    lease: PassiveMotionChild,
    action: (topology: ReorderLineLayoutRead, authority: ReorderLayoutReadAuthority) => T,
  ): T | null {
    if (!this.available(lease)) return null;
    const resources = this.resources,
      publication = this.publication;
    /** Delegated full readiness cannot replace the captured reservation or enter publication. */
    const reserved = (): boolean =>
      this.owns(lease) &&
      this.resources === resources &&
      this.publication === publication &&
      !this.transition &&
      !publication?.busy;
    return observeReorderSlotLayout(
      this.node,
      (phase) => reserved() && (phase === 'observation' || (this.available(lease) && reserved())),
      action,
    );
  }
  /** Only fully hydrated whole-mount plans can retain all their original media registrations for another order. */
  get reusable(): boolean {
    return !this.disposed && !this.publication?.busy && this.whole;
  }
  /** Exact reservation identity remains observable during its pending child's synchronous retirement. */
  owns(lease: PassiveMotionChild): boolean {
    return !this.disposed && this.lease === lease;
  }
  /** Failed retirement output can never reenter the idle pool; Stage retains sole terminal reservation accounting. */
  rejectReuse(lease: PassiveMotionChild): void {
    if (this.owns(lease)) this.whole = false;
  }
  /** Idle admission does not inspect geometry or modify an active accepted mount. */
  get idle(): boolean {
    return !this.disposed && !this.lease && !this.transition && !this.publication?.busy;
  }
  /** Construct canonical paint under the reserved lease, registering all resources before any hydration callback. */
  mount(prepared: PreparedPassiveMotion): boolean {
    if (this.resources || this.shell || this.publication?.busy) return false;
    const view = this.view;
    const doc = view.dom.ownerDocument,
      expected = this.lease;
    let node: HTMLElement | null = null;
    const acquired: HTMLElement[] = [];
    let paint: PassiveReorderPaint | null = null;
    const edges: HTMLElement[] = [];
    let transferred = false;
    /** Local acquisition remains owned until transfer; expectedfalse cleanup is outside the admission catch. */
    const cleanup = (): void => {
      if (transferred) {
        this.destroy();
        return;
      }
      this.disposed = true;
      this.publication?.retire();
      this.lease = null;
      const causes: unknown[] = [];
      for (const release of [
        () => this.media.destroy(),
        () => paint?.destroy(),
        ...edges.map((edge) => () => releaseReorderFragment(edge)),
        ...acquired.map((element) => () => element.remove()),
      ])
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      if (causes.length === 1) throw causes[0];
      if (causes.length)
        throw new AggregateError(causes, 'Passive untransferred retirement failed');
    };
    /** Cheap exact reservation checks never consult viewport geometry during WRITE. */
    const reserved = (): boolean =>
      !this.disposed &&
      !!expected &&
      this.lease === expected &&
      this.stage.current() &&
      expected.publicationCurrent() &&
      !this.disposed &&
      this.lease === expected;
    this.transition = true;
    try {
      try {
        if (!reserved()) return false;
        const styles = preparePassiveWrapperStyles(
          doc,
          prepared.context,
          prepared,
          prepared.whole ? 'whole' : 'partial',
        );
        if (!reserved()) return false;
        node = doc.createElement('span');
        acquired.push(node);
        this.shell = node;
        this.publication = new ReorderSlotPublication(node, this.stage.node);
        const scroller = doc.createElement('div');
        acquired.push(scroller);
        const content = doc.createElement('div');
        acquired.push(content);
        const line = doc.createElement('div');
        acquired.push(line);
        node.className = 'sp-reorder-motion-paint';
        node.inert = true;
        node.setAttribute('aria-hidden', 'true');
        Object.assign(node.style, {
          display: 'block',
          left: '0px',
          top: '0px',
          width: `${prepared.context.width}px`,
          height: 'auto',
          pointerEvents: 'none',
          visibility: prepared.whole ? 'visible' : 'hidden',
          margin: '0',
          padding: '0',
          border: '0',
          transform: 'none',
          zoom: '1',
        });
        this.publication.initialize(reserved);
        scroller.className = prepared.scrollerClasses;
        scroller.style.cssText = styles.scrollerCss;
        content.className = prepared.contentClasses;
        content.contentEditable = 'false';
        content.style.cssText = styles.contentCss;
        line.className = prepared.context.lineClasses;
        line.style.cssText = styles.lineCss;
        node.append(scroller);
        scroller.append(content);
        content.append(line);
        if (reserved()) {
          const canonical = prepared.preview.capture.projection?.visible ?? Decoration.none;
          for (const range of [prepared.context.prefix, prepared.context.suffix]) {
            if (!reserved()) break;
            const edge = doc.createElement('span');
            edges.push(edge);
            renderReorderFragment(
              view,
              edge,
              (part) => view.state.doc.sliceString(part.from, part.to),
              range,
              canonical === Decoration.none
                ? canonical
                : canonicalReorderFragment(view.state, canonical, range),
              null,
              this.media,
            );
          }
          if (edges.length === 2 && reserved()) {
            paint = new ReorderWidget(prepared.preview, null).mountPassive(view, this.media);
            if (reserved())
              line.append(
                requireValue(edges[0], 'reorder-passive-paint-slot'),
                paint.node,
                requireValue(edges[1], 'reorder-passive-paint-slot'),
              );
            if (reserved()) this.stage.node.append(node);
            if (reserved()) {
              this.resources = { node, line, paint, edges, styles };
              transferred = true;
            }
          }
        }
      } catch (primary) {
        const cancelled =
          primary instanceof ReadonlyMediaRetired && (this.disposed || this.lease !== expected);
        try {
          cleanup();
        } catch (retirement) {
          throw new AggregateError(
            [primary, retirement],
            'Passive slot construction/retirement failed',
          );
        }
        if (cancelled) return false;
        throw primary;
      }
      let admitted: boolean;
      try {
        admitted = transferred && this.available(expected);
      } catch (primary) {
        try {
          cleanup();
        } catch (retirement) {
          throw new AggregateError(
            [primary, retirement],
            'Passive slot admission/retirement failed',
          );
        }
        throw primary;
      }
      if (!admitted) {
        cleanup();
        return false;
      }
      return true;
    } finally {
      this.transition = false;
    }
  }
  /** Reconcile complete canonical output with bounded projection-empty fragment creation; resource-bearing misses leave old paint untouched for terminal discard. */
  reconcile(prepared: PreparedPassiveMotion): PassiveReorderUpdate {
    const expected = this.lease,
      resources = this.resources;
    if (
      this.disposed ||
      this.transition ||
      this.publication?.busy === true ||
      !expected ||
      !resources ||
      !this.available(expected)
    )
      return 'retired';
    this.transition = true;
    try {
      if (!this.whole) return 'miss';
      if (resources.node.parentElement !== this.stage.node) return 'retired';
      if (!this.available(expected) || this.resources !== resources) return 'retired';
      const styles = preparePassiveWrapperStyles(
        this.view.dom.ownerDocument,
        prepared.context,
        prepared,
        prepared.whole ? 'whole' : 'partial',
        resources.styles,
      );
      if (!this.available(expected) || this.resources !== resources) return 'retired';
      if (this.key !== this.identity(prepared, styles)) return 'miss';
      const result = this.paint.reconcile(prepared.preview);
      if (result !== 'ready') return result;
      if (
        this.disposed ||
        this.lease !== expected ||
        !expected.publicationCurrent() ||
        this.disposed ||
        this.lease !== expected
      )
        return 'retired';
      // Complete proposals share one calibrated plane while accepted and pending resources coexist.
      this.node.style.left = '0px';
      if (this.disposed || this.lease !== expected) return 'retired';
      this.node.style.top = '0px';
      if (this.disposed || this.lease !== expected) return 'retired';
      // The idle clip stays in place through fresh native geometry; only reveal admits visible output.
      if (this.node.parentElement !== this.stage.node) return 'retired';
      if (!this.available(expected)) return 'retired';
      this.paint.invalidateMeasurement();
      if (!this.available(expected) || this.resources !== resources) return 'retired';
      resources.styles = styles;
      return 'ready';
    } finally {
      this.transition = false;
    }
  }
  /** First whole-fit hydration retains the existing per-owner admission and unchanged viewport fallback semantics. */
  preflight(prepared: PreparedPassiveMotion): 'whole' | 'deferred' | 'retired' {
    const expected = this.lease,
      resources = this.resources;
    if (!resources) return 'retired';
    const result = this.media.preflightAll();
    if (result.kind === 'deferred') return 'deferred';
    if (result.kind === 'retired' || !result.plan.hydrate()) return 'retired';
    if (!this.available(expected) || this.resources !== resources) return 'retired';
    this.whole = true;
    this.key = this.identity(prepared, resources.styles);
    return 'whole';
  }
  /** Viewport-only fallback retains the original one-plan media owner and is never eligible for recycling. */
  get viewportMedia(): ReadonlyWidgetMedia {
    return this.media;
  }
  /** Delegate physical publication while retaining the sole exact lease/resource and failure-retirement authority. */
  publish(
    lease: PassiveMotionChild,
    admitted: () => boolean,
    write: (current: () => boolean) => void,
    clip?: ReorderMarkerRect | null,
  ): void {
    const publication = this.publication;
    if (!this.owns(lease) || this.transition) return;
    publication?.write(
      () => this.owns(lease),
      admitted,
      write,
      clip,
      () => {
        this.rejectReuse(lease);
        this.stage.release(lease);
      },
    );
  }
  /** Close pixels and independent root resets under the same physical write reservation before Stage may recycle resources. */
  close(
    lease: PassiveMotionChild,
    partial: boolean,
    resetRoots: () => readonly unknown[],
  ): readonly unknown[] {
    const failures =
      this.publication?.close(() => this.owns(lease), partial, resetRoots) ?? resetRoots();
    if (failures.length) this.rejectReuse(lease);
    return failures;
  }
  /** Physically closed idle paint retains its original connected canonical resources under the finite Stage. */
  release(lease: PassiveMotionChild): boolean {
    if (this.lease !== lease) return false;
    this.lease = null;
    if (!this.reusable || this.transition || this.publication?.busy) return false;
    this.transition = true;
    try {
      const resources = this.resources,
        retained = this.current();
      if (!retained || !resources || this.disposed || this.lease) return false;
      return !this.disposed && !this.lease && this.resources === resources;
    } finally {
      this.transition = false;
    }
  }
  /** Terminal retirement detaches its exclusively owned shell before independent resource cleanup, retaining raw causes in operation order. */
  destroy(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.publication?.retire();
    this.lease = null;
    this.whole = false;
    const resources = this.resources;
    this.resources = null;
    const causes: unknown[] = [];
    for (const release of [
      () => resources?.node.remove(),
      () => this.media.destroy(),
      () => resources?.paint.destroy(),
      ...(resources?.edges ?? []).map((edge) => () => releaseReorderFragment(edge)),
    ])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'Passive slot retirement failed');
  }
}
