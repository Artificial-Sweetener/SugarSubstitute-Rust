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
/** One finite accepted landing publication retains actual fragments and pending intent; the native SVG painter owns pixels. */
import type { ReorderLandingBubble } from './reorder-landing-geometry.js';
import type { ReorderMeasureInput } from './reorder-measurement-contracts.js';
import type { ReorderRect, ReorderMeasuredGeometry } from './reorder-geometry.js';
import {
  ReorderLandingPaint,
  reorderLandingFragments,
  type ReorderLandingPublication,
} from './reorder-landing-paint.js';
import type { ReorderMotionScene } from './reorder-motion-scene.js';
/** Contour publication borrows Session freshness and preparation intent without owning either scheduler. */
export class ReorderMotionLanding {
  private readonly landing: ReorderLandingPaint;
  private readonly available: () => boolean;
  private readonly current: (input: ReorderMeasureInput) => boolean;
  private readonly preparing: () => boolean;
  private heldLanding: {
    readonly input: ReorderMeasureInput;
    readonly endpoint: {
      readonly publication: ReorderLandingPublication;
      readonly current: () => boolean;
    } | null;
    readonly fragments: readonly ReorderLandingBubble[];
    readonly rects: readonly ReorderRect[];
    readonly held: number;
    readonly order: ReorderMeasureInput['order'];
    pointerPending: boolean;
    readonly viewport: ReorderMeasuredGeometry['viewport'];
    readonly color: string;
  } | null = null;
  /** Native paint lifetime remains delegated to its original one-surface owner. */
  constructor(
    document: Document,
    available: () => boolean,
    current: (input: ReorderMeasureInput) => boolean,
    preparing: () => boolean,
  ) {
    this.landing = new ReorderLandingPaint(document);
    this.available = available;
    this.current = current;
    this.preparing = preparing;
  }
  /** Appearance invalidation may renew only the actually retained semantic landing input. */
  get input(): ReorderMeasureInput | null {
    return this.heldLanding?.input ?? null;
  }
  /** Retire the accepted operand before independently owned native paint can reenter. */
  clear(): void {
    this.heldLanding = null;
    this.landing.clear();
  }
  /** Stage off-DOM through the sole painter; the exact Resource commits and retires its borrowed publication. */
  prepareEndpoint(
    input: ReorderMeasureInput,
    scene: ReorderMotionScene,
    hold: object | undefined,
    retained: () => boolean,
  ): ReorderLandingPublication | null {
    const previous = this.heldLanding,
      endpoint = scene.endpoint;
    if (
      !scene.whole ||
      !endpoint ||
      endpoint.order !== input.order ||
      endpoint.held !== input.held ||
      endpoint.hold !== hold
    )
      return null;
    const order = input.order,
      held = input.held;
    const fragments = reorderLandingFragments(scene.geometry, held, endpoint.viewport),
      publication = this.landing.prepare(fragments, endpoint.color, false, endpoint.viewport);
    const record = {
      input,
      fragments,
      rects: scene.geometry,
      held,
      order,
      pointerPending: false,
      viewport: endpoint.viewport,
      color: endpoint.color,
      endpoint: { publication, current: retained },
    };
    return {
      commit: (current) => {
        const admitted = (): boolean => this.available() && current();
        if (this.heldLanding !== previous || !admitted() || this.heldLanding !== previous)
          return false;
        this.heldLanding = record;
        return publication.commit(
          () => this.heldLanding === record && admitted() && this.heldLanding === record,
        );
      },
      current: () => this.heldLanding === record && publication.current(),
      refresh: (pending, current) => publication.refresh(pending, current),
      release: () => {
        if (this.heldLanding === record) this.heldLanding = null;
        publication.release();
      },
    };
  }
  /** Actual held fragments and computed native outline color are supplied by the existing CM read owner. */
  show(
    input: ReorderMeasureInput,
    rects: readonly ReorderRect[],
    color: string,
    viewport: ReorderMeasuredGeometry['viewport'],
    pointerPending = false,
  ): void {
    const retained = this.heldLanding,
      endpoint = retained?.endpoint;
    if (endpoint) {
      const admitted = endpoint.publication.current() && endpoint.current();
      if (this.heldLanding !== retained) return;
      if (admitted) {
        retained.pointerPending = pointerPending;
        this.pending(pointerPending);
        return;
      }
      this.clear();
    }
    if (this.available() && this.current(input)) {
      const previous = this.heldLanding;
      const fragments =
        previous?.input === input &&
        previous.rects === rects &&
        previous.held === input.held &&
        previous.order === input.order &&
        previous.viewport.left === viewport.left &&
        previous.viewport.right === viewport.right &&
        previous.viewport.top === viewport.top &&
        previous.viewport.bottom === viewport.bottom
          ? previous.fragments
          : reorderLandingFragments(rects, input.held, viewport);
      this.heldLanding = {
        input,
        endpoint: null,
        fragments,
        rects,
        held: input.held,
        order: input.order,
        pointerPending,
        viewport: Object.freeze({
          left: viewport.left,
          right: viewport.right,
          top: viewport.top,
          bottom: viewport.bottom,
        }),
        color,
      };
      this.landing.show(fragments, color, pointerPending || this.preparing(), viewport);
    } else {
      this.heldLanding = null;
      this.landing.clear();
    }
  }
  /** Current pointer delivery and finite preparation independently determine provisional landing opacity. */
  pending(pointerPending = false): void {
    const held = this.heldLanding;
    const admitted = !!(
      held &&
      this.available() &&
      (held.endpoint
        ? held.endpoint.publication.current() && held.endpoint.current()
        : this.current(held.input) &&
          held.order === held.input.order &&
          held.held === held.input.held)
    );
    if (this.heldLanding !== held) return;
    if (admitted && held) {
      held.pointerPending ||= pointerPending;
      if (held.endpoint) {
        held.endpoint.publication.refresh(
          held.pointerPending || this.preparing(),
          held.endpoint.current,
        );
        return;
      }
      this.landing.show(
        held.fragments,
        held.color,
        held.pointerPending || this.preparing(),
        held.viewport,
      );
    } else {
      this.heldLanding = null;
      this.landing.clear();
    }
  }
}
