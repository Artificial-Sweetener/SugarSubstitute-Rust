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
/** Present the searchable, scrollable Add Cube menu with Comfy-native styling. */

import { createCubeAddIconElement } from '../surface/CubeAddIcon.js';
import { resolveCubeModelTitle } from '../cube/CubeModelTitlePresentation.js';
import { createCubeModelTitleElement } from '../surface/CubeModelPillDomRenderer.js';
import type { CubeFaceActionAnchor } from '../surface/CubeFaceChromeActions.js';
import type { CubeAddCandidate, CubeAddCandidateGroups } from './CubeAddCandidateCatalog.js';

const MENU_ATTRIBUTE = 'data-sugarcubes-add-cube-menu';
const RESULT_ATTRIBUTE = 'data-sugarcubes-add-cube-result';
const OTHER_ATTRIBUTE = 'data-sugarcubes-add-cube-other-models';
const STYLE_ID = 'sugarcubes-add-cube-menu-styles';

export interface CubeAddMenuModel {
  groups: CubeAddCandidateGroups;
  search(query: string): readonly CubeAddCandidate[];
  select(type: string): void;
}

/** Own transient menu DOM, search interaction, viewport positioning, and cleanup. */
export class ComfyCubeAddMenuPresenter {
  readonly #document: Document;
  #menu: HTMLElement | null = null;
  #anchor: CubeFaceActionAnchor | null = null;
  #model: CubeAddMenuModel | null = null;
  #showOtherModels = false;

  /** Bind the menu to one Comfy document. */
  constructor(documentRef: Document) {
    this.#document = documentRef;
    this.#ensureStyles();
  }

