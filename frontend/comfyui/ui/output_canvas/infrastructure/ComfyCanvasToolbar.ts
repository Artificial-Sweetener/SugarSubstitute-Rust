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
/** Own the accessible Output Canvas affordance across identified native action-bar mounts. */

import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';

/** Reuse one toolbar button without moving Comfy controls or acquiring image resources. */
export class ComfyCanvasToolbar {
  readonly #button: HTMLButtonElement;
  #observer: HostSurfaceObserver | null = null;
  #disposed = false;

  /** Construct safe static icon/text chrome and bind the shared workspace toggle. */
  constructor(
    private readonly document: Document,
    private readonly toggle: () => void,
  ) {
    this.#button = document.createElement('button');
    this.#button.className = 'sc-output-canvas-toggle';
    this.#button.type = 'button';
    this.#button.setAttribute('aria-label', 'Toggle output canvas');
    const icon = document.createElement('span');
    icon.setAttribute('aria-hidden', 'true');
    icon.style.setProperty(
      '--sc-output-icon',
      `url("${new URL('../../../../icons/node/image.svg', import.meta.url).href}")`,
    );
    this.#button.append(icon);
    this.#button.addEventListener('click', toggle);
  }

  /** Observe only action-bar mounting and local host content replacement. */
  setup(): void {
    if (this.#observer || this.#disposed) return;
    this.#observer = new HostSurfaceObserver(
      this.document.body,
      '.actionbar-container',
      this.#attach,
      { content: true },
    );
    this.#attach();
  }

  /** Reflect actual drawer/workspace availability rather than merely stored desktop visibility. */
  update(visible: boolean, available: boolean): void {
    this.#button.setAttribute('aria-pressed', String(visible));
    this.#button.title = visible ? 'Close output canvas' : 'Open output canvas';
    this.#button.disabled = !available;
  }

  /** Release toolbar observation and the owned control with the canvas host. */
  dispose(): void {
    this.#disposed = true;
    this.#observer?.disconnect();
    this.#button.removeEventListener('click', this.toggle);
    this.#button.remove();
  }

  /** Insert alongside existing SugarCubes view controls, retaining all native node identity. */
  readonly #attach = (): void => {
    if (this.#disposed) return;
    const bar = this.document.querySelector('.actionbar-container');
    const queue = bar?.querySelector('[data-testid="queue-overlay-toggle"]');
    let slot = queue?.parentElement ?? null;
    while (slot && slot.parentElement !== bar) slot = slot.parentElement;
    if (!bar || !slot) {
      this.#button.remove();
      return;
    }
    const masonry = bar.querySelector('.sugarcubes-masonry-toggle');
    const anchor = masonry?.parentElement === bar ? masonry : slot;
    if (this.#button.parentElement !== bar || this.#button.previousElementSibling !== anchor)
      bar.insertBefore(this.#button, anchor.nextSibling);
  };
}
