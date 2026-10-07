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
/** Own floating coordinates and docking while retaining one mounted toolbar. */
/** Describe viewport bounds and the default floating origin supplied by native chrome. */
export interface OverrideToolbarFloatingSurface {
  bounds: Pick<DOMRect, 'left' | 'right' | 'top' | 'bottom' | 'width' | 'height'>;
  origin: { x: number; y: number };
}

/** Adapt native placement surfaces without exposing host discovery to pointer gestures. */
export interface OverrideToolbarHost {
  attach(element: HTMLElement, docked: boolean): boolean;
  floatingBounds(): OverrideToolbarFloatingSurface | null;
  dockBounds(): DOMRect | null;
  canDock(): boolean;
  dragging(active: boolean): void;
  preview(active: boolean): void;
  dispose(): void;
}
interface Position {
  x: number;
  y: number;
}
interface Gesture {
  start: Position;
  grip: Position;
  moved: boolean;
}

/** Reconcile host placement and settled sizes without observing disclosure animation frames. */
export class OverrideToolbarPlacement {
  #docked = false;
  #parent: HTMLElement | null = null;
  #observer: ResizeObserver | null = null;
  #frame: number | null = null;
  #desired: Position | null = null;
  #applied: Position = { x: 0, y: 0 };
  #size = { width: 0, height: 0 };
  #gesture: Gesture | null = null;
  #pointerPlaced = false;

  /** Bind the host boundary and publish placement changes to disclosure presentation. */
  constructor(
    private readonly root: HTMLElement,
    private readonly grip: HTMLElement,
    private readonly host: OverrideToolbarHost,
    private readonly changed: (docked: boolean) => void,
    private readonly settled: () => void,
  ) {
    root.dataset.placement = 'floating';
  }

