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
/** Own detached, bounded popup geometry and lifecycle without participating in application layout. */
import { applyOwnedTheme } from '../surface/appearance/ThemeService.js';
import { readOwnedDarkTheme } from '../surface/appearance/OwnedColorScheme.js';

export type FloatingAnchor =
  | { readonly kind: 'pointer'; readonly x: number; readonly y: number }
  | { readonly kind: 'below' | 'submenu'; readonly bounds: DOMRect | FloatingBounds };
export interface FloatingBounds {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}
export interface FloatingSurfaceOptions {
  /** Let native editor tooltips retain their own measured positions inside one shared top-layer plane. */
  readonly viewport?: boolean;
  /** Reserve one measured surface width to the right before interaction starts. */
  readonly reserveRight?: boolean;
  readonly anchor: FloatingAnchor;
  readonly dismiss: () => void;
  readonly containsTarget?: (target: Node) => boolean;
  readonly maxHeight?: number;
}

/** Expose only a native editor plane's finite publication lifetime to feature adapters. */
export interface FloatingPlane {
  show(): void;
  hide(): void;
  dispose(): void;
}

/** Keep browser transport and native ancestry restoration under the shared floating owner. */
export function createFloatingPlane(element: HTMLElement): FloatingPlane {
  const surface = new FloatingSurface(element);
  let origin: HTMLElement | null = null;
  return {
    /** Retain host ancestry before entering the browser top layer. */
    show(): void {
      const parent = element.parentElement;
      if (parent && parent !== element.ownerDocument.body) origin = parent;
      surface.open({
        anchor: { kind: 'pointer', x: 0, y: 0 },
        viewport: true,
        dismiss: () => undefined,
      });
    },
    /** Retain native connectivity so an inactive plane can reopen. */
    hide(): void {
      surface.close();
      origin?.append(element);
    },
    /** Retire transport without restoring a renderer that is being destroyed. */
    dispose(): void {
      surface.close();
      origin = null;
    },
  };
}

/** Keep all popup kinds within the viewport while preferring their natural opening direction. */
export function floatingPosition(
  anchor: FloatingAnchor,
  width: number,
  height: number,
  viewportWidth: number,
  viewportHeight: number,
  reserveRight = 0,
): { left: number; top: number } {
  const gap = 4;
  let left = anchor.kind === 'pointer' ? anchor.x : anchor.bounds.left;
  let top = anchor.kind === 'pointer' ? anchor.y : anchor.bounds.bottom + gap;
  if (anchor.kind === 'submenu') {
    left = anchor.bounds.right - gap;
    top = anchor.bounds.top;
    if (left + width > viewportWidth - gap) left = anchor.bounds.left - width + gap;
  } else if (anchor.kind === 'below' && top + height > viewportHeight - gap)
    top = anchor.bounds.top - height - gap;
  if (anchor.kind === 'pointer' && reserveRight > 0)
    left = Math.min(left, Math.max(gap, viewportWidth - width - reserveRight - gap));
  return {
    left: Math.max(gap, Math.min(left, viewportWidth - width - gap)),
    top: Math.max(gap, Math.min(top, viewportHeight - height - gap)),
  };
}

/** Own one top-layer surface, including a fixed body portal when popovers are unavailable. */
export class FloatingSurface {
  readonly #document: Document;
  readonly element: HTMLElement;
  #options: FloatingSurfaceOptions | null = null;
  #observer: ResizeObserver | null = null;

  /** Bind a feature-owned content element to the shared floating boundary. */
  constructor(element: HTMLElement) {
    this.element = element;
    this.#document = element.ownerDocument;
    element.dataset.scFloatingSurface = '';
    element.setAttribute('popover', 'manual');
    // Geometry is an invariant even before styles load or when feature styles are absent.
    for (const [name, value] of Object.entries({
      position: 'fixed',
      inset: 'auto',
      margin: '0',
      'box-sizing': 'border-box',
      'max-width': 'calc(100vw - 8px)',
      'max-height': 'calc(100vh - 8px)',
      'z-index': '100000',
    }))
      element.style.setProperty(name, value, 'important');
  }

