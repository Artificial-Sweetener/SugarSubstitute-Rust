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
/** Animate a loose node's existing graph presentation without publishing temporary graph bounds. */
import type { WorkflowMasonryGraphCard } from './WorkflowMasonryGraphGeometry.js';

interface GeometryIdentity {
  element: HTMLElement;
  name: string;
  value: string;
}

/** Retain the real graph layout while it blends into its native masonry presentation during travel. */
export class NativeGraphCardAssembly {
  readonly source: WorkflowMasonryGraphCard;
  readonly #transform: string;
  readonly #origin: { value: string; priority: string };
  readonly #identities: GeometryIdentity[];
  #attached = false;

  /** Capture stable graph geometry before any effect or canonical-observer suspension. */
  constructor(source: WorkflowMasonryGraphCard) {
    this.source = source;
    const element = source.element;
    this.#transform =
      element.ownerDocument.defaultView?.getComputedStyle(element).transform ?? 'none';
    this.#origin = {
      value: element.style.getPropertyValue('transform-origin'),
      priority: element.style.getPropertyPriority('transform-origin'),
    };
    this.#identities = [
      element,
      ...element.querySelectorAll<HTMLElement>('[data-widgets-grid-node-id]'),
    ].flatMap((target) =>
      ['data-node-id', 'data-widgets-grid-node-id'].flatMap((name) => {
        const value = target.getAttribute(name);
        return value === null ? [] : [{ element: target, name, value }];
      }),
    );
  }

  /** Blend native bounds and camera scale without letting host minimum heights pin the card shell. */
  frames(destination: DOMRectReadOnly, entering: boolean, interrupted: boolean): Keyframe[] {
    const { element, rectangle, scale, layoutWidth } = this.source;
    const computed = interrupted
      ? element.ownerDocument.defaultView?.getComputedStyle(element)
      : null;
    const graphTransform =
      this.#transform === 'none' ? 'translate(0px, 0px) scale(1)' : this.#transform;
    const base = this.#transform === 'none' ? '' : `${this.#transform} `;
    const masonryTransform = `${base}translate(${String((destination.x - rectangle.x) / scale)}px, ${String((destination.y - rectangle.y) / scale)}px) scale(${String(1 / scale)})`;
    const graphHeight = `${String(rectangle.height / scale)}px`;
    const masonryHeight = `${String(destination.height)}px`;
    return [
      {
        transform: computed?.transform || (entering ? graphTransform : masonryTransform),
        width: computed?.width || `${String(entering ? layoutWidth : destination.width)}px`,
        height: computed?.height || (entering ? graphHeight : masonryHeight),
        minHeight: '0px',
        overflow: 'clip',
      },
      {
        transform: entering ? masonryTransform : graphTransform,
        width: `${String(entering ? destination.width : layoutWidth)}px`,
        height: entering ? masonryHeight : graphHeight,
        minHeight: '0px',
        overflow: 'clip',
      },
    ];
  }

  /** Suspend only the host's DOM geometry identities for the duration of the width interpolation. */
  attach(): void {
    if (this.#attached) return;
    this.#attached = true;
    const element = this.source.element;
    element.dataset.scMasonryGraphNode = element.dataset.nodeId ?? '';
    element.style.setProperty('transform-origin', '0 0');
    // Comfy's shared ResizeObserver resolves canonical node bounds through these DOM attributes.
    for (const identity of this.#identities) identity.element.removeAttribute(identity.name);
  }

  /** Restore host identities after all effects have returned the original native dimensions. */
  dispose(): void {
    if (!this.#attached) return;
    this.#attached = false;
    const element = this.source.element;
    delete element.dataset.scMasonryGraphNode;
    if (this.#origin.value)
      element.style.setProperty('transform-origin', this.#origin.value, this.#origin.priority);
    else element.style.removeProperty('transform-origin');
    for (const identity of this.#identities)
      if (!identity.element.hasAttribute(identity.name))
        identity.element.setAttribute(identity.name, identity.value);
  }
}
