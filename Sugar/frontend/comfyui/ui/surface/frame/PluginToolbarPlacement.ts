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
/** Place existing owned frame controls in the native toolbar without moving host controls. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { relocateControl } from '../../core/ControlPlacement.js';
import { applyOwnedTheme } from '../appearance/ThemeService.js';

/** Retain application menu and generation identities through native toolbar replacement. */
export class PluginToolbarPlacement {
  readonly #start: HTMLElement;
  readonly #presence: HostSurfaceObserver;
  readonly #size: ResizeObserver | null;
  #bar: HTMLElement | null = null;
  #dark: boolean | null = null;

  /** Borrow owned controls and restore them to their frame on mode exit. */
  constructor(
    private readonly document: Document,
    private readonly frame: HTMLElement,
    private readonly applicationMenu: HTMLElement,
    private readonly generation: HTMLElement,
    private readonly placed: () => void,
  ) {
    this.#start = document.createElement('div');
    this.#start.className = 'sc-plugin-toolbar-start';
    const Observer = document.defaultView?.ResizeObserver;
    this.#size = Observer ? new Observer(placed) : null;
    this.#presence = new HostSurfaceObserver(document.body, '.actionbar-container', () =>
      this.#attach(),
    );
    this.#attach();
  }

  /** Theme the relocated controls through the same accepted frame palette. */
  setPalette(dark: boolean): void {
    this.#dark = dark;
    if (this.#bar) applyOwnedTheme(this.#bar, dark);
  }

  /** Restore every owned control before the frame or generation owner is disposed. */
  dispose(): void {
    this.#presence.disconnect();
    this.#size?.disconnect();
    this.#releaseBar();
    relocateControl(this.applicationMenu, this.frame);
    relocateControl(this.generation, this.frame);
    this.#start.remove();
    this.#bar = null;
    this.placed();
  }

  /** Release the toolbar's temporary shared skin when its native surface leaves Plugin Mode. */
  #releaseBar(): void {
    if (!this.#bar) return;
    this.#size?.unobserve(this.#bar);
    applyOwnedTheme(this.#bar, null);
    this.#bar.removeAttribute('data-sc-plugin-toolbar');
    this.#bar.removeAttribute('data-sc-style-theme-root');
  }

  /** Follow only an identified native toolbar mount and leave its view toggles in place. */
  #attach(): void {
    const bar = this.document.querySelector<HTMLElement>('.actionbar-container');
    if (bar === this.#bar) return;
    this.#releaseBar();
    this.#bar = bar;
    if (!bar) {
      relocateControl(this.applicationMenu, this.frame);
      relocateControl(this.generation, this.frame);
      this.#start.remove();
      this.placed();
      return;
    }
    this.#size?.observe(bar);
    bar.setAttribute('data-sc-plugin-toolbar', '');
    bar.setAttribute('data-sc-style-theme-root', '');
    if (this.#dark !== null) applyOwnedTheme(bar, this.#dark);
    relocateControl(this.#start, bar, bar.firstChild);
    relocateControl(this.applicationMenu, this.#start);
    relocateControl(this.generation, bar);
    this.placed();
  }
}
