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
/** Hold background model data back from a prompt while a person is using it. */
import type { LoraResourceAdapter } from '../../../../sugarprompt/services/src/index.js';

/** What the admitted data is applied to. */
export interface PromptCatalogTarget {
  /** Replace the editor's LoRA catalog; this retires its open menus, popups and gestures. */
  catalog(adapter: LoraResourceAdapter): void;
  /** Show the catalog's current LoRA images on the prompt's chips. */
  media(): void;
}

/**
 * Decide when one prompt editor receives model data that arrived in the background.
 *
 * The model library publishes while models are being looked up, which can go on for hours.
 * Replacing an editor's catalog closes its menu and completion popup and cancels a drag in
 * progress, so:
 * - a new catalog waits while the prompt has focus, its menu is open or a pointer is held on it;
 * - new chip images wait only while its menu is open or a pointer is held on it.
 * Whatever waited is applied, newest only, the moment the prompt is free.
 */
export class PromptCatalogAdmission {
  readonly #element: HTMLElement;
  readonly #target: PromptCatalogTarget;
  readonly #window: Window | null;
  #menuOpen = false;
  #pointerHeld = false;
  #catalog: LoraResourceAdapter | null = null;
  #media = false;
  #admitted = false;

  /** Watch the prompt's own focus and pointer; the menu's owner reports the menu. */
  constructor(element: HTMLElement, target: PromptCatalogTarget) {
    this.#element = element;
    this.#target = target;
    this.#window = element.ownerDocument.defaultView;
    element.addEventListener('pointerdown', this.#pressed, true);
    element.addEventListener('focusout', this.#released);
    this.#window?.addEventListener('pointerup', this.#released, true);
    this.#window?.addEventListener('pointercancel', this.#released, true);
  }

  /** Follow the prompt's menu, which takes focus out of the prompt while it is open. */
  menuPresented(open: boolean): void {
    this.#menuOpen = open;
    if (!open) this.#settleSoon();
  }

  /** Apply a catalog now, or keep the newest one until the prompt is free. */
  offerCatalog(adapter: LoraResourceAdapter): void {
    // The first catalog replaces nothing a person could be using.
    if (!this.#admitted || !this.#busy(true)) this.#applyCatalog(adapter);
    else this.#catalog = adapter;
  }

  /** Refresh chip images now, or once the menu is closed and the pointer released. */
  offerMedia(): void {
    if (this.#busy(false)) this.#media = true;
    else this.#target.media();
  }

  /** Stop watching; nothing held is applied to a disposed editor. */
  dispose(): void {
    this.#element.removeEventListener('pointerdown', this.#pressed, true);
    this.#element.removeEventListener('focusout', this.#released);
    this.#window?.removeEventListener('pointerup', this.#released, true);
    this.#window?.removeEventListener('pointercancel', this.#released, true);
    this.#catalog = null;
    this.#media = false;
  }

  readonly #pressed = (): void => {
    this.#pointerHeld = true;
  };

  readonly #released = (event: Event): void => {
    if (event.type !== 'focusout') this.#pointerHeld = false;
    this.#settleSoon();
  };

  /** Settle after the event that freed the prompt has finished moving focus. */
  #settleSoon(): void {
    queueMicrotask(() => this.#settle());
  }

  #settle(): void {
    if (this.#catalog && !this.#busy(true)) this.#applyCatalog(this.#catalog);
    else if (this.#media && !this.#busy(false)) {
      this.#media = false;
      this.#target.media();
    }
  }

  /** A new catalog carries its own images, so held images are covered by it. */
  #applyCatalog(adapter: LoraResourceAdapter): void {
    this.#admitted = true;
    this.#catalog = null;
    this.#media = false;
    this.#target.catalog(adapter);
  }

  #busy(focusCounts: boolean): boolean {
    return (
      this.#menuOpen ||
      this.#pointerHeld ||
      (focusCounts && this.#element.contains(this.#element.ownerDocument.activeElement))
    );
  }
}
