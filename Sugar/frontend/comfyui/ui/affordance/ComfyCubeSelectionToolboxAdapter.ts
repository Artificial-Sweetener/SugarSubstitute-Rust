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
/** Adapt Comfy's floating selection toolbox for one selected Cube. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type {
  ComfyCubeSaveButtonPresentationState,
  ComfyCubeSaveButtonPresenter,
} from './ComfyCubeSaveButtonPresenter.js';
import type { ComfyCubeCardVisibilityToolboxPresenter } from './ComfyCubeCardVisibilityToolboxPresenter.js';

export interface CubeVersionToolboxOwner {
  present(anchor: HTMLButtonElement, node: CubeNode): HTMLElement;
  clear(): void;
  dispose(): void;
}

const HIDDEN_ATTRIBUTE = 'data-sugarcubes-hidden-affordance';
const QUICK_ACTION_SELECTORS = [
  '[data-testid="convert-to-subgraph-button"]',
  '[data-testid="info-button"]',
] as const;

/** Own reversible mutations and Cube-owned actions inside Comfy's selection toolbox. */
export class ComfyCubeSelectionToolboxAdapter {
  readonly #document: Document;
  readonly #saveButton: ComfyCubeSaveButtonPresenter;
  readonly #cardVisibility: ComfyCubeCardVisibilityToolboxPresenter;
  readonly #versions: CubeVersionToolboxOwner;
  readonly #hiddenStates = new Map<HTMLElement, HiddenState>();
  readonly #saveButtonStates = new Map<HTMLButtonElement, ComfyCubeSaveButtonPresentationState>();

  /** Bind focused presenters to the stable selection-toolbox host anchor. */
  constructor(options: {
    document: Document;
    saveButton: ComfyCubeSaveButtonPresenter;
    cardVisibility: ComfyCubeCardVisibilityToolboxPresenter;
    versions: CubeVersionToolboxOwner;
  }) {
    this.#document = options.document;
    this.#saveButton = options.saveButton;
    this.#cardVisibility = options.cardVisibility;
    this.#versions = options.versions;
  }

  /** Reconcile the toolbox for one selected Cube, or restore it for other selections. */
  present(cube: CubeNode | null): void {
    if (!cube) {
      this.clear();
      return;
    }
    const toolbox = this.#document.querySelector<HTMLElement>('[data-testid="selection-toolbox"]');
    if (!toolbox) {
      this.#cardVisibility.clear();
      return;
    }
    for (const selector of QUICK_ACTION_SELECTORS) {
      for (const element of toolbox.querySelectorAll<HTMLElement>(selector)) this.#hide(element);
    }
    const saveButton = this.#labelIconButton(toolbox, 'i[class*="lucide--book-open"]', 'Save Cube');
    if (saveButton) {
      const identity = cube.properties.sugarcubes_cube;
      const hasVersion =
        identity &&
        typeof identity === 'object' &&
        typeof Reflect.get(identity, 'cube_id') === 'string' &&
        typeof Reflect.get(identity, 'cube_version') === 'string' &&
        cube.properties.sugarcubes_kind !== 'cube_draft';
      const anchor = hasVersion ? this.#versions.present(saveButton, cube) : saveButton;
      if (!hasVersion) this.#versions.clear();
      this.#cardVisibility.present(anchor, cube);
    } else {
      this.#versions.clear();
      this.#cardVisibility.clear();
    }
    this.#hideIconButton(toolbox, 'i[class*="lucide--settings-2"]');
  }

  /** Restore every host mutation owned by this toolbox adapter. */
  clear(): void {
    this.#cardVisibility.clear();
    this.#versions.clear();
    for (const [element, state] of this.#hiddenStates) {
      element.hidden = state.hidden;
      if (state.display) element.style.setProperty('display', state.display, state.priority);
      else element.style.removeProperty('display');
      element.removeAttribute(HIDDEN_ATTRIBUTE);
    }
    this.#hiddenStates.clear();
    for (const [element, state] of this.#saveButtonStates) this.#saveButton.restore(element, state);
    this.#saveButtonStates.clear();
  }

  /** Release Cube-owned toolbox DOM and restore Comfy's controls. */
  dispose(): void {
    this.#cardVisibility.dispose();
    this.#versions.dispose();
    this.clear();
  }

  /** Hide one host affordance while retaining its exact restorable presentation. */
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

  /** Hide one icon-identified host button without taking ownership of its DOM. */
  #hideIconButton(container: HTMLElement, selector: string): void {
    const button = container.querySelector(selector)?.closest<HTMLElement>('button');
    if (button) this.#hide(button);
  }

  /** Adapt one native icon button through the focused Save Cube presenter. */
  #labelIconButton(
    container: HTMLElement,
    selector: string,
    label: string,
  ): HTMLButtonElement | null {
    const icon = container.querySelector<HTMLElement>(selector);
    if (!icon) return null;
    const button = icon.closest<HTMLButtonElement>('button');
    if (!button) return null;
    const state = this.#saveButton.present(button, icon, label, this.#saveButtonStates.get(button));
    if (!this.#saveButtonStates.has(button)) this.#saveButtonStates.set(button, state);
    return button;
  }
}

interface HiddenState {
  hidden: boolean;
  display: string;
  priority: string;
}
