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
/** A single rounded union contour paints the actual proposed chip destination without changing native layout. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderRect, ReorderMeasuredGeometry } from './reorder-geometry.js';
import {
  prepareReorderLandingContour,
  type ReorderLandingBubble,
  landingStageWorkLimit,
} from './reorder-landing-geometry.js';
/** Inflate/clip before the donor's twelve-pixel bubble gap and center/height row coalescing; work is bounded by native snapshot admission. */
export function reorderLandingFragments(
  rects: readonly ReorderRect[],
  held: number,
  viewport: ReorderMeasuredGeometry['viewport'],
): readonly ReorderLandingBubble[] {
  if (
    rects.length > 4096 ||
    ![viewport.left, viewport.right, viewport.top, viewport.bottom].every(Number.isFinite) ||
    viewport.right <= viewport.left ||
    viewport.bottom <= viewport.top
  )
    return Object.freeze([]);
  const fragments = rects
    .filter(
      (rect) =>
        rect.kind === 'chip' &&
        rect.index === held &&
        [rect.left, rect.right, rect.top, rect.bottom].every(Number.isFinite) &&
        rect.right > rect.left &&
        rect.bottom > rect.top,
    )
    .map((rect) => ({
      left: Math.max(viewport.left, rect.left - 4),
      right: Math.min(viewport.right, rect.right + 4),
      top: Math.max(viewport.top, rect.top - 2),
      bottom: Math.min(viewport.bottom, rect.bottom + 2),
    }))
    .filter((rect) => rect.right > rect.left && rect.bottom > rect.top)
    .sort((a, b) => (a.top + a.bottom) / 2 - (b.top + b.bottom) / 2 || a.left - b.left);
  const rows: ReorderLandingBubble[][] = [];
  let work = 0;
  for (const rect of fragments) {
    let row: ReorderLandingBubble[] | undefined;
    for (const candidate of rows) {
      if (++work > landingStageWorkLimit) return Object.freeze([]);
      const first = requireValue(candidate[0], 'reorder-landing-paint');
      if (
        Math.abs((first.top + first.bottom - rect.top - rect.bottom) / 2) <=
        Math.max(1, Math.min(first.bottom - first.top, rect.bottom - rect.top)) / 2
      ) {
        row = candidate;
        break;
      }
    }
    if (row) row.push(rect);
    else rows.push([rect]);
  }
  rows.sort(
    (a, b) => Math.min(...a.map((rect) => rect.top)) - Math.min(...b.map((rect) => rect.top)),
  );
  const result: ReorderLandingBubble[] = [];
  for (const row of rows) {
    row.sort((a, b) => a.left - b.left);
    let current = requireValue(row[0], 'reorder-landing-paint');
    for (const rect of row.slice(1)) {
      if (rect.left <= current.right + 12)
        current = {
          left: Math.min(current.left, rect.left),
          right: Math.max(current.right, rect.right),
          top: Math.min(current.top, rect.top),
          bottom: Math.max(current.bottom, rect.bottom),
        };
      else {
        result.push(Object.freeze(current));
        current = rect;
      }
    }
    result.push(Object.freeze(current));
    if (result.length > 256) return Object.freeze([]);
  }
  return Object.freeze(result);
}
/** A staged contour owns detached construction, exact publication transfer and conditional withdrawal. */
export interface ReorderLandingPublication {
  /** Commit only while the exact acquiring endpoint remains current. Unavailable paint is a valid empty publication. */
  commit(current: () => boolean): boolean;
  /** Published ownership remains exact even when the admitted contour is unavailable. */
  current(): boolean;
  /** Renew appearance without replacing the exact endpoint token or rereading geometry. */
  refresh(pending: boolean, current: () => boolean): boolean;
  /** Old endpoint cleanup cannot withdraw a newer publication. */
  release(): void;
}
/** One admitted SVG resource may transfer between successive exact endpoint tokens. */
interface LandingSurface {
  readonly node: HTMLElement;
  readonly path: SVGPathElement;
  readonly key: string;
}
/** One finite external DOM owner retains exactly one accepted path and clipping context. */
export class ReorderLandingPaint {
  private readonly document: Document;
  private accepted: {
    readonly issue: object;
    readonly surface: LandingSurface | null;
  } | null = null;
  private writing: object | null = null;
  /** Geometry and viewport are supplied by the existing CM read owner; this resource only writes passive paint. */
  constructor(document: Document) {
    this.document = document;
  }
  /** Stage detached output in unlocked WRITE; preparation never replaces the currently accepted contour. */
  prepare(
    fragments: readonly ReorderLandingBubble[],
    color: string,
    pending: boolean,
    viewport: ReorderMeasuredGeometry['viewport'],
  ): ReorderLandingPublication {
    const previous = this.accepted,
      issue = {};
    const visible =
      fragments.length > 256
        ? []
        : fragments.filter(
            (rect) =>
              rect.right + 5 >= viewport.left &&
              rect.left - 5 <= viewport.right &&
              rect.bottom + 3 >= viewport.top &&
              rect.top - 3 <= viewport.bottom,
          );
    const key = visible
      .map((rect) => `${rect.left},${rect.right},${rect.top},${rect.bottom}`)
      .join(';');
    const reused =
      !this.writing &&
      previous?.surface?.key === key &&
      previous.surface.node.isConnected &&
      previous.surface.path.isConnected &&
      previous.surface.node.contains(previous.surface.path)
        ? previous.surface
        : null;
    const surface = reused ?? this.construct(visible, key);
    let state: 'prepared' | 'committed' | 'released' = 'prepared';
    const owned = { issue, surface };
    /** Captured removal never consults or destroys a newer accepted resource. */
    const remove = (): void => {
      if (surface && this.accepted?.surface !== surface) surface.node.remove();
    };
    const publication: ReorderLandingPublication = {
      commit: (current) => {
        if (
          state !== 'prepared' ||
          this.accepted !== previous ||
          !current() ||
          this.accepted !== previous
        )
          return false;
        state = 'committed';
        this.accepted = owned;
        this.writing = issue;
        /** A delegated native write may replace ownership; reentrant preparation never reuses a surface being written. */
        const admitted = (): boolean =>
          this.accepted === owned &&
          state === 'committed' &&
          current() &&
          this.accepted === owned &&
          state === 'committed';
        let published = false;
        const causes: unknown[] = [];
        try {
          if (
            (!surface || this.update(surface, color, pending, viewport, admitted)) &&
            admitted()
          ) {
            if (surface && !surface.node.isConnected) this.document.body.append(surface.node);
            published = admitted();
          }
        } catch (cause) {
          causes.push(cause);
        }
        if (this.writing === issue) this.writing = null;
        for (const cleanup of [
          () => {
            if (!published) publication.release();
          },
          () => {
            if (previous?.surface && this.accepted?.surface !== previous.surface)
              previous.surface.node.remove();
          },
          remove,
        ])
          try {
            cleanup();
          } catch (cause) {
            causes.push(cause);
          }
        if (causes.length === 1) throw causes[0];
        if (causes.length) throw new AggregateError(causes, 'reorder-landing-publication-failed');
        return published;
      },
      refresh: (pending, current) => {
        const admitted = (): boolean => publication.current() && current() && publication.current();
        if (!admitted()) return false;
        this.writing = issue;
        try {
          return (
            (!surface || this.update(surface, color, pending, viewport, admitted)) && admitted()
          );
        } finally {
          if (this.writing === issue) this.writing = null;
        }
      },
      current: () =>
        state === 'committed' &&
        this.accepted === owned &&
        (!surface ||
          (surface.node.isConnected &&
            surface.path.isConnected &&
            surface.node.contains(surface.path))) &&
        this.accepted === owned &&
        state === 'committed',
      release: () => {
        if (state === 'released') return;
        state = 'released';
        if (this.accepted === owned) this.accepted = null;
        remove();
      },
    };
    return publication;
  }
  /** Warm geometry reuses the actual SVG/path; opacity and color update without rebuilding the union. */
  show(
    fragments: readonly ReorderLandingBubble[],
    color: string,
    pending: boolean,
    viewport: ReorderMeasuredGeometry['viewport'],
  ): void {
    const publication = this.prepare(fragments, color, pending, viewport);
    if (!publication.commit(() => true)) publication.release();
  }
  /** Construct one optional detached surface through the existing bounded contour algorithm. */
  private construct(visible: readonly ReorderLandingBubble[], key: string): LandingSurface | null {
    const contour = prepareReorderLandingContour(visible);
    if (contour.status !== 'ready') return null;
    const node = this.document.createElement('span'),
      namespace = 'http://www.w3.org/2000/svg';
    try {
      const svg = this.document.createElementNS(namespace, 'svg'),
        path = this.document.createElementNS(namespace, 'path');
      node.className = 'sp-reorder-landing';
      node.inert = true;
      node.setAttribute('aria-hidden', 'true');
      Object.assign(node.style, {
        position: 'fixed',
        display: 'block',
        contain: 'layout paint style',
        left: '0',
        top: '0',
        width: '100vw',
        height: '100vh',
        overflow: 'hidden',
        pointerEvents: 'none',
        zIndex: '100',
      });
      svg.setAttribute(
        'viewBox',
        `${contour.left} ${contour.top} ${contour.width} ${contour.height}`,
      );
      svg.setAttribute('width', String(contour.width));
      svg.setAttribute('height', String(contour.height));
      Object.assign(svg.style, {
        position: 'absolute',
        left: `${contour.left}px`,
        top: `${contour.top}px`,
        overflow: 'visible',
      });
      path.setAttribute('d', contour.path);
      path.setAttribute('fill', 'none');
      path.setAttribute('stroke-width', '1.25');
      path.setAttribute('stroke-linejoin', 'round');
      path.setAttribute('data-reorder-landing-contour', 'true');
      svg.append(path);
      node.append(svg);
      return { node, path, key };
    } catch (primary) {
      try {
        node.remove();
      } catch (cleanup) {
        throw new AggregateError([primary, cleanup], 'reorder-landing-construction-failed');
      }
      throw primary;
    }
  }
  /** Fence every delegated write and reuse an exact actual SVG stroke declaration. */
  private update(
    surface: LandingSurface,
    color: string,
    pending: boolean,
    viewport: ReorderMeasuredGeometry['viewport'],
    current: () => boolean,
  ): boolean {
    for (const write of [
      () => {
        surface.node.style.opacity = pending ? '0.52' : '0.82';
      },
      () => {
        surface.node.dataset.reorderLanding = pending ? 'pending' : 'settled';
      },
      () => {
        surface.node.style.clipPath = `polygon(${viewport.left}px ${viewport.top}px,${viewport.right}px ${viewport.top}px,${viewport.right}px ${viewport.bottom}px,${viewport.left}px ${viewport.bottom}px)`;
      },
    ]) {
      if (!current()) return false;
      write();
      if (!current()) return false;
    }
    if (!current()) return false;
    const stroke = surface.path.getAttribute('stroke');
    if (!current()) return false;
    if (stroke !== color) {
      surface.path.setAttribute('stroke', color);
      if (!current()) return false;
    }
    return true;
  }
  /** Retire the actual owned node before later callbacks can renew another accepted resource. */
  clear(): void {
    const old = this.accepted;
    this.accepted = null;
    old?.surface?.node.remove();
  }
}
