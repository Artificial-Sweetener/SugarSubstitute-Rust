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
/** Hide incompatible Comfy context-menu commands for Cube selections. */

const HIDDEN_ATTRIBUTE = 'data-sugarcubes-hidden-affordance';
const MENU_ICON_SELECTORS = [
  'i[class*="lucide--shrink"]',
  'i[class*="lucide--expand"]',
  'i[class*="lucide--folder-plus"]',
  'i[class*="lucide--settings-2"]:not(.sugarcubes-configure-cube-command)',
  'i[class*="lucide--info"]',
  'i.pi-info-circle',
] as const;

/** Own reversible visibility changes inside Comfy's transient context menus. */
export class ComfyCubeSelectionMenuVisibilityAdapter {
  readonly #document: Document;
  readonly #hiddenStates = new Map<HTMLElement, HiddenState>();

  /** Bind menu adaptation to one document. */
  constructor(documentRef: Document) {
    this.#document = documentRef;
  }

  /** Reconcile context-menu visibility for the current Cube selection state. */
  present(containsCube: boolean): void {
    if (!containsCube) {
      this.clear();
      return;
    }
    for (const menu of this.#document.querySelectorAll<HTMLElement>('.p-contextmenu')) {
      for (const selector of MENU_ICON_SELECTORS) {
        for (const icon of menu.querySelectorAll<HTMLElement>(selector)) {
          const item = icon.closest<HTMLElement>('li');
          if (item) this.#hide(item);
        }
      }
    }
  }

  /** Restore every host menu item changed by this adapter. */
  clear(): void {
    for (const [element, state] of this.#hiddenStates) {
      element.hidden = state.hidden;
      if (state.display) element.style.setProperty('display', state.display, state.priority);
      else element.style.removeProperty('display');
      element.removeAttribute(HIDDEN_ATTRIBUTE);
    }
    this.#hiddenStates.clear();
  }

  /** Preserve and hide one host menu item exactly once. */
  #hide(element: HTMLElement): void {
    if (!this.#hiddenStates.has(element)) {
      this.#hiddenStates.set(element, {
        hidden: element.hidden,
        display: element.style.getPropertyValue('display'),
        priority: element.style.getPropertyPriority('display'),
      });
    }
    element.hidden = true;
    element.style.setProperty('display', 'none', 'important');
    element.setAttribute(HIDDEN_ATTRIBUTE, 'true');
  }
}

interface HiddenState {
  hidden: boolean;
  display: string;
  priority: string;
}
