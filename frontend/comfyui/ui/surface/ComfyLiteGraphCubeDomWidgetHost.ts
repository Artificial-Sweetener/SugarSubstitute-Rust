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
/** Mount Comfy's exact Nodes 1.0 DOM widgets over native Cube-face cards. */

import type { ComfyNode, ComfyWidget } from '../types/graph.js';
import type { CubeCanvasCard, CubeCanvasLayout } from './CubeCanvasLayout.js';
import { findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
import { cubeFaceNodeWidgetStartY } from './CubeFaceNodePresentationPolicy.js';
import { cubeFaceUnconsumedWidgets } from './CubeFaceWidgetPolicy.js';
import { CubePromptDomWidgetSizingHost } from './CubePromptDomWidgetSizingHost.js';

export interface LiteGraphCubeDomWidgetCanvas {
  canvas: HTMLCanvasElement;
  ds?: {
    scale?: number;
    offset?: ArrayLike<number>;
  };
  read_only?: boolean;
}

export interface LiteGraphCubeDomWidgetItem {
  layout: CubeCanvasLayout;
}

export interface ComfyLiteGraphCubeDomWidgetHostOptions {
  document: Document;
  canvas: LiteGraphCubeDomWidgetCanvas;
  logger?: Pick<Console, 'warn'>;
  onGeometryChange?(): void;
}

interface NativeDomWidget extends ComfyWidget {
  element: HTMLElement;
}

interface NativeElementOrigin {
  parent: Node | null;
  nextSibling: ChildNode | null;
  hadFullHeight: boolean;
  hadFullWidth: boolean;
}

interface MountedNativeWidget {
  element: HTMLElement;
  origin: NativeElementOrigin;
  wrapper: HTMLDivElement;
}

interface WidgetGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  disabled: boolean;
  promptWidget: NativeDomWidget | null;
}

const DEFAULT_WIDGET_MARGIN = 10;
const DEFAULT_WIDGET_HEIGHT = 50;

/** Own reparenting and graph-to-viewport positioning for native DOM widgets. */
export class ComfyLiteGraphCubeDomWidgetHost {
  readonly #document: Document;
  readonly #canvas: LiteGraphCubeDomWidgetCanvas;
  readonly #logger: Pick<Console, 'warn'> | null;
  readonly #promptSizing: CubePromptDomWidgetSizingHost;
  readonly #mounted = new Map<HTMLElement, MountedNativeWidget>();
  #root: HTMLDivElement | null = null;

