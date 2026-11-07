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
/** Observe one web-owned surface allocation and its conservative native clip. */

import { HostAncestryObserver } from '../../core/HostAncestryObserver.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import type { CanvasFrameClock } from '../application/CanvasCameraTransition.js';
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import { canvasGeometryAncestry, canvasVisibleClip } from './CanvasVisibleClip.js';

/**
 * Contribute host input-routing members to measured geometry.
 *
 * Only a host that routes native gestures receives these members, so an observer without a
 * routing source keeps the exact geometry shape older hosts accept.
 */
export interface CanvasSurfaceRouting {
  /** Request remeasurement whenever the routing members may have changed; returns its release. */
  observe(changed: () => void): () => void;
  /** Measure occlusions and the modal flag against the viewport allocation. */
  read(
    origin: CanvasSurfaceGeometry['origin'],
    viewport: CanvasSurfaceGeometry['viewport'],
  ): Pick<CanvasSurfaceGeometry, 'occlusions' | 'modal'>;
}

/** Contribute independently observed native material regions without granting input routing. */
export interface CanvasSurfaceBackdrops {
  /** Subscribe to material layout changes and return their owned teardown. */
  observe(changed: () => void): () => void;
  /** Withdraw previously measured material before allocation or density changes. */
  invalidate(): void;
  /** Read immutable viewport-relative CSS regions independently of gesture routing. */
  read(
    origin: CanvasSurfaceGeometry['origin'],
    viewport: CanvasSurfaceGeometry['viewport'],
  ): NonNullable<CanvasSurfaceGeometry['backdrops']>;
}

/** Coalesce layout, visibility, scrolling, and density changes within the attachment's document. */
export class CanvasSurfaceGeometryObserver {
  readonly #resize: ResizeObserver | null;
  readonly #ancestry: HostAncestryObserver;
  #presence: HostSurfaceObserver | null = null;
  readonly #stops: (() => void)[] = [];
  readonly #ancestryStops: (() => void)[] = [];
  #ancestors: readonly Element[] = [];
  #roots: readonly ShadowRoot[] = [];
  #media: (() => void) | null = null;
  #frame: (() => void) | null = null;
  #disposed = false;

