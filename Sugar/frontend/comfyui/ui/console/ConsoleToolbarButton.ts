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
/** Place the console toggle in ComfyUI's action bar beside SugarCubes' other view toggles. */
import { createFluentIcon } from '../core/FluentIcons.js';
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';

/** One checkable console button, kept in the action bar as ComfyUI remounts it. */
export class ConsoleToolbarButton {
  readonly #button: HTMLButtonElement;
  #observer: HostSurfaceObserver | null = null;
  #disposed = false;

  /** Build the button with its console glyph. */
  constructor(
    private readonly document: Document,
    toggle: () => void,
  ) {
    this.#button = document.createElement('button');
    this.#button.type = 'button';
    this.#button.className = 'sc-console-toggle sc-output-canvas-toggle';
    this.#button.setAttribute('aria-label', 'Toggle console');
    this.#button.append(createFluentIcon(document, 'window-console', 18));
    this.#button.addEventListener('click', toggle);
  }

  /** Follow action-bar mounts and place the button. */
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

  /** Reflect whether the console is open. */
  update(open: boolean): void {
    this.#button.setAttribute('aria-pressed', String(open));
    this.#button.title = open ? 'Hide console' : 'Show console';
  }

  /** Remove the button and stop following the action bar. */
  dispose(): void {
    this.#disposed = true;
    this.#observer?.disconnect();
    this.#button.remove();
  }

  /** Sit after the output canvas toggle, or after the masonry toggle when that is absent. */
  readonly #attach = (): void => {
    if (this.#disposed) return;
    const bar = this.document.querySelector('.actionbar-container');
    if (!bar) {
      this.#button.remove();
      return;
    }
    const canvas = [...bar.querySelectorAll(':scope > .sc-output-canvas-toggle')].find(
      (element) => element !== this.#button,
    );
    const masonry = bar.querySelector(':scope > .sugarcubes-masonry-toggle');
    const anchor = canvas ?? masonry;
    if (!anchor) return;
    if (this.#button.parentElement !== bar || this.#button.previousElementSibling !== anchor)
      bar.insertBefore(this.#button, anchor.nextSibling);
  };
}