  /** Toggle a fresh menu at one renderer-neutral viewport anchor. */
  toggle(anchor: CubeFaceActionAnchor, model: CubeAddMenuModel): void {
    if (this.#menu) {
      this.close();
      return;
    }
    this.#anchor = anchor;
    this.#model = model;
    this.#showOtherModels = false;
    const menu = this.#document.createElement('section');
    menu.setAttribute(MENU_ATTRIBUTE, '');
    menu.setAttribute('role', 'dialog');
    menu.setAttribute('aria-label', 'Add Cube');
    const search = this.#document.createElement('input');
    search.type = 'search';
    search.placeholder = 'Search Cubes';
    search.setAttribute('aria-label', 'Search Cubes');
    search.addEventListener('input', () => this.#renderResults(search.value));
    const results = this.#document.createElement('div');
    results.className = 'sugarcubes-add-cube-menu__results';
    results.setAttribute('role', 'menu');
    menu.append(search, results);
    this.#document.body.append(menu);
    this.#menu = menu;
    this.#renderResults('');
    this.#position();
    this.#document.addEventListener('pointerdown', this.#handleOutsidePointer, true);
    this.#document.addEventListener('keydown', this.#handleKeyDown, true);
    this.#document.defaultView?.addEventListener('resize', this.close);
    this.#document.defaultView?.addEventListener('scroll', this.#handleScroll, true);
    search.focus();
  }

  /** Report whether this presenter owns a connected menu. */
  isOpen(): boolean {
    return this.#menu?.isConnected ?? false;
  }

  /** Close the menu and release every transient listener. */
  readonly close = (): void => {
    this.#menu?.remove();
    this.#menu = null;
    this.#anchor = null;
    this.#model = null;
    this.#showOtherModels = false;
    this.#document.removeEventListener('pointerdown', this.#handleOutsidePointer, true);
    this.#document.removeEventListener('keydown', this.#handleKeyDown, true);
    this.#document.defaultView?.removeEventListener('resize', this.close);
    this.#document.defaultView?.removeEventListener('scroll', this.#handleScroll, true);
  };

  /** Release every owned menu resource. */
  dispose(): void {
    this.close();
  }

  /** Render primary, expanded other-model, or cross-model search results. */
  #renderResults(query: string): void {
    const results = this.#menu?.querySelector<HTMLElement>('.sugarcubes-add-cube-menu__results');
    const model = this.#model;
    if (!results || !model) return;
    const normalized = query.trim();
    const candidates = normalized
      ? model.search(normalized)
      : this.#showOtherModels
        ? model.groups.otherModels
        : model.groups.sameModel;
    const rows: HTMLElement[] = candidates.map((candidate) => this.#createCandidateRow(candidate));
    if (rows.length === 0) rows.push(this.#createEmptyRow(normalized));
    if (!normalized && !this.#showOtherModels && model.groups.otherModels.length > 0) {
      rows.push(this.#createOtherModelsRow(model.groups.otherModels.length));
    }
    results.replaceChildren(...rows);
    this.#position();
  }

  /** Build one literal-safe candidate row. */
  #createCandidateRow(candidate: CubeAddCandidate): HTMLButtonElement {
    const row = this.#document.createElement('button');
    row.type = 'button';
    row.setAttribute(RESULT_ATTRIBUTE, candidate.type);
    row.setAttribute('role', 'menuitem');
    row.append(createCubeAddIconElement(this.#document));
    const presentation = resolveCubeModelTitle({
      targetModel: candidate.targetModel,
      title: candidate.displayName,
    });
    row.append(
      createCubeModelTitleElement(this.#document, presentation, {
        className: 'sugarcubes-add-cube-menu__result-title',
      }),
    );
    row.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      const activeModel = this.#model;
      this.close();
      activeModel?.select(candidate.type);
    });
    return row;
  }

  /** Build the bottom-level navigation into boundary-compatible other models. */
  #createOtherModelsRow(count: number): HTMLButtonElement {
    const row = this.#document.createElement('button');
    row.type = 'button';
    row.setAttribute(OTHER_ATTRIBUTE, '');
    row.setAttribute('role', 'menuitem');
    const icon = this.#document.createElement('i');
    icon.className = 'pi pi-chevron-right';
    icon.setAttribute('aria-hidden', 'true');
    const label = this.#document.createElement('span');
    label.textContent = `Other models (${String(count)})`;
    row.append(icon, label);
    row.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      this.#showOtherModels = true;
      this.#renderResults('');
    });
    return row;
  }

  /** Build one non-interactive empty-state row. */
  #createEmptyRow(query: string): HTMLParagraphElement {
    const empty = this.#document.createElement('p');
    empty.className = 'sugarcubes-add-cube-menu__empty';
    empty.textContent = query ? 'No compatible Cubes match this search.' : 'No compatible Cubes.';
    return empty;
  }

  /** Keep the menu inside the viewport while preferring the action's lower edge. */
  #position(): void {
    if (!this.#menu || !this.#anchor) return;
    const windowRef = this.#document.defaultView;
    const viewportWidth = windowRef?.innerWidth ?? this.#document.documentElement.clientWidth;
    const viewportHeight = windowRef?.innerHeight ?? this.#document.documentElement.clientHeight;
    const left = Math.max(
      4,
      Math.min(
        this.#anchor.right - this.#menu.offsetWidth,
        viewportWidth - this.#menu.offsetWidth - 4,
      ),
    );
    const below = this.#anchor.bottom + 4;
    const top =
      below + this.#menu.offsetHeight <= viewportHeight - 4
        ? below
        : Math.max(4, this.#anchor.top - this.#menu.offsetHeight - 4);
    this.#menu.style.left = `${String(left)}px`;
    this.#menu.style.top = `${String(top)}px`;
  }

  /** Close when a different host surface receives the pointer. */
  readonly #handleOutsidePointer = (event: PointerEvent): void => {
    const target = event.target;
    if (!(target instanceof Node) || this.#menu?.contains(target)) return;
    this.close();
  };

  /** Preserve menu-owned scrolling while closing after external viewport movement. */
  readonly #handleScroll = (event: Event): void => {
    const target = event.target;
    if (target instanceof Node && this.#menu?.contains(target)) return;
    this.close();
  };

  /** Close the menu through the standard accessible escape path. */
  readonly #handleKeyDown = (event: KeyboardEvent): void => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    this.close();
  };

  /** Install one theme-aware Comfy menu stylesheet. */
  #ensureStyles(): void {
    if (this.#document.getElementById(STYLE_ID)) return;
    const style = this.#document.createElement('style');
    style.id = STYLE_ID;
    style.textContent = `
      [${MENU_ATTRIBUTE}] { box-sizing: border-box; position: fixed; z-index: 100000;
        display: flex; flex-direction: column; width: min(24rem, calc(100vw - 0.5rem));
        max-height: min(32rem, calc(100vh - 0.5rem)); padding: 0.5rem; gap: 0.375rem;
        border: 1px solid color-mix(in srgb, currentColor 24%, transparent); border-radius: 0.5rem;
        color: var(--input-text, #f0f2f5); background: var(--comfy-menu-bg, #171b20);
        box-shadow: 0 0.5rem 1.5rem rgb(0 0 0 / 45%); }
      [${MENU_ATTRIBUTE}] input { box-sizing: border-box; width: 100%; flex: 0 0 auto;
        padding: 0.55rem 0.65rem; border: 1px solid color-mix(in srgb, currentColor 25%, transparent);
        border-radius: 0.375rem; color: inherit; background: var(--p-inputtext-background, #111418); }
      .sugarcubes-add-cube-menu__results { min-height: 0; overflow-y: auto; overscroll-behavior: contain; }
      [${RESULT_ATTRIBUTE}], [${OTHER_ATTRIBUTE}] { appearance: none; display: grid; width: 100%;
        grid-template-columns: 1.25rem minmax(0, 1fr); align-items: center; gap: 0.625rem;
        padding: 0.55rem; border: 0; border-radius: 0.375rem; color: inherit;
        background: transparent; cursor: pointer; text-align: left; content-visibility: auto;
        contain-intrinsic-size: auto 2.75rem; }
      [${RESULT_ATTRIBUTE}]:hover, [${RESULT_ATTRIBUTE}]:focus-visible,
      [${OTHER_ATTRIBUTE}]:hover, [${OTHER_ATTRIBUTE}]:focus-visible {
        outline: none; background: color-mix(in srgb, currentColor 10%, transparent); }
      .sugarcubes-add-cube-menu__result-title { min-width: 0; font-weight: 600; }
      .sugarcubes-add-cube-menu__empty { margin: 0; padding: 0.75rem; opacity: 0.7; }
    `;
    this.#document.head.append(style);
  }
}
