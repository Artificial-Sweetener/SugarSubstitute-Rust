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
/** Position the metadata HUD inside the authoritative native graph viewport. */
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';

const VIEWPORT_SELECTOR = '#graph-canvas-container > #graph-canvas';
const VIEWPORT_GUTTER_PX = 16;

/** Follow graph allocation and renderer replacement without rediscovering graph contents or popups. */
export class CubeEditorHudPositioner {
  readonly #presence: HostSurfaceObserver;
  readonly #resize: ResizeObserver | null;
  #viewport: HTMLElement | null = null;
  #queued = false;
  #disposed = false;

  /** Bind one HUD to actual canvas bounds, independently of toolbar visibility or window layout. */
  constructor(
    private readonly document: Document,
    private readonly root: HTMLElement,
  ) {
    const Observer = document.defaultView?.ResizeObserver;
    this.#resize = Observer ? new Observer(() => this.#schedule()) : null;
    this.#presence = new HostSurfaceObserver(
      document.body,
      '#graph-canvas-container, #graph-canvas',
      () => this.#schedule(),
      { attributes: ['class', 'style', 'data-sugarcubes-masonry'] },
    );
    document.defaultView?.addEventListener('resize', this.#schedule);
    this.sync();
  }

  /** Release geometry observation and discard pending callbacks when this HUD retires. */
  dispose(): void {
    this.#disposed = true;
    this.#presence.disconnect();
    this.#resize?.disconnect();
    this.document.defaultView?.removeEventListener('resize', this.#schedule);
  }

  /** Keep the entire HUD within the current native graph rectangle and hide it before a viewport exists. */
  sync(): void {
    if (this.#disposed) return;
    const viewport = this.document.querySelector<HTMLElement>(VIEWPORT_SELECTOR);
    if (viewport !== this.#viewport) {
      this.#resize?.disconnect();
      this.#viewport = viewport;
      if (viewport) this.#resize?.observe(viewport);
    }
    const bounds = viewport?.getBoundingClientRect();
    const style = viewport ? this.document.defaultView?.getComputedStyle(viewport) : null;
    const visible =
      !!bounds &&
      bounds.width > 0 &&
      bounds.height > 0 &&
      style?.display !== 'none' &&
      style?.visibility !== 'hidden';
    const hidden = !visible;
    if (this.root.hidden !== hidden) this.root.hidden = hidden;
    if (!visible || !bounds) return;
    const gutterX = Math.min(VIEWPORT_GUTTER_PX, bounds.width / 2);
    const gutterY = Math.min(VIEWPORT_GUTTER_PX, bounds.height / 2);
    const left = Math.ceil(bounds.left + gutterX);
    const top = Math.ceil(bounds.top + gutterY);
    this.#set('--sugarcubes-cube-editor-metadata-left', `${left}px`);
    this.#set('--sugarcubes-cube-editor-metadata-top', `${top}px`);
    this.#set('max-width', `${Math.max(0, Math.floor(bounds.right - left - gutterX))}px`);
    this.#set('max-height', `${Math.max(0, Math.floor(bounds.bottom - top - gutterY))}px`);
  }

  /** Coalesce accepted viewport geometry changes without waking on unrelated popup activity. */
  readonly #schedule = (): void => {
    if (this.#disposed || this.#queued) return;
    this.#queued = true;
    queueMicrotask(() => {
      this.#queued = false;
      this.sync();
    });
  };

  /** Preserve mounted controls and avoid producing redundant geometry mutations. */
  #set(property: string, value: string): void {
    if (this.root.style.getPropertyValue(property) !== value)
      this.root.style.setProperty(property, value);
  }
}
