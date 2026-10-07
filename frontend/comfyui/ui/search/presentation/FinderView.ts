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
/** Render Substitute's compact Fluent text finder without owning matching or widget state. */
import type { SearchMode } from '../domain/SearchQuery.js';
import {
  applyOwnedFluentTheme,
  createFluentFinderInput,
} from '../../surface/appearance/FluentThemeRuntime.js';
import { readComfyDarkTheme } from '../../surface/appearance/ComfyColorScheme.js';
import type { FluentFinderInput } from '../../surface/appearance/FluentFinderControls.js';
import { ensureFinderStyles } from './FinderStyles.js';
import { ContextMenu } from '../../menus/ContextMenu.js';

export interface FinderActions {
  input(mode: SearchMode, text: string): void;
  cycle(direction: 1 | -1): void;
  close(focusMatch: boolean): void;
}
/** Retain native Fluent controls while finder state and results change. */
export class FinderView {
  readonly element: HTMLElement;
  readonly #input: FluentFinderInput;
  readonly #mode: HTMLElement;
  readonly #modeLabel: HTMLElement;
  readonly #menu: ContextMenu;
  readonly #next: HTMLElement;
  readonly #previous: HTMLElement;
  readonly #clear: HTMLElement;
  readonly #status: HTMLElement;
  readonly #actions: FinderActions;
  #context: SearchMode = 'text';
  #syncing = false;

