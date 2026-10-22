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
/** Project Comfy's existing search and close controls into a reversible top toolbar. */
import { SettingsSearchNavigation } from './SettingsSearchNavigation.js';
import { relocateControl } from '../core/ControlPlacement.js';

/** Keep native search controls stable while the settings shell changes presentation. */
export class SettingsSearchPlacement {
  readonly #bar: HTMLElement;
  readonly #moves: { element: HTMLElement; slot: Comment }[] = [];
  readonly #navigation: SettingsSearchNavigation | null;

  /** Preserve native input, popup, focus, keyboard and event-handler identity. */
  constructor(root: HTMLElement) {
    const document = root.ownerDocument;
    this.#bar = document.createElement('header');
    this.#bar.className = 'sc-settings-search-bar sc-settings-owned';
    const title = document.createElement('h2');
    title.textContent = root.querySelector('header h2')?.textContent ?? 'Settings';
    this.#bar.append(title);
    root.prepend(this.#bar);
    const sidebar = root.querySelector('nav');
    const search = sidebar?.querySelector<HTMLInputElement>('input[placeholder]');
    this.#navigation = search ? new SettingsSearchNavigation(root, search) : null;
    let wrapper: HTMLElement | null = search ?? null;
    while (wrapper?.parentElement && wrapper.parentElement !== sidebar)
      wrapper = wrapper.parentElement;
    const close = root.querySelector<HTMLElement>(
      'button[aria-label="Close dialog"], :scope > div > div > header button',
    );
    for (const element of [wrapper, close]) {
      if (!element?.parentNode) continue;
      const slot = document.createComment('SugarCubes settings presentation slot');
      element.before(slot);
      this.#moves.push({ element, slot });
      relocateControl(element, this.#bar, null);
    }
  }

  /** Restore native slots even when a palette or mode changes with the dialog open. */
  dispose(): void {
    this.#navigation?.dispose();
    for (const { element, slot } of this.#moves) {
      if (slot.parentElement) {
        relocateControl(element, slot.parentElement, slot);
        slot.remove();
      }
    }
    this.#bar.remove();
  }

  /** Reconcile native search completion without taking ownership of its result filtering. */
  reconcile(): boolean {
    return this.#navigation?.reconcile() ?? false;
  }
}