  /** Reattach after native shell replacement, preserving the requested resting placement. */
  attached(): boolean {
    if (!this.host.attach(this.root, this.#docked)) {
      if (!this.#docked) return false;
      this.#desired = null;
      this.#pointerPlaced = false;
      this.#setDocked(false);
      if (!this.host.attach(this.root, false)) return false;
    }
    const parent = this.root.closest<HTMLElement>('.p-splitterpanel') ?? this.root.parentElement;
    if (this.#parent !== parent) {
      this.#observer?.disconnect();
      this.#observer = null;
      this.#parent = parent;
      const Observer = this.root.ownerDocument.defaultView?.ResizeObserver;
      if (Observer && parent) {
        this.#observer = new Observer(this.#hostResized);
        this.#observer.observe(parent);
      }
    }
    this.#place();
    this.#schedule();
    return true;
  }

  /** Clamp against the disclosure destination rather than its moving intermediate bounds. */
  resize(width: number, height: number): void {
    this.#size = { width, height };
    this.#place();
  }

  /** Capture the pointer's position inside the grip before either placement can change. */
  begin(x: number, y: number): void {
    const grip = this.grip.getBoundingClientRect();
    this.#gesture = { start: { x, y }, grip: { x: x - grip.left, y: y - grip.top }, moved: false };
    const bounds = this.root.getBoundingClientRect();
    this.resize(bounds.width, bounds.height);
  }

  /** Undock after deliberate movement and follow the grip without translating popup containers. */
  move(x: number, y: number): void {
    const gesture = this.#gesture;
    if (!gesture || (!gesture.moved && Math.hypot(x - gesture.start.x, y - gesture.start.y) < 4))
      return;
    gesture.moved = true;
    this.#pointerPlaced = true;
    if (this.#docked) {
      this.#setDocked(false);
      this.attached();
      const bounds = this.root.getBoundingClientRect();
      this.#size = { width: bounds.width, height: bounds.height };
    }
    this.host.dragging(true);
    const root = this.root.getBoundingClientRect(),
      grip = this.grip.getBoundingClientRect();
    this.#desired = {
      x: x - gesture.grip.x - (grip.left - root.left),
      y: y - gesture.grip.y - (grip.top - root.top),
    };
    this.#place();
    this.host.preview(this.#nearDock(x, y));
  }

  /** Dock only a completed drop inside the dedicated placeholder. */
  end(x: number, y: number, accepted: boolean): void {
    const gesture = this.#gesture;
    this.#gesture = null;
    const docked = accepted && gesture && gesture.moved && this.#nearDock(x, y);
    this.host.dragging(false);
    if (docked) {
      this.#setDocked(true);
      this.attached();
    }
  }

  /** Offer keyboard docking without requiring a pointer gesture. */
  toggleDock(): void {
    if (!this.#docked && !this.host.canDock()) return;
    this.#setDocked(!this.#docked);
    this.attached();
  }

  /** Move an undocked toolbar through the same bounded coordinate owner. */
  shift(x: number, y: number): void {
    if (this.#docked) {
      this.#setDocked(false);
      this.attached();
    }
    this.#desired = { x: this.#applied.x + x, y: this.#applied.y + y };
    this.#place();
  }

  /** Restore the original floating position without changing disclosure or widget values. */
  reset(): void {
    this.#setDocked(false);
    this.attached();
    this.#desired = null;
    this.#pointerPlaced = false;
    this.#place();
  }

  /** Release host chrome, geometry observation, and queued work at toolbar teardown. */
  dispose(): void {
    this.#observer?.disconnect();
    if (this.#frame !== null)
      this.root.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
    this.#parent = null;
    this.#gesture = null;
    this.host.dispose();
  }

  /** Publish a mode once, releasing viewport coordinates while native layout owns the dock. */
  #setDocked(docked: boolean): void {
    if (docked === this.#docked) return;
    this.#docked = docked;
    this.root.dataset.placement = docked ? 'docked' : 'floating';
    this.root.style.removeProperty('left');
    this.root.style.removeProperty('top');
    this.#applied = { x: 0, y: 0 };
    this.changed(docked);
  }

  /** Accept only the placeholder surface, leaving Run controls outside the drop zone. */
  #nearDock(x: number, y: number): boolean {
    const dock = this.host.dockBounds();
    return (
      !!dock &&
      this.host.canDock() &&
      x >= dock.left &&
      x <= dock.right &&
      y >= dock.top &&
      y <= dock.bottom
    );
  }

  /** Keep the resting toolbar inside a resized pane while allowing active pointer movement. */
  readonly #hostResized = (): void => {
    if (!this.#gesture) this.#pointerPlaced = false;
    this.#schedule();
  };

  /** Coalesce host resizing and leave animated geometry to its settled destination. */
  readonly #schedule = (): void => {
    if (this.#frame !== null) return;
    const window = this.root.ownerDocument.defaultView;
    if (!window?.requestAnimationFrame) return;
    this.#frame = window.requestAnimationFrame(() => {
      this.#frame = null;
      if (!this.host.attach(this.root, this.#docked)) {
        this.attached();
        return;
      }
      if (this.root.hasAttribute('data-sc-override-moving')) this.#place();
      else {
        this.settled();
        const bounds = this.root.getBoundingClientRect();
        this.resize(bounds.width, bounds.height);
      }
    });
  };

  /** Follow viewport coordinates inside the workflow surface without consuming a layout row. */
  #place(): void {
    if (this.#docked || !this.#parent || this.#size.width <= 0) return;
    const surface = this.host.floatingBounds();
    if (!surface) return;
    const panel = surface.bounds;
    if (panel.width <= 0 || panel.height <= 0) return;
    this.#desired ??= surface.origin;
    const minimumX = panel.left + 4;
    // Native dragging may carry a wide bar past the viewport; keep its recovery controls reachable.
    const visibleWidth = this.#pointerPlaced ? Math.min(88, this.#size.width) : this.#size.width;
    const maximumX = Math.max(minimumX, panel.right - 4 - visibleWidth);
    const maximumY = Math.max(panel.top, panel.bottom - 4 - this.#size.height);
    this.#applied = {
      x: Math.max(minimumX, Math.min(maximumX, this.#desired.x)),
      y: Math.max(panel.top, Math.min(maximumY, this.#desired.y)),
    };
    const left = `${this.#applied.x}px`,
      top = `${this.#applied.y}px`;
    if (this.root.style.left !== left) this.root.style.left = left;
    if (this.root.style.top !== top) this.root.style.top = top;
  }
}
