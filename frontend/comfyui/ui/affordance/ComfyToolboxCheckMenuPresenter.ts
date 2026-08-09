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
/** Present one reusable checked-row menu for SugarCubes toolbox controls. */

const MENU_ATTRIBUTE = 'data-sugarcubes-toolbox-check-menu';
const ITEM_ATTRIBUTE = 'data-sugarcubes-toolbox-check-menu-item';
const STYLE_ID = 'sugarcubes-toolbox-check-menu-styles';

export interface ToolboxCheckMenuItem {
  id: string;
  label: string;
  checked: boolean;
}

export interface ToolboxCheckMenuModel {
  ariaLabel: string;
  selectionMode: 'single' | 'multiple';
  items(): readonly ToolboxCheckMenuItem[];
  select(id: string): void;
  closeOnSelect: boolean;
}

/** Own portal DOM, checked-row rendering, positioning, and transient menu events. */
export class ComfyToolboxCheckMenuPresenter {
  readonly #document: Document;
  readonly #featureMenuAttribute: string;
  readonly #featureItemAttribute: string;
  #anchor: HTMLButtonElement | null = null;
  #menu: HTMLElement | null = null;
  #model: ToolboxCheckMenuModel | null = null;
  #signature = '';

  /** Bind feature-specific compatibility attributes to the shared menu surface. */
  constructor(options: {
    document: Document;
    featureMenuAttribute: string;
    featureItemAttribute: string;
  }) {
    this.#document = options.document;
    this.#featureMenuAttribute = options.featureMenuAttribute;
    this.#featureItemAttribute = options.featureItemAttribute;
    this.#ensureStyles();
  }

