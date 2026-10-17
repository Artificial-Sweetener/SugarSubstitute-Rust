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
import { FloatingSurface } from '../menus/FloatingSurface.js';
import { createMenuItem, createMenuList } from '../menus/MenuControlRuntime.js';

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
  #model: CubeAddMenuModel | null = null;
  #showOtherModels = false;
  #surface: FloatingSurface | null = null;

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
    const results = createMenuList(this.#document);
    results.className = 'sugarcubes-add-cube-menu__results';
    results.setAttribute('role', 'menu');
    menu.append(search, results);
    this.#menu = menu;
    this.#surface = new FloatingSurface(menu);
    this.#surface.open({ anchor: { kind: 'below', bounds: anchor }, dismiss: this.close });
    this.#renderResults('');
    this.#document.addEventListener('keydown', this.#handleKeyDown, true);
    search.focus();
  }

  /** Report whether this presenter owns a connected menu. */
  isOpen(): boolean {
    return this.#menu?.isConnected ?? false;
  }

  /** Close the menu and release every transient listener. */
  readonly close = (): void => {
    this.#surface?.close();
    this.#surface = null;
    this.#menu = null;
    this.#model = null;
    this.#showOtherModels = false;
    this.#document.removeEventListener('keydown', this.#handleKeyDown, true);
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
    this.#surface?.reposition();
  }

  /** Build one literal-safe candidate row. */
  #createCandidateRow(candidate: CubeAddCandidate): HTMLElement {
    const row = createMenuItem(this.#document);
    row.setAttribute(RESULT_ATTRIBUTE, candidate.type);
    row.setAttribute('role', 'menuitem');
    const icon = createCubeAddIconElement(this.#document);
    icon.slot = 'start';
    row.append(icon);
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
  #createOtherModelsRow(count: number): HTMLElement {
    const row = createMenuItem(this.#document);
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
      [${MENU_ATTRIBUTE}] {
        display: flex; flex-direction: column; width: min(24rem, calc(100vw - 0.5rem));
        max-height: min(32rem, calc(100vh - 0.5rem)); padding: 0.5rem; gap: 0.375rem;
        border: 1px solid var(--sc-border-subtle); border-radius: var(--sc-control-radius);
        color: var(--sc-text); background: var(--sc-surface);
        box-shadow: var(--sc-shadow-medium); font:var(--sc-font-weight) var(--sc-font-size)/var(--lineHeightBase300) var(--sc-font-family); }
      [${MENU_ATTRIBUTE}] input { box-sizing: border-box; width: 100%; flex: 0 0 auto;
        padding: 0.55rem 0.65rem; border: 1px solid var(--sc-border); border-bottom:2px solid var(--sc-border-accessible);
        border-radius: var(--sc-control-radius); color: inherit; background: var(--sc-surface); font:inherit; }
      [${MENU_ATTRIBUTE}] input:focus { outline:0; border-bottom-color:var(--sc-brand-border); }
      .sugarcubes-add-cube-menu__results { min-height: 0; max-inline-size:none; box-shadow:none; border:0; padding:0; overflow-y:auto; overscroll-behavior:contain; }
      [${RESULT_ATTRIBUTE}] { content-visibility:auto; contain-intrinsic-size:auto 2.75rem; }
      .sugarcubes-add-cube-menu__result-title { min-width: 0; font-weight: 600; }
      .sugarcubes-add-cube-menu__empty { margin: 0; padding: 0.75rem; opacity: 0.7; }
    `;
    this.#document.head.append(style);
  }
}
