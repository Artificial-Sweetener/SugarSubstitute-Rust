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
/** Own every SugarCubes menu's Fluent rows, nested interaction and floating presentation. */
import { FloatingSurface } from './FloatingSurface.js';
import type { FloatingAnchor } from './FloatingSurface.js';
import { createFluentMenuList, createFluentMenuItem } from './FluentMenuRuntime.js';

export interface ContextMenuEntry {
  readonly label: string;
  readonly disabled?: boolean;
  readonly checked?: boolean;
  readonly selection?: 'single' | 'multiple';
  readonly separator?: boolean;
  readonly section?: boolean;
  readonly icon?: (document: Document) => HTMLElement;
  readonly children?: readonly ContextMenuEntry[];
  readonly attributes?: Readonly<Record<string, string>>;
  readonly select?: () => void;
  readonly keepOpen?: boolean;
}
export interface ContextMenuOptions {
  readonly label: string;
  readonly entries: readonly ContextMenuEntry[];
  readonly anchor: FloatingAnchor;
  readonly trigger?: HTMLElement | null;
  readonly attributes?: Readonly<Record<string, string>>;
  readonly closed?: () => void;
  readonly focusChecked?: boolean;
}
interface MenuLevel {
  readonly surface: FloatingSurface;
  readonly rows: HTMLElement[];
  readonly trigger: HTMLElement | null;
}

/** Own a complete transient menu tree; features supply values, icons and actions only. */
export class ContextMenu {
  static readonly #active = new WeakMap<Document, ContextMenu>();
  readonly #document: Document;
  #options: ContextMenuOptions | null = null;
  #levels: MenuLevel[] = [];
  #previous: HTMLElement | null = null;

  /** Bind menu interaction to one explicit browser document. */
  constructor(document: Document) {
    this.#document = document;
  }