  /** Bind one focused adapter to Comfy's canvas transform and document. */
  constructor(options: ComfyLiteGraphCubeDomWidgetHostOptions) {
    this.#document = options.document;
    this.#canvas = options.canvas;
    this.#logger = options.logger ?? null;
    this.#promptSizing = new CubePromptDomWidgetSizingHost(
      options.document,
      options.onGeometryChange ?? (() => undefined),
    );
  }

  /** Reconcile exact native elements with the currently revealed Cube cards. */
  sync(items: readonly LiteGraphCubeDomWidgetItem[]): void {
    const desired = new Map<HTMLElement, WidgetGeometry>();
    for (const item of items) {
      for (const card of item.layout.cards) {
        this.#collectCardWidgets(card, desired);
      }
    }
    for (const [element, mounted] of [...this.#mounted]) {
      if (!desired.has(element)) this.#restore(mounted);
    }
    for (const [element, geometry] of desired) {
      const mounted = this.#mounted.get(element) ?? this.#mount(element);
      this.#reclaimElement(mounted);
      this.#promptSizing.reconcile(mounted.element, geometry.promptWidget);
      this.#position(mounted.wrapper, geometry);
    }
    this.#removeEmptyRoot();
  }

  /** Restore native ownership for every element and remove the overlay root. */
  dispose(): void {
    for (const mounted of [...this.#mounted.values()]) this.#restore(mounted);
    this.#promptSizing.dispose();
    this.#root?.remove();
    this.#root = null;
  }

  /** Collect valid DOM widgets after Comfy has laid out the native face card. */
  #collectCardWidgets(card: CubeCanvasCard, desired: Map<HTMLElement, WidgetGeometry>): void {
    for (const widget of cubeFaceUnconsumedWidgets(card.node)) {
      const nativeWidget = this.#asNativeDomWidget(widget);
      if (!nativeWidget || !this.#isVisible(card.node, nativeWidget)) continue;
      const geometry = this.#geometry(card, nativeWidget);
      if (geometry) desired.set(nativeWidget.element, geometry);
    }
  }

  /** Narrow dynamic Comfy widget state to a real element from this document. */
  #asNativeDomWidget(widget: ComfyWidget): NativeDomWidget | null {
    const element = widget.element;
    const elementType = this.#document.defaultView?.HTMLElement;
    if (!elementType || !(element instanceof elementType)) return null;
    return widget as NativeDomWidget;
  }

  /** Respect Comfy's own widget and node visibility policies. */
  #isVisible(node: ComfyNode, widget: NativeDomWidget): boolean {
    try {
      const widgetVisibility = widget.isVisible;
      if (typeof widgetVisibility === 'function' && widgetVisibility.call(widget) === false) {
        return false;
      }
      const nodeVisibility = node.isWidgetVisible;
      return typeof nodeVisibility !== 'function' || nodeVisibility.call(node, widget) !== false;
    } catch (error: unknown) {
      this.#logger?.warn('SugarCubes could not evaluate a native DOM widget.', {
        nodeId: node.id,
        widgetName: widget.name,
        error,
      });
      return false;
    }
  }

  /** Translate Comfy's native widget layout into fixed viewport geometry. */
  #geometry(card: CubeCanvasCard, widget: NativeDomWidget): WidgetGeometry | null {
    const promptWidget = findCubeFacePromptWidget(card.node) === widget ? widget : null;
    const widgetY =
      promptWidget === null
        ? (finiteNumber(widget.y) ?? finiteNumber(widget.last_y))
        : cubeFaceNodeWidgetStartY(card.node);
    if (widgetY === null) return null;
    const margin = Math.max(0, finiteNumber(widget.margin) ?? DEFAULT_WIDGET_MARGIN);
    const computedHeight =
      promptWidget === null
        ? (finiteNumber(widget.computedHeight) ?? DEFAULT_WIDGET_HEIGHT)
        : Math.max(1, card.bodyHeight - widgetY);
    const titleHeight = Math.max(1, card.rect.height - card.bodyHeight);
    const scale = Math.max(0.01, finiteNumber(this.#canvas.ds?.scale) ?? 1);
    const offset = this.#canvas.ds?.offset;
    const bounds = this.#canvas.canvas.getBoundingClientRect();
    const graphX = card.rect.x + margin;
    const graphY = card.rect.y + titleHeight + widgetY + margin;
    return {
      x: bounds.left + (graphX + finiteVectorValue(offset, 0)) * scale,
      y: bounds.top + (graphY + finiteVectorValue(offset, 1)) * scale,
      width: Math.max(1, card.rect.width - margin * 2),
      height: Math.max(1, computedHeight - margin * 2),
      scale,
      disabled: this.#canvas.read_only === true || widget.computedDisabled === true,
      promptWidget,
    };
  }

  /** Move one exact element into a native-compatible positioning wrapper. */
  #mount(element: HTMLElement): MountedNativeWidget {
    const origin: NativeElementOrigin = {
      parent: element.parentNode,
      nextSibling: element.nextSibling,
      hadFullHeight: element.classList.contains('h-full'),
      hadFullWidth: element.classList.contains('w-full'),
    };
    const wrapper = this.#document.createElement('div');
    wrapper.className = 'dom-widget size-full sugarcubes-cube-face-dom-widget';
    wrapper.dataset.sugarcubesCubeFaceDomWidget = '';
    wrapper.style.position = 'fixed';
    wrapper.style.transformOrigin = '0 0';
    wrapper.style.boxSizing = 'border-box';
    element.classList.add('h-full', 'w-full');
    wrapper.append(element);
    this.#ensureRoot().append(wrapper);
    const mounted: MountedNativeWidget = { element, origin, wrapper };
    this.#mounted.set(element, mounted);
    return mounted;
  }

  /** Reassert face ownership after Comfy's native widget mount runs asynchronously. */
  #reclaimElement(mounted: MountedNativeWidget): void {
    if (mounted.element.parentNode === mounted.wrapper) return;
    mounted.wrapper.replaceChildren(mounted.element);
  }

  /** Apply Comfy's fixed-position transform model without changing the element. */
  #position(wrapper: HTMLDivElement, geometry: WidgetGeometry): void {
    wrapper.style.left = `${String(geometry.x)}px`;
    wrapper.style.top = `${String(geometry.y)}px`;
    wrapper.style.width = `${String(geometry.width)}px`;
    wrapper.style.height = `${String(geometry.height)}px`;
    wrapper.style.transform = `scale(${String(geometry.scale)})`;
    wrapper.style.pointerEvents = geometry.disabled ? 'none' : 'auto';
    wrapper.style.opacity = geometry.disabled ? '0.5' : '1';
  }

  /** Restore one exact native element to its prior DOM owner and class state. */
  #restore(mounted: MountedNativeWidget): void {
    const { element, origin, wrapper } = mounted;
    this.#promptSizing.release(element);
    if (!origin.hadFullHeight) element.classList.remove('h-full');
    if (!origin.hadFullWidth) element.classList.remove('w-full');
    if (origin.parent) {
      const insertionPoint =
        origin.nextSibling?.parentNode === origin.parent ? origin.nextSibling : null;
      origin.parent.insertBefore(element, insertionPoint);
    } else {
      element.remove();
    }
    wrapper.remove();
    this.#mounted.delete(element);
  }

  /** Create the pointer-transparent overlay without intercepting graph input. */
  #ensureRoot(): HTMLDivElement {
    if (this.#root) return this.#root;
    const root = this.#document.createElement('div');
    root.dataset.sugarcubesCubeFaceDomWidgets = '';
    root.style.position = 'fixed';
    root.style.inset = '0';
    root.style.pointerEvents = 'none';
    root.style.overflow = 'hidden';
    this.#document.body.append(root);
    this.#root = root;
    return root;
  }

  /** Remove an unused root so it cannot outlive the active renderer mode. */
  #removeEmptyRoot(): void {
    if (this.#mounted.size > 0) return;
    this.#root?.remove();
    this.#root = null;
  }
}

/** Read one finite dynamic host number without letting invalid values inward. */
function finiteNumber(value: unknown): number | null {
  const number = typeof value === 'number' ? value : Number.NaN;
  return Number.isFinite(number) ? number : null;
}

/** Read one finite graph transform component with a neutral fallback. */
function finiteVectorValue(value: ArrayLike<number> | undefined, index: number): number {
  return finiteNumber(value?.[index]) ?? 0;
}