  /** Attach content outside every feature container and position it before accepting interaction. */
  open(options: FloatingSurfaceOptions): void {
    this.close();
    this.#options = options;
    this.#document.body.append(this.element);
    applyOwnedTheme(this.element, readOwnedDarkTheme(this.#document));
    if (typeof this.element.showPopover === 'function') this.element.showPopover();
    this.reposition();
    const windowRef = this.#document.defaultView;
    if (windowRef?.ResizeObserver) {
      this.#observer = new windowRef.ResizeObserver(() => this.reposition());
      this.#observer.observe(this.element);
    }
    this.#document.addEventListener('pointerdown', this.#outside, true);
    this.#document.addEventListener('scroll', this.#scroll, true);
    windowRef?.addEventListener('resize', this.#dismiss);
    this.element.addEventListener('pointerdown', this.#stop);
    this.element.addEventListener('wheel', this.#stop);
    this.element.addEventListener('contextmenu', this.#context);
  }

  /** Reconcile geometry when popup content changes without moving surrounding content. */
  reposition(): void {
    if (!this.#options) return;
    if (this.#options.viewport) {
      for (const [name, value] of Object.entries({
        left: '0',
        top: '0',
        width: '100vw',
        height: '100vh',
        'max-width': 'none',
        'max-height': 'none',
      }))
        this.element.style.setProperty(name, value, 'important');
      return;
    }
    const windowRef = this.#document.defaultView;
    this.element.style.setProperty(
      'max-height',
      this.#options.maxHeight === undefined
        ? 'calc(100vh - 8px)'
        : `${Math.max(0, Math.min(this.#options.maxHeight, (windowRef?.innerHeight ?? this.#document.documentElement.clientHeight) - 8))}px`,
      'important',
    );
    const rect = this.element.getBoundingClientRect();
    const position = floatingPosition(
      this.#options.anchor,
      rect.width,
      rect.height,
      windowRef?.innerWidth ?? this.#document.documentElement.clientWidth,
      windowRef?.innerHeight ?? this.#document.documentElement.clientHeight,
      this.#options.reserveRight ? rect.width : 0,
    );
    this.element.style.setProperty('left', `${String(position.left)}px`, 'important');
    this.element.style.setProperty('top', `${String(position.top)}px`, 'important');
  }

  /** Release all observers, listeners and top-layer state without invoking feature callbacks. */
  close(): void {
    this.#observer?.disconnect();
    this.#observer = null;
    if (this.element.isConnected && typeof this.element.hidePopover === 'function')
      this.element.hidePopover();
    applyOwnedTheme(this.element, null);
    this.element.remove();
    this.#document.removeEventListener('pointerdown', this.#outside, true);
    this.#document.removeEventListener('scroll', this.#scroll, true);
    this.#document.defaultView?.removeEventListener('resize', this.#dismiss);
    this.element.removeEventListener('pointerdown', this.#stop);
    this.element.removeEventListener('wheel', this.#stop);
    this.element.removeEventListener('contextmenu', this.#context);
    this.#options = null;
  }

  /** Dismiss only interactions outside the complete owning popup tree. */
  readonly #outside = (event: Event): void => {
    if (event.target instanceof Node && !this.#contains(event.target)) this.#dismiss();
  };
  /** Keep internal scrolling independent from graph and page scrolling. */
  readonly #scroll = (event: Event): void => {
    if (!(event.target instanceof Node) || !this.#contains(event.target)) this.#dismiss();
  };
  /** Let the feature decide how dismissal changes its state. */
  readonly #dismiss = (): void => this.#options?.dismiss();
  /** Prevent host graph controls from receiving popup input. */
  readonly #stop = (event: Event): void => event.stopPropagation();
  /** Prevent a browser-native menu from replacing an owned menu. */
  readonly #context = (event: Event): void => {
    event.preventDefault();
    event.stopPropagation();
  };
  /** Include sibling submenu portals and the originating action in outside-click ownership. */
  #contains(target: Node): boolean {
    return this.element.contains(target) || (this.#options?.containsTarget?.(target) ?? false);
  }
}
