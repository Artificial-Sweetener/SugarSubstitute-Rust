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
/** Measure web content covering a native viewport so the host can route OS gestures around it. */
import {
  CANVAS_SURFACE_OCCLUSIONS,
  CANVAS_SURFACE_OCCLUSION_EXTENT,
} from '../../core/embedded/CanvasRuntime.js';
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type { CanvasSurfaceRouting } from './CanvasSurfaceGeometryObserver.js';
import { observeNativeCanvasOverlays } from './NativeCanvasOverlayObserver.js';

type Occlusion = NonNullable<CanvasSurfaceGeometry['occlusions']>[number];

/** The attachment's own chrome over its viewport: tabs, status, divider, grid and editing tools. */
const ATTACHMENT_CHROME =
  '.sc-output-tabstrip, .sc-output-status, .sc-output-wipe, .sc-output-overview, .sc-canvas-edit-chrome';
/** Elements examined per measurement, in document order, bounding work in large documents. */
const MAX_SCANNED_ELEMENTS = 6000;
/** Positioned elements over the viewport whose paint order is confirmed per measurement. */
const MAX_CANDIDATES = 48;
/** Fractions of an intersection sampled for paint order: its centre and four inner points. */
const SAMPLES = [
  [0.5, 0.5],
  [0.25, 0.25],
  [0.75, 0.25],
  [0.25, 0.75],
  [0.75, 0.75],
] as const;
const OUT_OF_FLOW = new Set(['fixed', 'absolute', 'sticky']);

interface ClientBox {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

/**
 * Publish the parts of one native viewport covered by web content, and whether a modal blocks it.
 *
 * Any document content may cover the viewport, including popups from ComfyUI, custom nodes and
 * other extensions, so candidates are found structurally rather than by name: visible
 * out-of-flow or top-layer elements, other than the viewport, its ancestors and descendants,
 * whose box meets the viewport. Each is confirmed by hit-testing sample points of the
 * intersection, so content painted behind the viewport and non-interactive layers do not
 * occlude it. The attachment's own chrome is always reported. Rects are clipped to the
 * viewport, relative to its origin, and merged past the protocol ceiling.
 */
export class NativeCanvasOcclusions implements CanvasSurfaceRouting {
  /** Measure against one viewport inside the attachment root that owns its chrome. */
  constructor(
    private readonly root: HTMLElement,
    private readonly viewport: HTMLElement,
  ) {}

  /** Invalidate on any body mutation that can create, show, hide or move an overlay. */
  observe(changed: () => void): () => void {
    return observeNativeCanvasOverlays(this.root.ownerDocument, changed);
  }

  /** Measure covering content against the viewport allocation in client CSS pixels. */
  read(
    origin: CanvasSurfaceGeometry['origin'],
    viewport: CanvasSurfaceGeometry['viewport'],
  ): Pick<CanvasSurfaceGeometry, 'occlusions' | 'modal'> {
    const box: ClientBox = {
      left: origin.x,
      top: origin.y,
      right: origin.x + viewport.width,
      bottom: origin.y + viewport.height,
    };
    const occlusions = new Map<string, Occlusion>();
    const add = (rect: Occlusion | null): void => {
      if (rect) occlusions.set(JSON.stringify(rect), rect);
    };
    for (const chrome of this.root.querySelectorAll<HTMLElement>(ATTACHMENT_CHROME))
      if (rendered(chrome)) add(clipped(chrome.getBoundingClientRect(), box));
    let covering = false;
    for (const candidate of this.#candidates(box)) {
      const intersection = clipped(candidate.getBoundingClientRect(), box);
      if (!intersection || within(intersection, occlusions.values())) continue;
      for (const element of this.#painted(candidate, intersection, box)) {
        const rect = clipped(element.getBoundingClientRect(), box);
        add(rect);
        covering ||= rect !== null && spans(rect, viewport);
      }
    }
    return { occlusions: bounded([...occlusions.values()]), modal: covering || this.#modal() };
  }

  /** Find visible out-of-flow or top-layer elements meeting the viewport, within the scan bound. */
  #candidates(box: ClientBox): readonly Element[] {
    const document = this.root.ownerDocument;
    const window = document.defaultView;
    const body = document.body;
    if (!window || !body) return [];
    const viewport = this.viewport;
    const walker = document.createTreeWalker(body, window.NodeFilter.SHOW_ELEMENT, {
      // The viewport's own subtree can never cover it.
      acceptNode: (node) =>
        node === viewport ? window.NodeFilter.FILTER_REJECT : window.NodeFilter.FILTER_ACCEPT,
    });
    const candidates: Element[] = [];
    let node = walker.nextNode();
    for (
      let scanned = 0;
      node && scanned < MAX_SCANNED_ELEMENTS && candidates.length < MAX_CANDIDATES;
      scanned++
    ) {
      if (
        node instanceof window.Element &&
        !node.contains(viewport) &&
        clipped(node.getBoundingClientRect(), box) &&
        positioned(node, window) &&
        rendered(node)
      )
        candidates.push(node);
      node = walker.nextNode();
    }
    return candidates;
  }

