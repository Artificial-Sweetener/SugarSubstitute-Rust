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
/** Allocate a stable graph sibling beneath native toolbar rows and execution overlays. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import type { CanvasFrameClock } from '../application/CanvasCameraTransition.js';
import { ComfyCanvasToolbarLayout } from './ComfyCanvasToolbarLayout.js';

interface WorkspaceBounds {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
  readonly frameWidth: number;
  readonly frameHeight: number;
}

/** Own stable workspace geometry observation beneath the qualified native toolbar. */
export class ComfyCanvasWorkspaceFrame {
  readonly element: HTMLDivElement;
  readonly #resize: ResizeObserver | null;
  readonly #toolbar: ComfyCanvasToolbarLayout;
  #mounts: HostSurfaceObserver | null = null;
  #pending: (() => void) | null = null;
  #bounds: WorkspaceBounds | null = null;
  #disposed = false;

  /** Retain native chrome identity and observe only the qualified workspace and toolbar mounts. */
  constructor(
    private readonly root: HTMLElement,
    private readonly panel: HTMLElement,
    private readonly frames: CanvasFrameClock,
    private readonly changed: (width: number, height: number) => void,
  ) {
    this.element = root.ownerDocument.createElement('div');
    this.element.className = 'sc-output-workspace-frame';
    this.#toolbar = new ComfyCanvasToolbarLayout(panel);
    const Observer = root.ownerDocument.defaultView?.ResizeObserver;
    this.#resize = Observer ? new Observer(this.#schedule) : null;
    try {
      this.#mounts = new HostSurfaceObserver(root, '.actionbar-container', () => {
        this.#observe();
        this.#schedule();
      });
      this.#observe();
      root.append(this.element);
      root.ownerDocument.defaultView?.addEventListener('resize', this.#schedule);
    } catch (error: unknown) {
      this.#resize?.disconnect();
      this.#mounts?.disconnect();
      this.#toolbar.dispose();
      this.element.remove();
      throw error;
    }
  }

  /** Publish the first allocation after the workspace input and attachment owners are ready. */
  measure(): void {
    if (this.#disposed) return;
    this.#toolbar.measure();
    const next = this.#readBounds();
    const previous = this.#bounds;
    if (
      previous &&
      next.left === previous.left &&
      next.top === previous.top &&
      next.width === previous.width &&
      next.height === previous.height &&
      next.frameWidth === previous.frameWidth &&
      next.frameHeight === previous.frameHeight
    )
      return;
    this.#bounds = next;
    for (const property of ['left', 'top', 'width', 'height'] as const) {
      const value = `${next[property]}px`;
      if (this.element.style.getPropertyValue(property) !== value)
        this.element.style.setProperty(property, value);
    }
    if (
      !previous ||
      next.width !== previous.width ||
      next.height !== previous.height ||
      next.frameWidth !== previous.frameWidth ||
      next.frameHeight !== previous.frameHeight
    )
      this.changed(next.width, next.height);
  }

  /** Reanchor sibling pixels synchronously after native allocation moves their shared origin. */
  syncOrigin(): void {
    if (this.#disposed) return;
    const next = this.#readBounds();
    for (const property of ['left', 'top'] as const) {
      const value = `${next[property]}px`;
      if (this.element.style.getPropertyValue(property) !== value)
        this.element.style.setProperty(property, value);
    }
    if (this.#bounds) this.#bounds = { ...this.#bounds, left: next.left, top: next.top };
  }

  /** Measure native content in the current graph coordinate origin without changing host geometry. */
  #readBounds(): WorkspaceBounds {
    const origin = this.root.getBoundingClientRect();
    const panel = this.panel.getBoundingClientRect();
    const frame = this.root.parentElement?.getBoundingClientRect();
    return {
      left: panel.left - origin.left,
      top: panel.top - origin.top,
      width: panel.width,
      height: panel.height,
      frameWidth: frame?.width ?? 0,
      frameHeight: frame?.height ?? 0,
    };
  }

  /** Release geometry scheduling and the owned sibling while leaving every native surface intact. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#pending?.();
    this.#resize?.disconnect();
    this.#mounts?.disconnect();
    this.#toolbar.dispose();
    this.root.ownerDocument.defaultView?.removeEventListener('resize', this.#schedule);
    this.element.remove();
  }

  /** Rebind only identified geometry sources when the host replaces its toolbar. */
  #observe(): void {
    this.#resize?.disconnect();
    for (const element of [
      this.root,
      this.root.parentElement,
      this.panel,
      this.panel.parentElement?.parentElement,
      this.#toolbar.row(),
    ])
      if (element) this.#resize?.observe(element);
  }

  /** Coalesce host resize bursts without scheduling work for arbitrary popup content. */
  readonly #schedule = (): void => {
    if (this.#disposed || this.#pending) return;
    this.#pending = this.frames.request(() => {
      this.#pending = null;
      this.measure();
    });
  };
}
