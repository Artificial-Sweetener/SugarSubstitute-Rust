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
/**
 * The graph/masonry toggle's animated artwork: four outlined nodes linked in series that rearrange
 * into four masonry tiles (`WORKFLOW_VIEW_ICON`, from the shared element record).
 *
 * Position 0 draws the graph and 1 the masonry. Moving between them, every node eases along
 * smoothstep and each link withdraws into the node it leaves, gone before the nodes settle. The
 * fourth graph node waits beyond the icon's right edge, so its link trails off the icon.
 */
import { WORKFLOW_VIEW_ICON, type IconBox } from '../../core/SharedElements.js';

const SVG = 'http://www.w3.org/2000/svg';

/** Smoothstep easing, shared by every node so they arrive together. */
export function easeWorkflowView(t: number): number {
  return t * t * (3 - 2 * t);
}

/** The node boxes at one position between the graph (0) and masonry (1) layouts. */
export function workflowViewBoxes(position: number): IconBox[] {
  const e = easeWorkflowView(Math.min(1, Math.max(0, position)));
  return WORKFLOW_VIEW_ICON.graph.map((graph, index) => {
    const masonry = WORKFLOW_VIEW_ICON.masonry[index] ?? graph;
    return [0, 1, 2, 3].map(
      (k) => (graph[k] ?? 0) + ((masonry[k] ?? 0) - (graph[k] ?? 0)) * e,
    ) as unknown as IconBox;
  });
}

/** How much of each link still shows at one position: all at 0, none from about 0.45. */
export function workflowViewLinkShare(position: number): number {
  return Math.max(0, 1 - WORKFLOW_VIEW_ICON.linkRetract * position);
}

/** One drawn icon that can be posed at any position and animated between the two ends. */
export class WorkflowViewIcon {
  readonly element: SVGSVGElement;
  readonly #links: SVGPathElement[];
  readonly #nodes: SVGRectElement[];
  readonly #window: Window | null;
  #position = 1;
  #frame = 0;

  /** Draw the icon at `position` in the button's text colour. */
  constructor(document: Document, position: number) {
    this.#window = document.defaultView;
    this.element = document.createElementNS(SVG, 'svg');
    this.element.setAttribute('viewBox', '0 0 20 20');
    // The graph's last node lies beyond the right edge by design: the icon ends at its box, so
    // that node and the link reaching for it are cut off. The clip is the icon's own, set
    // inline, so no stylesheet can undo it.
    this.element.style.overflow = 'hidden';
    this.element.setAttribute('aria-hidden', 'true');
    this.element.setAttribute('focusable', 'false');
    const { outline, link } = WORKFLOW_VIEW_ICON;
    this.#links = WORKFLOW_VIEW_ICON.swing.map(() => {
      const path = document.createElementNS(SVG, 'path');
      path.setAttribute('fill', 'none');
      path.setAttribute('stroke', 'currentColor');
      path.setAttribute('stroke-width', String(link));
      path.setAttribute('stroke-linecap', 'round');
      path.setAttribute('pathLength', '1');
      this.element.append(path);
      return path;
    });
    this.#nodes = WORKFLOW_VIEW_ICON.graph.map(() => {
      const rect = document.createElementNS(SVG, 'rect');
      rect.setAttribute('fill', 'none');
      rect.setAttribute('stroke', 'currentColor');
      rect.setAttribute('stroke-width', String(outline));
      this.element.append(rect);
      return rect;
    });
    this.pose(position);
  }

  /** The position currently drawn. */
  get position(): number {
    return this.#position;
  }

  /** Draw the icon at one position at once, cancelling any animation. */
  pose(position: number): void {
    if (this.#frame) this.#window?.cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    this.#draw(position);
  }

  /**
   * Move to `target` over the record's duration from wherever the icon is, or at once when the
   * viewer asks for reduced motion or the page has no animation frames.
   */
  animateTo(target: number): void {
    const window = this.#window;
    const reduced = window?.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true;
    if (!window || reduced || typeof window.requestAnimationFrame !== 'function') {
      this.pose(target);
      return;
    }
    if (this.#frame) window.cancelAnimationFrame(this.#frame);
    const start = this.#position;
    const span = Math.abs(target - start) * WORKFLOW_VIEW_ICON.durationMs;
    if (span === 0) return;
    let began: number | null = null;
    const step = (now: number): void => {
      began ??= now;
      const t = Math.min(1, (now - began) / span);
      this.#draw(start + (target - start) * t);
      this.#frame = t < 1 ? window.requestAnimationFrame(step) : 0;
    };
    this.#frame = window.requestAnimationFrame(step);
  }

  /** Stop any animation with the button that owns the icon. */
  dispose(): void {
    if (this.#frame) this.#window?.cancelAnimationFrame(this.#frame);
    this.#frame = 0;
  }

  #draw(position: number): void {
    this.#position = position;
    const { outline, cornerRadius, swing } = WORKFLOW_VIEW_ICON;
    const half = outline / 2;
    const boxes = workflowViewBoxes(position);
    const share = workflowViewLinkShare(position);
    this.#links.forEach((path, index) => {
      const from = boxes[index];
      const to = boxes[index + 1];
      const reach = swing[index] ?? 0;
      if (!from || !to || share <= 0) {
        path.setAttribute('visibility', 'hidden');
        return;
      }
      const [ax, ay] = [from[2] - half, (from[1] + from[3]) / 2];
      const [bx, by] = [to[0] + half, (to[1] + to[3]) / 2];
      path.setAttribute(
        'd',
        `M${String(ax)} ${String(ay)}C${String(ax + reach)} ${String(ay)} ${String(bx - reach)} ${String(by)} ${String(bx)} ${String(by)}`,
      );
      path.setAttribute('stroke-dasharray', `${String(share)} 1`);
      path.setAttribute('visibility', 'visible');
    });
    this.#nodes.forEach((rect, index) => {
      const box = boxes[index];
      if (!box) return;
      const [left, top, right, bottom] = box;
      rect.setAttribute('x', String(left + half));
      rect.setAttribute('y', String(top + half));
      rect.setAttribute('width', String(Math.max(0, right - left - outline)));
      rect.setAttribute('height', String(Math.max(0, bottom - top - outline)));
      rect.setAttribute('rx', String(Math.max(0, cornerRadius - half)));
    });
  }
}
