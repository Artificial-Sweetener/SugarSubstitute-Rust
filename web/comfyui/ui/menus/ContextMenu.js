var _a;
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
/** Own every SugarCubes menu's themed rows, nested interaction and floating presentation. */
import { createContextMenuField } from './ContextMenuField.js';
import { FloatingSurface } from './FloatingSurface.js';
import { createMenuList, createMenuItem, setMenuItemChecked, menuItemSlot, } from './MenuControlRuntime.js';
import { subscribeVisualStyle } from '../surface/appearance/VisualStyleBoundary.js';
import { identifyMenuSurface } from './MenuSurface.js';
/** Own a complete transient menu tree; features supply values, icons and actions only. */
export class ContextMenu {
    static #active = new WeakMap();
    #document;
    #options = null;
    #levels = [];
    #previous = null;
    #stopStyle = null;
    /** Bind menu interaction to one explicit browser document. */
    constructor(document) {
        this.#document = document;
    }
    /** Open a fresh menu without inserting floating content inside the originating component. */
    open(options) {
        _a.#active.get(this.#document)?.close();
        this.close();
        _a.#active.set(this.#document, this);
        this.#options = options;
        this.#stopStyle = subscribeVisualStyle(this.#document, this.close);
        this.#previous =
            options.focusReturn !== undefined
                ? options.focusReturn
                : (options.trigger ??
                    (this.#document.activeElement instanceof HTMLElement
                        ? this.#document.activeElement
                        : null));
        this.#openLevel(options.entries, options.anchor, null, 0);
        this.#document.addEventListener('keydown', this.#key, true);
        if (options.focusChecked)
            this.#levels[0]?.rows
                .find((row) => row.getAttribute('aria-checked') === 'true' &&
                row.getAttribute('aria-disabled') !== 'true')
                ?.focus();
        if (!this.#levels[0]?.rows.some((row) => row === this.#document.activeElement))
            (this.#levels[0]?.rows.find((row) => row.getAttribute('aria-disabled') !== 'true' &&
                !(row instanceof HTMLInputElement && row.disabled)) ?? this.#levels[0]?.surface.element)?.focus();
    }
    /** Report the complete tree's lifetime rather than one submenu's state. */
    isOpen() {
        return this.#levels.length > 0;
    }
    /** Close every level, release listeners and restore the originating control's focus. */
    close = () => this.#close(true);
    /** Keep dismissal and command handoff under the same focus and exclusivity owner. */
    #close(returnFocus) {
        if (_a.#active.get(this.#document) === this)
            _a.#active.delete(this.#document);
        const options = this.#options;
        this.#options = null;
        this.#stopStyle?.();
        this.#stopStyle = null;
        this.#trim(0);
        this.#document.removeEventListener('keydown', this.#key, true);
        const previous = this.#previous;
        this.#previous = null;
        if (returnFocus && previous?.isConnected)
            previous.focus();
        options?.closed?.();
    }
    /** Render official controls with literal text and feature-owned icon slots. */
    #openLevel(entries, anchor, trigger, depth, opened) {
        this.#trim(depth);
        const menu = createMenuList(this.#document);
        menu.className = 'sc-context-menu';
        identifyMenuSurface(menu, 'actions');
        menu.setAttribute('role', 'menu');
        menu.setAttribute('tabindex', '-1');
        menu.setAttribute('aria-label', this.#options?.label ?? 'Actions');
        menu.style.setProperty('width', 'max-content');
        menu.style.setProperty('font-family', 'var(--sc-font-family)');
        menu.style.setProperty('max-inline-size', 'calc(100vw - 8px)');
        menu.style.setProperty('overflow', 'auto');
        menu.style.setProperty('scrollbar-width', 'none');
        if (depth === 0)
            for (const [name, value] of Object.entries(this.#options?.attributes ?? {}))
                menu.setAttribute(name, value);
        const updates = [];
        const refresh = () => {
            for (const update of updates)
                update();
        };
        const rows = this.#renderRows(menu, entries, depth, updates, refresh);
        const surface = new FloatingSurface(menu);
        const level = { surface, rows, trigger, refresh, updates };
        this.#levels.push(level);
        surface.open({
            anchor,
            reserveRight: depth === 0 && this.#options?.preferRightSubmenus === true,
            dismiss: this.close,
            containsTarget: (target) => this.#levels.some((level) => level.surface.element.contains(target)) ||
                (this.#options?.anchor.kind === 'below' &&
                    (this.#options.trigger?.contains(target) ?? false)),
        });
        if (opened)
            level.release = opened(() => {
                for (const current of this.#levels)
                    current.refresh();
            }, this.close);
    }
    /** Update checked choices without replacing the floating surface or losing its scroll position. */
    updateEntries(entries) {
        const level = this.#levels[0];
        if (!level)
            return;
        const active = this.#document.activeElement;
        const index = active instanceof HTMLElement ? level.rows.indexOf(active) : -1;
        this.#trim(1);
        level.surface.element.replaceChildren();
        level.updates.length = 0;
        level.rows.splice(0, level.rows.length, ...this.#renderRows(level.surface.element, entries, 0, level.updates, level.refresh));
        (level.rows[Math.max(0, index)] ?? level.surface.element).focus();
        level.surface.reposition();
    }
    /** Build rows for one level from the current feature-owned choices. */
    #renderRows(menu, entries, depth, updates, refresh) {
        const rows = [];
        for (const entry of entries) {
            if (entry.input) {
                const field = createContextMenuField(this.#document, {
                    ...entry,
                    input: entry.input,
                    get label() {
                        return entry.label;
                    },
                    get disabled() {
                        return entry.disabled ?? false;
                    },
                }, refresh);
                updates.push(field.refresh);
                rows.push(field.input);
                menu.append(field.root);
                continue;
            }
            if (entry.section) {
                const heading = this.#document.createElement('div');
                heading.dataset.scMenuSection = '';
                heading.setAttribute('role', 'presentation');
                heading.textContent = entry.label;
                heading.style.cssText = 'padding:6px 12px;font-size:12px;color:var(--sc-text-muted);';
                menu.append(heading);
                continue;
            }
            if (entry.separator) {
                const separator = this.#document.createElement('div');
                separator.setAttribute('role', 'separator');
                separator.style.cssText = 'height:1px;margin:4px 8px;background:var(--sc-border-subtle);';
                menu.append(separator);
                continue;
            }
            const row = createMenuItem(this.#document);
            row.setAttribute('role', entry.checked === undefined
                ? 'menuitem'
                : entry.selection === 'multiple'
                    ? 'menuitemcheckbox'
                    : 'menuitemradio');
            row.setAttribute('tabindex', '-1');
            row.setAttribute('aria-disabled', String(entry.disabled ?? false));
            if (entry.disabled)
                row.setAttribute('disabled', '');
            if (entry.checked !== undefined) {
                setMenuItemChecked(row, entry.checked);
            }
            for (const [name, value] of Object.entries(entry.attributes ?? {}))
                row.setAttribute(name, value);
            if (entry.icon) {
                const icon = entry.icon(this.#document);
                icon.slot = menuItemSlot(this.#document, 'start');
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
            const children = () => entry.children ?? [];
            if (entry.children) {
                row.setAttribute('aria-haspopup', 'menu');
                row.setAttribute('aria-expanded', 'false');
                const arrow = this.#document.createElement('span');
                arrow.slot = menuItemSlot(this.#document, 'end');
                arrow.setAttribute('aria-hidden', 'true');
                arrow.style.cssText =
                    'width:6px;height:6px;border:solid currentColor;border-width:1px 1px 0 0;transform:rotate(45deg);';
                row.append(arrow);
            }
            /** Activate feature policy once while suppressing the component's independent submenu lifecycle. */
            const activate = () => {
                if (entry.disabled)
                    return;
                if (entry.children) {
                    if (this.#levels[depth + 1]?.trigger !== row)
                        this.#openLevel(children(), { kind: entry.submenuPlacement ?? 'submenu', bounds: row.getBoundingClientRect() }, row, depth + 1, entry.opened);
                    row.setAttribute('aria-expanded', 'true');
                    (this.#levels[depth + 1]?.rows.find((item) => item.getAttribute('aria-disabled') !== 'true' &&
                        !(item instanceof HTMLInputElement && item.disabled)) ?? this.#levels[depth + 1]?.surface.element)?.focus();
                }
                else {
                    if (entry.keepOpen === 'parent' && depth > 0) {
                        const parent = this.#levels[depth]?.trigger;
                        this.#trim(depth);
                        parent?.focus();
                    }
                    else if (!entry.keepOpen || entry.keepOpen === 'parent')
                        this.#close(this.#options?.returnFocusOnSelect !== false);
                    entry.select?.();
                }
            };
            row.addEventListener('click', (event) => {
                event.preventDefault();
                event.stopImmediatePropagation();
                activate();
            }, true);
            row.addEventListener('pointerenter', () => {
                if (entry.disabled)
                    return;
                if (entry.children && this.#levels[depth + 1]?.trigger !== row) {
                    this.#openLevel(children(), { kind: entry.submenuPlacement ?? 'submenu', bounds: row.getBoundingClientRect() }, row, depth + 1, entry.opened);
                    row.setAttribute('aria-expanded', 'true');
                }
                else if (!entry.children)
                    this.#trim(depth + 1);
            });
            const update = () => {
                label.textContent = entry.label;
                row.setAttribute('aria-disabled', String(entry.disabled ?? false));
                if (entry.disabled)
                    row.setAttribute('disabled', '');
                else
                    row.removeAttribute('disabled');
                if (entry.checked !== undefined)
                    setMenuItemChecked(row, entry.checked);
            };
            updates.push(update);
            rows.push(row);
            menu.append(row);
        }
        return rows;
    }
    /** Remove descendant panels without disturbing the parent menu's focus or actions. */
    #trim(depth) {
        for (const level of this.#levels.splice(depth).reverse()) {
            level.release?.();
            level.surface.close();
            level.trigger?.setAttribute('aria-expanded', 'false');
        }
    }
    /** Keep all keyboard policy in one owner for both DOM and Vue consumers. */
    #key = (event) => {
        if (![
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
        ].includes(event.key))
            return;
        if (this.#document.activeElement instanceof HTMLInputElement &&
            !['Escape', 'Tab'].includes(event.key))
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        if (event.key === 'Tab') {
            this.close();
            return;
        }
        const active = this.#document.activeElement;
        const depth = this.#levels.findIndex((level) => level.surface.element === active ||
            level.rows.includes(active instanceof HTMLElement ? active : this.#document.body));
        const index = Math.max(0, depth);
        const level = this.#levels[index];
        if (!level)
            return;
        if (event.key === 'Escape' || event.key === 'ArrowLeft') {
            if (index > 0) {
                const trigger = level.trigger;
                this.#trim(index);
                trigger?.focus();
            }
            else
                this.close();
            return;
        }
        const rows = level.rows.filter((row) => row.getAttribute('aria-disabled') !== 'true' &&
            !(row instanceof HTMLInputElement && row.disabled));
        const current = rows.indexOf(active instanceof HTMLElement ? active : this.#document.body);
        if (event.key === 'ArrowRight') {
            if (rows[current]?.getAttribute('aria-haspopup') === 'menu')
                rows[current]?.click();
            return;
        }
        if (event.key === 'Enter' || event.key === ' ') {
            rows[current]?.click();
            return;
        }
        this.#trim(index + 1);
        const next = event.key === 'Home'
            ? 0
            : event.key === 'End'
                ? rows.length - 1
                : (current + (event.key === 'ArrowUp' ? -1 : 1) + rows.length) % rows.length;
        rows[next]?.focus();
    };
}
_a = ContextMenu;