  /** Open a fresh menu without inserting floating content inside the originating component. */
  open(options: ContextMenuOptions): void {
    ContextMenu.#active.get(this.#document)?.close();
    this.close();
    ContextMenu.#active.set(this.#document, this);
    this.#options = options;
    this.#previous =
      options.trigger ??
      (this.#document.activeElement instanceof HTMLElement ? this.#document.activeElement : null);
    this.#openLevel(options.entries, options.anchor, null, 0);
    this.#document.addEventListener('keydown', this.#key, true);
    if (options.focusChecked)
      this.#levels[0]?.rows.find((row) => row.getAttribute('aria-checked') === 'true')?.focus();
    if (!this.#levels[0]?.rows.some((row) => row === this.#document.activeElement))
      this.#levels[0]?.rows[0]?.focus();
  }

  /** Report the complete tree's lifetime rather than one submenu's state. */
  isOpen(): boolean {
    return this.#levels.length > 0;
  }

  /** Close every level, release listeners and restore the originating control's focus. */
  readonly close = (): void => {
    if (ContextMenu.#active.get(this.#document) === this)
      ContextMenu.#active.delete(this.#document);
    const options = this.#options;
    this.#options = null;
    this.#trim(0);
    this.#document.removeEventListener('keydown', this.#key, true);
    const previous = this.#previous;
    this.#previous = null;
    if (previous?.isConnected) previous.focus();
    options?.closed?.();
  };

  /** Render official Fluent controls with literal text and feature-owned icon slots. */
  #openLevel(
    entries: readonly ContextMenuEntry[],
    anchor: FloatingAnchor,
    trigger: HTMLElement | null,
    depth: number,
  ): void {
    this.#trim(depth);
    const menu = createFluentMenuList(this.#document);
    menu.className = 'sc-context-menu';
    menu.setAttribute('role', 'menu');
    menu.setAttribute('aria-label', this.#options?.label ?? 'Actions');
    menu.style.setProperty('width', 'max-content');
    menu.style.setProperty('font-family', 'var(--fontFamilyBase)');
    menu.style.setProperty('max-inline-size', 'calc(100vw - 8px)');
    menu.style.setProperty('overflow', 'auto');
    menu.style.setProperty('scrollbar-width', 'none');
    if (depth === 0)
      for (const [name, value] of Object.entries(this.#options?.attributes ?? {}))
        menu.setAttribute(name, value);
    const rows = this.#renderRows(menu, entries, depth);
    const surface = new FloatingSurface(menu);
    this.#levels.push({ surface, rows, trigger });
    surface.open({
      anchor,
      dismiss: this.close,
      containsTarget: (target) =>
        this.#levels.some((level) => level.surface.element.contains(target)) ||
        (this.#options?.trigger?.contains(target) ?? false),
    });
  }

  /** Update checked choices without replacing the floating surface or losing its scroll position. */
  updateEntries(entries: readonly ContextMenuEntry[]): void {
    const level = this.#levels[0];
    if (!level) return;
    const active = this.#document.activeElement;
    const index = active instanceof HTMLElement ? level.rows.indexOf(active) : -1;
    this.#trim(1);
    level.surface.element.replaceChildren();
    level.rows.splice(0, level.rows.length, ...this.#renderRows(level.surface.element, entries, 0));
    level.rows[Math.max(0, index)]?.focus();
    level.surface.reposition();
  }

  /** Build rows for one level from the current feature-owned choices. */
  #renderRows(
    menu: HTMLElement,
    entries: readonly ContextMenuEntry[],
    depth: number,
  ): HTMLElement[] {
    const rows: HTMLElement[] = [];
    for (const entry of entries) {
      if (entry.section) {
        const heading = this.#document.createElement('div');
        heading.dataset.scMenuSection = '';
        heading.setAttribute('role', 'presentation');
        heading.textContent = entry.label;
        heading.style.cssText =
          'padding:6px 12px;font-size:12px;color:var(--colorNeutralForeground3);';
        menu.append(heading);
        continue;
      }
      if (entry.separator) {
        const separator = this.#document.createElement('div');
        separator.setAttribute('role', 'separator');
        separator.style.cssText =
          'height:1px;margin:4px 8px;background:var(--colorNeutralStroke2);';
        menu.append(separator);
        continue;
      }
      const row = createFluentMenuItem(this.#document);
      row.setAttribute(
        'role',
        entry.checked === undefined
          ? 'menuitem'
          : entry.selection === 'multiple'
            ? 'menuitemcheckbox'
            : 'menuitemradio',
      );
      row.setAttribute('tabindex', '-1');
      row.setAttribute('aria-disabled', String(entry.disabled ?? false));
      if (entry.disabled) row.setAttribute('disabled', '');
      if (entry.checked !== undefined) {
        row.setAttribute('aria-checked', String(entry.checked));
        if (entry.checked) row.setAttribute('checked', '');
      }
      for (const [name, value] of Object.entries(entry.attributes ?? {}))
        row.setAttribute(name, value);
      if (entry.icon) {
        const icon = entry.icon(this.#document);
        icon.slot = 'start';
        icon.setAttribute('aria-hidden', 'true');
        icon.style.width = '16px';
        icon.style.height = '16px';
        menu.style.setProperty('--_col-start-width', '24px');
        row.append(icon);
      }
      const label = this.#document.createElement('span');
      label.textContent = entry.label;
      label.style.cssText = 'white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
      row.append(label);
      if (entry.children?.length) {
        row.setAttribute('aria-haspopup', 'menu');
        row.setAttribute('aria-expanded', 'false');
        const arrow = this.#document.createElement('span');
        arrow.slot = 'end';
        arrow.setAttribute('aria-hidden', 'true');
        arrow.style.cssText =
          'width:6px;height:6px;border:solid currentColor;border-width:1px 1px 0 0;transform:rotate(45deg);';
        row.append(arrow);
      }
      /** Activate feature policy once while suppressing the component's independent submenu lifecycle. */
      const activate = (): void => {
        if (entry.disabled) return;
        if (entry.children?.length) {
          this.#openLevel(
            entry.children,
            { kind: 'submenu', bounds: row.getBoundingClientRect() },
            row,
            depth + 1,
          );
          row.setAttribute('aria-expanded', 'true');
          this.#levels[depth + 1]?.rows[0]?.focus();
        } else {
          if (!entry.keepOpen) this.close();
          entry.select?.();
        }
      };
      row.addEventListener(
        'click',
        (event) => {
          event.preventDefault();
          event.stopImmediatePropagation();
          activate();
        },
        true,
      );
      row.addEventListener('pointerenter', () => {
        if (entry.disabled) return;
        if (entry.children?.length && this.#levels[depth + 1]?.trigger !== row) {
          this.#openLevel(
            entry.children,
            { kind: 'submenu', bounds: row.getBoundingClientRect() },
            row,
            depth + 1,
          );
          row.setAttribute('aria-expanded', 'true');
        } else if (!entry.children?.length) this.#trim(depth + 1);
      });
      if (!entry.disabled) rows.push(row);
      menu.append(row);
    }
    return rows;
  }

  /** Remove descendant panels without disturbing the parent menu's focus or actions. */
  #trim(depth: number): void {
    for (const level of this.#levels.splice(depth).reverse()) {
      level.surface.close();
      level.trigger?.setAttribute('aria-expanded', 'false');
    }
  }

  /** Keep all keyboard policy in one owner for both DOM and Vue consumers. */
  readonly #key = (event: KeyboardEvent): void => {
    if (
      ![
        'Escape',
        'Tab',
        'ArrowLeft',
        'ArrowRight',
        'ArrowUp',
        'ArrowDown',
        'Home',
        'End',
        'Enter',
        ' ',
      ].includes(event.key)
    )
      return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (event.key === 'Tab') {
      this.close();
      return;
    }
    const active = this.#document.activeElement;
    const depth = this.#levels.findIndex((level) =>
      level.rows.includes(active instanceof HTMLElement ? active : this.#document.body),
    );
    const index = Math.max(0, depth);
    const level = this.#levels[index];
    if (!level) return;
    if (event.key === 'Escape' || event.key === 'ArrowLeft') {
      if (index > 0) {
        const trigger = level.trigger;
        this.#trim(index);
        trigger?.focus();
      } else this.close();
      return;
    }
    const current = level.rows.indexOf(
      active instanceof HTMLElement ? active : this.#document.body,
    );
    if (event.key === 'ArrowRight') {
      if (level.rows[current]?.getAttribute('aria-haspopup') === 'menu')
        level.rows[current]?.click();
      return;
    }
    if (event.key === 'Enter' || event.key === ' ') {
      level.rows[current]?.click();
      return;
    }
    this.#trim(index + 1);
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? level.rows.length - 1
          : (current + (event.key === 'ArrowUp' ? -1 : 1) + level.rows.length) % level.rows.length;
    level.rows[next]?.focus();
  };
}
