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
/** Render the optional-card menu outside all Cube and native-node geometry. */

import type { CubeFaceCardMenuEntry } from './CubeFaceCardPolicy.js';
import { createComfyPrimeIconElement } from './ComfyPrimeIcons.js';

const POPUP_GAP = 6;
const VIEWPORT_INSET = 8;

/** Own one external reveal popup and its transient interaction state. */
export class CubeFaceRevealMenuView {
  readonly button: HTMLButtonElement;
  readonly #document: Document;
  readonly #window: Window;
  readonly #menu: HTMLDivElement;

  /** Create a stable header button and a document-level popup layer. */
  constructor(documentRef: Document) {
    const windowRef = documentRef.defaultView;
    if (!windowRef) throw new Error('Cube reveal menus require a browser window.');
    this.#document = documentRef;
    this.#window = windowRef;
    this.button = createMenuButton(documentRef);
    this.#menu = documentRef.createElement('div');
    this.#menu.className = 'sugarcubes-cube-face__card-menu';
    this.#menu.dataset.cubeCardMenu = '';
    this.#menu.setAttribute('role', 'menu');
    this.#menu.hidden = true;
    this.button.addEventListener('click', this.#onButtonClick);
  }

  /** Replace policy-owned entries without touching the Cube node subtree. */
  render(
    entries: readonly CubeFaceCardMenuEntry[],
    onRevealChange: (nodeId: string, revealed: boolean) => void,
  ): void {
    const rows = entries.map((entry) => {
      const label = this.#document.createElement('label');
      label.className = 'sugarcubes-cube-face__card-menu-row';
      label.setAttribute('role', 'menuitemcheckbox');
      label.setAttribute('aria-checked', String(entry.revealed));
      const checkbox = this.#document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.checked = entry.revealed;
      checkbox.dataset.cubeCardReveal = entry.id;
      checkbox.addEventListener('change', () => onRevealChange(entry.id, checkbox.checked));
      const text = this.#document.createElement('span');
      text.textContent = entry.label;
      label.append(checkbox, text);
      return label;
    });
    this.#menu.replaceChildren(...rows);
    this.button.hidden = rows.length === 0;
    if (rows.length === 0) this.#setOpen(false);
  }

  /** Remove the external popup and all document-level listeners. */
  dispose(): void {
    this.#setOpen(false);
    this.button.removeEventListener('click', this.#onButtonClick);
    this.#menu.remove();
  }

  /** Toggle only transient popup state without changing node layout. */
  readonly #onButtonClick = (event: MouseEvent): void => {
    event.preventDefault();
    event.stopPropagation();
    this.#setOpen(this.#menu.hidden);
  };

  /** Close when interaction leaves both the stable button and external popup. */
  readonly #onDocumentPointerDown = (event: PointerEvent): void => {
    const target = event.target;
    if (!(target instanceof Node)) return;
    if (this.button.contains(target) || this.#menu.contains(target)) return;
    this.#setOpen(false);
  };

  /** Close transient state without moving focus or changing face geometry. */
  readonly #onDocumentKeyDown = (event: KeyboardEvent): void => {
    if (event.key === 'Escape') this.#setOpen(false);
  };

  /** Keep the popup aligned when the browser viewport changes. */
  readonly #onViewportChange = (): void => {
    if (!this.#menu.hidden) this.#position();
  };

  /** Synchronize popup visibility and temporary global listeners. */
  #setOpen(open: boolean): void {
    if (open === !this.#menu.hidden) return;
    this.#menu.hidden = !open;
    this.button.setAttribute('aria-expanded', String(open));
    if (open) {
      this.#document.body.append(this.#menu);
      this.#position();
      this.#document.addEventListener('pointerdown', this.#onDocumentPointerDown, true);
      this.#document.addEventListener('keydown', this.#onDocumentKeyDown, true);
      this.#window.addEventListener('resize', this.#onViewportChange);
      this.#window.addEventListener('scroll', this.#onViewportChange, true);
    } else {
      this.#document.removeEventListener('pointerdown', this.#onDocumentPointerDown, true);
      this.#document.removeEventListener('keydown', this.#onDocumentKeyDown, true);
      this.#window.removeEventListener('resize', this.#onViewportChange);
      this.#window.removeEventListener('scroll', this.#onViewportChange, true);
      this.#menu.remove();
    }
  }

  /** Anchor the fixed popup to its button without participating in node flow. */
  #position(): void {
    const rect = this.button.getBoundingClientRect();
    const top = Math.min(
      Math.max(VIEWPORT_INSET, rect.bottom + POPUP_GAP),
      Math.max(VIEWPORT_INSET, this.#window.innerHeight - VIEWPORT_INSET),
    );
    this.#menu.style.top = `${String(top)}px`;
    this.#menu.style.right = `${String(
      Math.max(VIEWPORT_INSET, this.#window.innerWidth - rect.right),
    )}px`;
    this.#menu.style.maxHeight = `${String(
      Math.max(0, this.#window.innerHeight - top - VIEWPORT_INSET),
    )}px`;
  }
}

/** Build the only reveal-menu element that remains inside the native header. */
function createMenuButton(documentRef: Document): HTMLButtonElement {
  const button = documentRef.createElement('button');
  button.type = 'button';
  button.dataset.cubeAction = 'card-menu';
  button.append(createComfyPrimeIconElement(documentRef, 'eye'));
  button.title = 'Reveal optional Cube cards';
  button.setAttribute('aria-label', 'Reveal optional Cube cards');
  button.setAttribute('aria-haspopup', 'menu');
  button.setAttribute('aria-expanded', 'false');
  button.addEventListener('pointerdown', (event) => event.stopPropagation());
  return button;
}