  /**
   * Hit-test sample points of the intersection and return the covering interactive roots.
   *
   * A sample confirms the candidate when the candidate or a descendant precedes the viewport in
   * the paint stack. The reported element climbs from the hit only while each ancestor receives
   * pointer events, so a pass-through container contributes just its interactive children.
   */
  #painted(candidate: Element, intersection: Occlusion, box: ClientBox): readonly Element[] {
    const document = this.root.ownerDocument;
    // Without hit testing, paint order cannot be proven; report the candidate conservatively.
    if (typeof document.elementsFromPoint !== 'function') return [candidate];
    const window = document.defaultView;
    const roots = new Set<Element>();
    for (const [fx, fy] of SAMPLES) {
      const stack = document.elementsFromPoint(
        box.left + intersection.x + intersection.width * fx,
        box.top + intersection.y + intersection.height * fy,
      );
      const below = stack.indexOf(this.viewport);
      if (below < 0) continue;
      const hit = stack.slice(0, below).find((element) => candidate.contains(element));
      if (!hit) continue;
      let interactive = hit;
      while (
        interactive !== candidate &&
        interactive.parentElement &&
        window?.getComputedStyle(interactive.parentElement).pointerEvents !== 'none'
      )
        interactive = interactive.parentElement;
      roots.add(interactive);
    }
    return [...roots];
  }

  /** Report an open modal dialog or explicitly modal content anywhere in the document. */
  #modal(): boolean {
    const document = this.root.ownerDocument;
    return [...document.querySelectorAll('dialog[open], [aria-modal="true"]')].some(
      (element) => rendered(element) && (element.localName !== 'dialog' || modalDialog(element)),
    );
  }
}

/** Report whether an element leaves normal flow or sits in the top layer. */
function positioned(element: Element, window: Window): boolean {
  if (OUT_OF_FLOW.has(window.getComputedStyle(element).position)) return true;
  if (element.localName === 'dialog' && element.hasAttribute('open')) return true;
  try {
    return element.matches(':popover-open');
  } catch {
    // Engines without the Popover API reject the selector; their elements are never open.
    return false;
  }
}

/** Report whether a dialog was opened modally; engines without `:modal` rely on aria-modal. */
function modalDialog(dialog: Element): boolean {
  const declared = dialog.getAttribute('aria-modal') === 'true';
  try {
    return declared || dialog.matches(':modal');
  } catch {
    return declared;
  }
}

/** Report whether an element renders, including hidden ancestors where the engine can tell. */
function rendered(element: Element): boolean {
  if (!element.isConnected) return false;
  if (typeof element.checkVisibility === 'function')
    return element.checkVisibility({ checkVisibilityCSS: true });
  const style = element.ownerDocument.defaultView?.getComputedStyle(element);
  return !style || (style.display !== 'none' && style.visibility !== 'hidden');
}

/** Clip a client box to the viewport, relative to the viewport origin. */
function clipped(bounds: DOMRect, box: ClientBox): Occlusion | null {
  const left = Math.max(bounds.left, box.left);
  const top = Math.max(bounds.top, box.top);
  const width = Math.min(bounds.right, box.right) - left;
  const height = Math.min(bounds.bottom, box.bottom) - top;
  const limit = CANVAS_SURFACE_OCCLUSION_EXTENT;
  const rect = { x: left - box.left, y: top - box.top, width, height };
  return Object.values(rect).every((value) => Number.isFinite(value) && Math.abs(value) <= limit) &&
    width > 0 &&
    height > 0
    ? rect
    : null;
}

/** Report whether a rect lies inside one already reported, which needs no confirmation. */
function within(rect: Occlusion, reported: Iterable<Occlusion>): boolean {
  for (const other of reported)
    if (
      rect.x >= other.x &&
      rect.y >= other.y &&
      rect.x + rect.width <= other.x + other.width &&
      rect.y + rect.height <= other.y + other.height
    )
      return true;
  return false;
}

/** Report whether a rect covers the whole viewport. */
function spans(rect: Occlusion, viewport: CanvasSurfaceGeometry['viewport']): boolean {
  return (
    rect.x <= 0 && rect.y <= 0 && rect.width >= viewport.width && rect.height >= viewport.height
  );
}

/** Keep the protocol's rect ceiling by merging every rect past it into their covering box. */
function bounded(occlusions: readonly Occlusion[]): readonly Occlusion[] {
  if (occlusions.length <= CANVAS_SURFACE_OCCLUSIONS) return occlusions;
  const rest = occlusions.slice(CANVAS_SURFACE_OCCLUSIONS - 1);
  const left = Math.min(...rest.map(({ x }) => x));
  const top = Math.min(...rest.map(({ y }) => y));
  const right = Math.max(...rest.map(({ x, width }) => x + width));
  const bottom = Math.max(...rest.map(({ y, height }) => y + height));
  return [
    ...occlusions.slice(0, CANVAS_SURFACE_OCCLUSIONS - 1),
    { x: left, y: top, width: right - left, height: bottom - top },
  ];
}
