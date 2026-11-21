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
/** Keep graph layers suppressed across workflow replacement and restore native interaction exactly. */
import { workflowMasonryGraphLayers } from './WorkflowMasonryGraphGeometry.js';
import {
  MASONRY_HIDES_ATTRIBUTE,
  markMasonryGraphChromeHidden,
  markMasonryGraphHidden,
} from './WorkflowMasonryStyles.js';
import { SurfaceInputHold } from '../motion/SurfaceInputHold.js';

/** Own graph visibility independently from the lifetime of borrowed masonry cards. */
export class WorkflowMasonryGraphVisibility {
  readonly #inert = new Map<HTMLElement, boolean>();
  readonly #held = new Map<HTMLElement, SurfaceInputHold>();
  readonly #hidden = new Set<HTMLElement>();
  readonly #layers = new Set<HTMLElement>();
  /** The graph containers whose navigation chrome is hidden with their layers. */
  readonly #chrome = new Set<HTMLElement>();

  /** Resolve the current allocation even before a destination masonry surface exists. */
  constructor(private readonly getContainer: () => HTMLElement | null) {}

  /** Whether the graph is currently hidden behind a masonry. */
  get hidden(): boolean {
    return this.#hidden.size > 0;
  }

  /** Give back the input that `holdInput` held, leaving each layer's `inert` state as it is. */
  releaseInput(): void {
    for (const hold of this.#held.values()) hold.release();
    this.#held.clear();
  }

  /** Suppress existing and replacement graph layers without removing their measurable layout. */
  setHidden(hidden: boolean): void {
    this.setInert(hidden);
    if (!hidden) {
      for (const element of this.#hidden) delete element.dataset.sugarcubesMasonry;
      this.#hidden.clear();
      for (const layer of this.#layers)
        if (layer.getAttribute(MASONRY_HIDES_ATTRIBUTE) === 'graph')
          layer.removeAttribute(MASONRY_HIDES_ATTRIBUTE);
      this.#layers.clear();
      for (const root of this.#chrome) {
        markMasonryGraphChromeHidden(root, false);
        markMasonryGraphHidden(root, false);
      }
      this.#chrome.clear();
      return;
    }
    const container = this.getContainer();
    if (!container) return;
    const root = container.closest<HTMLElement>('#graph-canvas-container');
    for (const element of new Set([container, root])) {
      if (!element) continue;
      this.#hidden.add(element);
      if (element.dataset.sugarcubesMasonry !== 'true') element.dataset.sugarcubesMasonry = 'true';
    }
    // Each layer carries its own mark, so hiding what is inside it restyles that layer alone.
    for (const layer of workflowMasonryGraphLayers(container)) this.#hide(layer);
    if (!root || this.#chrome.has(root)) return;
    this.#chrome.add(root);
    markMasonryGraphChromeHidden(root, true);
    // Whatever else the container holds, now or later, is hidden with the layers above.
    markMasonryGraphHidden(root, true, container);
  }

  /** Mark one layer hidden, remembering it for release. */
  #hide(element: HTMLElement): void {
    this.#layers.add(element);
    if (element.getAttribute(MASONRY_HIDES_ATTRIBUTE) !== 'graph')
      element.setAttribute(MASONRY_HIDES_ATTRIBUTE, 'graph');
  }

  /** Block native input during card travel while preserving each layer's prior inert state. */
  setInert(inert: boolean): void {
    for (const hold of this.#held.values()) hold.release();
    this.#held.clear();
    if (!inert) {
      for (const [element, previous] of this.#inert)
        if (element.inert !== previous) element.inert = previous;
      this.#inert.clear();
      return;
    }
    const container = this.getContainer();
    if (!container) return;
    for (const layer of workflowMasonryGraphLayers(container)) {
      if (!this.#inert.has(layer)) this.#inert.set(layer, layer.inert);
      if (!layer.inert) layer.inert = true;
    }
  }

  /**
   * Keep input out of the layers by a hold instead of `inert`, until `setInert` next runs.
   *
   * Called while borrowed cards are inside a layer or about to return to it: `inert` reaches
   * every descendant's style, so setting or clearing it then restyles every one of them.
   */
  holdInput(): void {
    const container = this.getContainer();
    for (const layer of container ? workflowMasonryGraphLayers(container) : [])
      if (!this.#inert.has(layer)) this.#inert.set(layer, layer.inert);
    for (const [layer, previous] of this.#inert) {
      if (previous || this.#held.has(layer)) continue;
      if (layer.inert) layer.inert = false;
      const hold = new SurfaceInputHold();
      hold.hold(layer);
      this.#held.set(layer, hold);
    }
  }
}