  /** Match the reference's 296px input group, context selector and ordered icon buttons. */
  constructor(documentRef: Document, actions: FinderActions) {
    this.#actions = actions;
    ensureFinderStyles(documentRef);
    this.element = documentRef.createElement('div');
    this.element.className = 'sc-masonry-finder';
    this.element.setAttribute('role', 'search');
    this.element.setAttribute('aria-label', 'Find in masonry');
    this.element.setAttribute('data-capture-wheel', 'true');
    this.element.hidden = true;
    applyOwnedFluentTheme(this.element, readComfyDarkTheme(documentRef));
    this.#input = createFluentFinderInput(documentRef);
    this.#input.className = 'sc-masonry-finder__input';
    this.#input.setAttribute('aria-label', 'Search masonry');
    this.#input.setAttribute('placeholder', 'Search…');
    this.#input.setAttribute('control-size', 'medium');
    this.#menu = new ContextMenu(documentRef);
    this.#mode = documentRef.createElement('fluent-button');
    this.#mode.className = 'sc-masonry-finder__mode';
    this.#mode.setAttribute('slot', 'start');
    this.#mode.setAttribute('aria-label', 'Search context');
    this.#mode.setAttribute('appearance', 'transparent');
    this.#mode.setAttribute('aria-haspopup', 'menu');
    this.#mode.setAttribute('aria-expanded', 'false');
    this.#modeLabel = documentRef.createElement('span');
    const chevron = documentRef.createElement('span');
    chevron.className = 'sc-masonry-finder__chevron';
    chevron.setAttribute('aria-hidden', 'true');
    this.#mode.append(this.#modeLabel, chevron);
    this.#mode.addEventListener('click', this.#openContext);
    this.#clear = this.#button('Clear search', 'dismiss', () => {
      this.#input.value = '';
      this.#change();
      this.#input.control.focus();
    });
    const search = this.#button('Search', 'search', () => this.#change());
    const end = documentRef.createElement('span');
    end.className = 'sc-masonry-finder__input-actions';
    end.slot = 'end';
    end.append(this.#clear, search);
    this.#input.append(this.#mode, end);
    this.#next = this.#button('Next match (Enter)', 'arrow-down', () => actions.cycle(1));
    this.#previous = this.#button('Previous match (Shift+Enter)', 'arrow-up', () =>
      actions.cycle(-1),
    );
    const close = this.#button('Close search (Esc)', 'dismiss', () => actions.close(false));
    this.#status = documentRef.createElement('span');
    this.#status.className = 'sc-masonry-finder__status';
    this.#status.setAttribute('role', 'status');
    this.#status.setAttribute('aria-live', 'polite');
    this.element.append(this.#input, this.#next, this.#previous, close, this.#status);
    this.#input.addEventListener('input', this.#change);
    this.element.addEventListener('keydown', this.#key);
    this.element.addEventListener('pointerdown', this.#stop);
    this.element.addEventListener('wheel', this.#stop);
  }

  /** Reflect consumed context commands without replacing controls or disturbing normal input selection. */
  sync(mode: SearchMode, text: string, count: number, index: number, status: string): void {
    this.#syncing = true;
    this.#context = mode;
    if (this.#input.value !== text) this.#input.value = text;
    this.#modeLabel.textContent = mode === 'text' ? 'Text' : mode === 'field' ? 'Field' : 'Node';
    this.#clear.hidden = !text;
    const enabled = mode !== 'field' && count > 0;
    for (const button of [this.#next, this.#previous]) button.toggleAttribute('disabled', !enabled);
    this.#status.textContent = count ? `${index + 1} of ${count} matches` : status;
    this.#syncing = false;
  }
  /** Place the overlay in its owning viewport so it stays below native Comfy chrome. */
  show(container: HTMLElement, focus: boolean): void {
    if (this.element.parentElement !== container) container.append(this.element);
    this.element.hidden = false;
    this.#input.control.setAttribute('aria-label', 'Search masonry');
    applyOwnedFluentTheme(this.element, readComfyDarkTheme(this.element.ownerDocument));
    if (focus) {
      this.#input.control.focus();
      this.#input.select();
    }
  }
  /** Retain the query controls while removing the visible overlay. */
  hide(): void {
    this.#menu.close();
    this.element.hidden = true;
  }
  /** Remove the owned overlay and scoped theme on surface release. */
  dispose(): void {
    this.#menu.close();
    applyOwnedFluentTheme(this.element, null);
    this.element.remove();
  }

  /** Emit only validated context values from official Fluent controls. */
  readonly #change = (): void => {
    if (this.#syncing) return;
    this.#actions.input(this.#context, this.#input.value);
  };
  /** Choose a search context through the shared Fluent menu with normal keyboard and focus behavior. */
  readonly #openContext = (): void => {
    const modes: readonly SearchMode[] = ['text', 'field', 'node'];
    this.#mode.setAttribute('aria-expanded', 'true');
    this.#menu.open({
      label: 'Search context',
      anchor: { kind: 'below', bounds: this.#mode.getBoundingClientRect() },
      trigger: this.#mode,
      focusChecked: true,
      closed: () => this.#mode.setAttribute('aria-expanded', 'false'),
      entries: modes.map((mode) => ({
        label: mode === 'text' ? 'Text' : mode === 'field' ? 'Field' : 'Node',
        checked: mode === this.#context,
        selection: 'single',
        select: () => {
          this.#actions.input(mode, this.#input.value);
          this.#input.control.focus();
        },
      })),
    });
  };
  /** Match reference keyboard behavior while preventing graph commands inside the finder. */
  readonly #key = (event: KeyboardEvent): void => {
    if (event.composedPath().includes(this.#input.control)) {
      if (event.key === 'Escape') {
        event.preventDefault();
        this.#actions.close(true);
      } else if (event.key === 'Enter' && this.#context === 'text') {
        event.preventDefault();
        this.#actions.cycle(event.shiftKey ? -1 : 1);
      }
    }
    event.stopPropagation();
  };
  /** Keep finder pointer and wheel input out of graph event handlers. */
  readonly #stop = (event: Event): void => event.stopPropagation();
  /** Use official Fluent buttons with licensed Microsoft system icons. */
  #button(label: string, icon: string, action: () => void): HTMLElement {
    const documentRef = this.element.ownerDocument;
    const button = documentRef.createElement('fluent-button');
    button.setAttribute('appearance', 'transparent');
    button.setAttribute('aria-label', label);
    button.title = label;
    const image = documentRef.createElement('span');
    image.className = 'sc-masonry-finder__icon';
    const url = new URL(`../../../../icons/search/${icon}.svg`, import.meta.url);
    image.style.setProperty('--sc-finder-icon', `url("${url.href}")`);
    image.setAttribute('aria-hidden', 'true');
    button.append(image);
    button.addEventListener('click', action);
    return button;
  }
}
