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
/** Adapt Comfy's hard-coded Vue selection affordances for Cube selections. */

import type { CubeEditorContextResolver } from '../surface/CubeEditorContextResolver.js';
import type {
  ComfyCubeSaveButtonPresentationState,
  ComfyCubeSaveButtonPresenter,
} from './ComfyCubeSaveButtonPresenter.js';

const HIDDEN_ATTRIBUTE = 'data-sugarcubes-hidden-affordance';
const QUICK_ACTION_SELECTORS = [
  '[data-testid="convert-to-subgraph-button"]',
  '[data-testid="info-button"]',
] as const;
const MENU_ICON_SELECTORS = [
  'i[class*="lucide--shrink"]',
  'i[class*="lucide--expand"]',
  'i[class*="lucide--folder-plus"]',
  'i[class*="lucide--settings-2"]:not(.sugarcubes-configure-cube-command)',
  'i[class*="lucide--info"]',
  'i.pi-info-circle',
] as const;

interface SelectionCanvas {
  selectedItems: Set<unknown>;
}

/** Own DOM-only suppression and accessible labels around stable Comfy anchors. */
export class ComfyCubeSelectionSurfaceAdapter {
  readonly #document: Document;
  readonly #canvas: SelectionCanvas;
  readonly #contexts: CubeEditorContextResolver;
  readonly #saveButton: ComfyCubeSaveButtonPresenter;
  readonly #observer: MutationObserver;
  readonly #hiddenStates = new Map<HTMLElement, HiddenState>();
  readonly #saveButtonStates = new Map<HTMLButtonElement, ComfyCubeSaveButtonPresentationState>();
  #scheduled = false;

  /** Bind the document surface and shared semantic selection resolver. */
  constructor(options: {
    document: Document;
    canvas: SelectionCanvas;
    contexts: CubeEditorContextResolver;
    saveButton: ComfyCubeSaveButtonPresenter;
  }) {
    this.#document = options.document;
    this.#canvas = options.canvas;
    this.#contexts = options.contexts;
    this.#saveButton = options.saveButton;
    this.#observer = new MutationObserver(() => this.#schedule());
  }

  /** Observe Vue mounts and renderer switches, then reconcile immediately. */
  install(): void {
    this.#observer.observe(this.#document.body, { childList: true, subtree: true });
    this.refresh();
  }

  /** Restore presentation mutations when Comfy replaces the Cube runtime. */
  dispose(): void {
    this.#observer.disconnect();
    this.#restoreMutations();
  }

  /** Reconcile the mounted selection toolbox and More Options menu. */
  refresh(): void {
    this.#scheduled = false;
    const selection = this.#contexts.resolveSelection(this.#canvas.selectedItems);
    if (!selection.containsCube) {
      this.#restoreMutations();
      return;
    }
    const toolbox = this.#document.querySelector<HTMLElement>('[data-testid="selection-toolbox"]');
    if (toolbox) {
      for (const selector of QUICK_ACTION_SELECTORS) {
        for (const element of toolbox.querySelectorAll<HTMLElement>(selector)) this.#hide(element);
      }
      this.#labelIconButton(toolbox, 'i[class*="lucide--book-open"]', 'Save Cube');
      this.#hideIconButton(toolbox, 'i[class*="lucide--settings-2"]');
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

  /** Coalesce mutation bursts produced by PrimeVue menu mounting. */
  #schedule(): void {
    if (this.#scheduled) return;
    this.#scheduled = true;
    queueMicrotask(() => this.refresh());
  }

  /** Undo only mutations owned by this adapter. */
  #restoreMutations(): void {
    for (const [element, state] of this.#hiddenStates) {
      element.hidden = state.hidden;
      if (state.display) element.style.setProperty('display', state.display, state.priority);
      else element.style.removeProperty('display');
      element.removeAttribute(HIDDEN_ATTRIBUTE);
    }
    this.#hiddenStates.clear();
    for (const [element, state] of this.#saveButtonStates) {
      this.#saveButton.restore(element, state);
    }
    this.#saveButtonStates.clear();
  }

  /** Hide one hard-coded host affordance without removing Vue-owned DOM. */
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

  /** Hide one icon-identified toolbox button owned by Comfy. */
  #hideIconButton(container: HTMLElement, selector: string): void {
    const button = container.querySelector(selector)?.closest<HTMLElement>('button');
    if (button) this.#hide(button);
  }

  /** Adapt one native icon button through the focused PrimeVue host boundary. */
  #labelIconButton(container: HTMLElement, selector: string, label: string): void {
    const icon = container.querySelector<HTMLElement>(selector);
    if (!icon) return;
    const button = icon.closest<HTMLButtonElement>('button');
    if (!button) return;
    const state = this.#saveButton.present(button, icon, label, this.#saveButtonStates.get(button));
    if (!this.#saveButtonStates.has(button)) this.#saveButtonStates.set(button, state);
  }
}

interface HiddenState {
  hidden: boolean;
  display: string;
  priority: string;
}