  /** Bind explicit document/window geometry and release observer/listener ownership together. */
  constructor(
    private readonly element: HTMLElement,
    private readonly frames: CanvasFrameClock,
    private readonly changed: (geometry: CanvasSurfaceGeometry) => void,
    private readonly routing: CanvasSurfaceRouting | null = null,
    private readonly backdrops: CanvasSurfaceBackdrops | null = null,
  ) {
    const document = element.ownerDocument;
    const window = document.defaultView;
    if (!window) throw new Error('Canvas document window is unavailable.');
    if (!element.hasAttribute('data-sc-canvas-geometry'))
      throw new Error('Canvas geometry surface is not identified.');
    const Resize = window.ResizeObserver;
    this.#resize = Resize ? new Resize(() => this.#schedule()) : null;
    this.#ancestry = new HostAncestryObserver(window, () => {
      this.#observeAncestry();
      this.#schedule();
    }, ['data-canvas-motion']);
    this.#observeAncestry();
    const schedule = () => this.#schedule();
    const layout = () => {
      this.backdrops?.invalidate();
      this.#schedule();
    };
    window.addEventListener('resize', layout);
    window.addEventListener('scroll', layout);
    document.addEventListener('scroll', layout, true);
    document.addEventListener('visibilitychange', layout);
    window.visualViewport?.addEventListener('resize', layout);
    window.visualViewport?.addEventListener('scroll', layout);
    element.addEventListener('focusin', schedule);
    element.addEventListener('focusout', schedule);
    this.#stops.push(
      () => window.removeEventListener('resize', layout),
      () => window.removeEventListener('scroll', layout),
      () => document.removeEventListener('scroll', layout, true),
      () => document.removeEventListener('visibilitychange', layout),
      () => window.visualViewport?.removeEventListener('resize', layout),
      () => window.visualViewport?.removeEventListener('scroll', layout),
      () => element.removeEventListener('focusin', schedule),
      () => element.removeEventListener('focusout', schedule),
    );
    if (routing) this.#stops.push(routing.observe(schedule));
    if (backdrops) this.#stops.push(backdrops.observe(schedule));
    this.#density();
  }

  /** Read CSS allocation and display density without traversing the workflow graph. */
  read(): CanvasSurfaceGeometry {
    const bounds = this.element.getBoundingClientRect();
    const document = this.element.ownerDocument;
    const density = document.defaultView?.devicePixelRatio ?? 1;
    const valid =
      [bounds.left, bounds.top, bounds.right, bounds.bottom, bounds.width, bounds.height].every(
        (value) => Number.isFinite(value) && Math.abs(value) <= 1_000_000,
      ) &&
      bounds.width >= 0 &&
      bounds.height >= 0 &&
      bounds.right >= bounds.left &&
      bounds.bottom >= bounds.top &&
      Math.abs(bounds.right - bounds.left - bounds.width) <= 0.000_001 &&
      Math.abs(bounds.bottom - bounds.top - bounds.height) <= 0.000_001 &&
      Number.isFinite(density) &&
      density > 0 &&
      density <= 64;
    const measured = valid ? canvasVisibleClip(this.element, bounds) : null;
    const origin = Object.freeze({ x: valid ? bounds.left : 0, y: valid ? bounds.top : 0 });
    const viewport = Object.freeze({
      width: valid ? bounds.width : 0,
      height: valid ? bounds.height : 0,
    });
    const routing = this.routing?.read(origin, viewport);
    const backdrops = this.backdrops?.read(origin, viewport);
    return Object.freeze({
      coordinateSpace: 'client',
      units: 'css-pixels',
      origin,
      viewport,
      clip: measured?.clip ?? Object.freeze({ x: 0, y: 0, width: 0, height: 0 }),
      physicalPixelsPerCssPixel: valid ? density : 1,
      visible: measured?.visible ?? false,
      focused: this.element.contains(document.activeElement),
      ...(backdrops
        ? { backdrops: Object.freeze(backdrops.map((region) => Object.freeze({ ...region }))) }
        : {}),
      ...(routing
        ? {
            occlusions: Object.freeze(routing.occlusions?.map((rect) => Object.freeze(rect)) ?? []),
            modal: routing.modal ?? false,
          }
        : {}),
    });
  }

  /** Stop geometry delivery before the viewport is detached. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#resize?.disconnect();
    this.#ancestry.disconnect();
    this.#presence?.disconnect();
    this.#presence = null;
    this.#ancestors = [];
    this.#roots = [];
    this.#frame?.();
    this.#frame = null;
    this.#media?.();
    this.#media = null;
    for (const stop of this.#ancestryStops.splice(0)) stop();
    for (const stop of this.#stops.splice(0)) stop();
  }

  /** Observe every current clipping ancestor, including reparenting of a staged surface. */
  #observeAncestry(): void {
    if (this.#disposed) return;
    if (!this.element.isConnected) {
      this.#presence ??= new HostSurfaceObserver(
        this.element.ownerDocument.documentElement,
        '[data-sc-canvas-geometry]',
        (changes) => {
          if (
            !changes.some((change) => change.element === this.element && change.element.isConnected)
          )
            return;
          this.#presence?.disconnect();
          this.#presence = null;
          this.#observeAncestry();
          this.#schedule();
        },
      );
      if (this.#ancestors.length) return;
    } else if (this.#presence) {
      this.#presence.disconnect();
      this.#presence = null;
    }
    const measured = canvasGeometryAncestry(this.element);
    const ancestors = measured?.elements ?? [this.element];
    const roots = measured?.roots ?? [];
    if (
      ancestors.length === this.#ancestors.length &&
      ancestors.every((node, index) => node === this.#ancestors[index]) &&
      roots.length === this.#roots.length &&
      roots.every((root, index) => root === this.#roots[index])
    )
      return;
    this.#ancestors = ancestors;
    this.#roots = roots;
    for (const stop of this.#ancestryStops.splice(0)) stop();
    for (const node of ancestors) {
      if (node.localName === 'slot') {
        const changed = () => {
          this.#observeAncestry();
          this.#schedule();
        };
        node.addEventListener('slotchange', changed);
        this.#ancestryStops.push(() => node.removeEventListener('slotchange', changed));
      }
      const motion = (event: Event) => {
        if (event.target === node) this.#schedule();
      };
      for (const type of [
        'animationstart',
        'animationend',
        'animationcancel',
        'transitionrun',
        'transitionend',
        'transitioncancel',
      ]) {
        node.addEventListener(type, motion);
        this.#ancestryStops.push(() => node.removeEventListener(type, motion));
      }
    }
    this.#resize?.disconnect();
    for (const node of ancestors) this.#resize?.observe(node);
    this.#ancestry.observe(this.element, ancestors, roots);
    if (this.#moving()) this.#schedule();
  }

  /** Observe motion already underway when this attachment or its ancestry is installed. */
  #moving(): boolean {
    return this.#ancestors.some(
      (node) =>
        typeof node.getAnimations === 'function' &&
        node
          .getAnimations()
          .some((animation) => animation.playState === 'running' || animation.pending),
    );
  }

  /** Re-arm a density notification only when monitor/page density changes. */
  #density(): void {
    const window = this.element.ownerDocument.defaultView;
    this.#media?.();
    if (!window?.matchMedia || this.#disposed) return;
    const query = window.matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
    const changed = () => {
      this.backdrops?.invalidate();
      this.#density();
      this.#schedule();
    };
    query.addEventListener('change', changed);
    this.#media = () => query.removeEventListener('change', changed);
  }

  /** Schedule one measurement for a burst of geometry events. */
  #schedule(): void {
    if (this.#frame || this.#disposed) return;
    this.#frame = this.frames.request(() => {
      this.#frame = null;
      if (!this.#disposed) {
        this.#observeAncestry();
        this.changed(this.read());
        if (!this.#disposed && this.#moving()) this.#schedule();
      }
    });
  }
}