  /** Toggle the shared menu beside one toolbox action. */
  toggle(anchor: HTMLButtonElement, model: ToolboxCheckMenuModel): void {
    if (this.#menu) {
      this.close();
      return;
    }
    this.#anchor = anchor;
    this.#model = model;
    const menu = this.#document.createElement('div');
    menu.setAttribute(MENU_ATTRIBUTE, '');
    menu.setAttribute(this.#featureMenuAttribute, '');
    menu.setAttribute('role', 'menu');
    menu.setAttribute('aria-label', model.ariaLabel);
    this.#document.body.append(menu);
    this.#menu = menu;
    anchor.setAttribute('aria-expanded', 'true');
    this.refresh(model);
    this.#position();
    this.#document.addEventListener('pointerdown', this.#handleOutsidePointer, true);
    this.#document.addEventListener('keydown', this.#handleKeyDown, true);
    this.#document.defaultView?.addEventListener('resize', this.close);
    this.#document.defaultView?.addEventListener('scroll', this.close, true);
    menu.querySelector<HTMLElement>(`[${ITEM_ATTRIBUTE}]`)?.focus();
  }

  /** Reconcile one open menu without rebuilding unchanged rows. */
  refresh(model: ToolboxCheckMenuModel): void {
    if (!this.#menu) return;
    this.#model = model;
    this.#menu.setAttribute('aria-label', model.ariaLabel);
    const items = model.items();
    if (model.selectionMode === 'single' && items.filter((item) => item.checked).length !== 1) {
      this.close();
      throw new Error('Single-selection toolbox menus require exactly one checked item.');
    }
    const signature = JSON.stringify(items);
    if (signature === this.#signature) return;
    this.#signature = signature;
    const rows = items.map((item) => this.#createRow(item));
    if (rows.length === 0) {
      this.close();
      return;
    }
    this.#menu.replaceChildren(...rows);
  }

  /** Report whether this presenter currently owns a connected menu. */
  isOpen(): boolean {
    return this.#menu?.isConnected ?? false;
  }

  /** Close the menu and release every viewport listener. */
  readonly close = (): void => {
    this.#menu?.remove();
    this.#menu = null;
    this.#signature = '';
    this.#anchor?.setAttribute('aria-expanded', 'false');
    this.#document.removeEventListener('pointerdown', this.#handleOutsidePointer, true);
    this.#document.removeEventListener('keydown', this.#handleKeyDown, true);
    this.#document.defaultView?.removeEventListener('resize', this.close);
    this.#document.defaultView?.removeEventListener('scroll', this.close, true);
  };

  /** Release all owned transient state. */
  dispose(): void {
    this.close();
    this.#anchor = null;
    this.#model = null;
  }

  /** Create one literal checked row and bind its feature-owned selection callback. */
  #createRow(item: ToolboxCheckMenuItem): HTMLButtonElement {
    const row = this.#document.createElement('button');
    row.type = 'button';
    row.setAttribute(ITEM_ATTRIBUTE, item.id);
    row.setAttribute(this.#featureItemAttribute, item.id);
    row.setAttribute(
      'role',
      this.#model?.selectionMode === 'single' ? 'menuitemradio' : 'menuitemcheckbox',
    );
    row.setAttribute('aria-checked', String(item.checked));
    const check = this.#document.createElement('i');
    check.className = item.checked ? 'pi pi-check' : 'pi';
    check.setAttribute('aria-hidden', 'true');
    const label = this.#document.createElement('span');
    label.textContent = item.label;
    row.append(check, label);
    row.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      const model = this.#model;
      if (!model) return;
      model.select(item.id);
      if (model.closeOnSelect) this.close();
      else this.refresh(model);
    });
    return row;
  }

  /** Position the menu below its action or above it when the viewport requires. */
  #position(): void {
    if (!this.#anchor || !this.#menu) return;
    const anchor = this.#anchor.getBoundingClientRect();
    const windowRef = this.#document.defaultView;
    const viewportWidth = windowRef?.innerWidth ?? this.#document.documentElement.clientWidth;
    const viewportHeight = windowRef?.innerHeight ?? this.#document.documentElement.clientHeight;
    const left = Math.max(4, Math.min(anchor.left, viewportWidth - this.#menu.offsetWidth - 4));
    const below = anchor.bottom + 4;
    const top =
      below + this.#menu.offsetHeight <= viewportHeight - 4
        ? below
        : Math.max(4, anchor.top - this.#menu.offsetHeight - 4);
    this.#menu.style.left = `${left}px`;
    this.#menu.style.top = `${top}px`;
  }

  /** Close when a different host surface receives the pointer. */
  readonly #handleOutsidePointer = (event: PointerEvent): void => {
    const target = event.target;
    if (!(target instanceof Node)) return;
    if (this.#menu?.contains(target) || this.#anchor?.contains(target)) return;
    this.close();
  };

  /** Close on Escape and restore focus to the action. */
  readonly #handleKeyDown = (event: KeyboardEvent): void => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    const anchor = this.#anchor;
    this.close();
    anchor?.focus();
  };

  /** Install the single authoritative checked-menu theme. */
  #ensureStyles(): void {
    if (this.#document.getElementById(STYLE_ID)) return;
    const style = this.#document.createElement('style');
    style.id = STYLE_ID;
    style.textContent = `
      [${MENU_ATTRIBUTE}] {
        box-sizing: border-box; position: fixed; z-index: 100000; display: flex;
        flex-direction: column; min-width: 13rem; max-width: min(22rem, 70vw);
        padding: 0.375rem; border: 1px solid color-mix(in srgb, currentColor 24%, transparent);
        border-radius: 0.5rem; color: var(--input-text, #f0f2f5);
        background: var(--comfy-menu-bg, #171b20); box-shadow: 0 0.5rem 1.5rem rgb(0 0 0 / 45%);
      }
      [${ITEM_ATTRIBUTE}] {
        display: grid; grid-template-columns: 1rem minmax(0, 1fr); align-items: center;
        gap: 0.5rem; width: 100%; padding: 0.5rem; border: 0; border-radius: 0.375rem;
        color: inherit; background: transparent; cursor: pointer; text-align: left; white-space: nowrap;
      }
      [${ITEM_ATTRIBUTE}]:hover, [${ITEM_ATTRIBUTE}]:focus-visible {
        outline: none; background: color-mix(in srgb, currentColor 10%, transparent);
      }
      [${ITEM_ATTRIBUTE}] span { overflow: hidden; text-overflow: ellipsis; }
    `;
    this.#document.head.append(style);
  }
}
