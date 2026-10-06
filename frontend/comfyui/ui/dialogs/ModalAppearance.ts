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
/** Own the shared modal's Fluent palette lifecycle without changing form state. */
import { applyOwnedFluentTheme } from '../surface/appearance/FluentThemeRuntime.js';
import { readComfyDarkTheme } from '../surface/appearance/ComfyColorScheme.js';
import { injectModalAppearanceStyles } from './ModalAppearanceStyles.js';

/** Keep every open SugarCubes dialog synchronized with Comfy's active palette. */
export class ModalAppearance {
  #observer: MutationObserver | null = null;
  #overlay: HTMLElement | null = null;
  #dark: boolean | null = null;

  /** Activate shared styling and observe only host palette attributes. */
  open(overlay: HTMLElement): void {
    this.close();
    const document = overlay.ownerDocument;
    injectModalAppearanceStyles(document);
    this.#overlay = overlay;
    overlay.dataset.scModalAppearance = 'fluent';
    this.#refresh();
    const Observer = document.defaultView?.MutationObserver;
    if (!Observer) return;
    this.#observer = new Observer(() => this.#refresh());
    for (const target of [
      document.documentElement,
      document.body,
      document.getElementById('vue-app'),
    ]) {
      if (target)
        this.#observer.observe(target, { attributes: true, attributeFilter: ['class', 'style'] });
    }
  }

  /** Release observation and scoped tokens when the dialog closes. */
  close(): void {
    this.#observer?.disconnect();
    this.#observer = null;
    if (this.#overlay) {
      applyOwnedFluentTheme(this.#overlay, null);
      delete this.#overlay.dataset.scModalAppearance;
    }
    this.#overlay = null;
    this.#dark = null;
  }

  /** Apply official tokens only when the host changes palette. */
  #refresh(): void {
    if (!this.#overlay) return;
    const dark = readComfyDarkTheme(this.#overlay.ownerDocument);
    if (dark === this.#dark) return;
    this.#dark = dark;
    applyOwnedFluentTheme(this.#overlay, dark);
  }
}
