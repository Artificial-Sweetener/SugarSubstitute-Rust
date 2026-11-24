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
import { ensureContextMenuStyles } from './ContextMenuStyles.js';
import { createContextMenuField } from './ContextMenuField.js';
import { FloatingSurface, sameFloatingAnchor } from './FloatingSurface.js';
import { createMenuList, createMenuItem, menuItemSlot } from './MenuControlRuntime.js';
import { setMenuItemChecked } from './MenuItemChecked.js';
import { subscribeVisualStyle } from '../surface/appearance/VisualStyleBoundary.js';
import { identifyMenuSurface } from './MenuSurface.js';
/**
 * How one entry's row is built. Two entry lists built alike keep their rows: each row's label,
 * disabled state and check are updated in place and only the actions behind them change. An
 * entry with an inline field always rebuilds.
 */
function presentation(entry) {
    return JSON.stringify([
        entry.separator ?? false,
        entry.section ? entry.label : false,
        entry.input ? true : false,
        entry.checked === undefined ? 'item' : (entry.selection ?? 'single'),
        entry.children ? true : false,
        entry.icon ? true : false,
        entry.attributes ?? null,
    ]);
}
/** The first row a keyboard or pointer user can act on. */
function firstEnabledRow(level) {
    return level.rows.find((row) => row.getAttribute('aria-disabled') !== 'true' &&
        !(row instanceof HTMLInputElement && row.disabled));
}
/**
 * Own a complete transient menu tree; features supply values, icons and actions only.
 *
 * Whether an open menu needs new rows is decided here and nowhere else: features hand the menu
 * their current entries as often as their state changes, by `open` at the same spot or by
 * `updateEntries`, and rows that would come out the same are kept.
 */
export class ContextMenu {
    static #active = new WeakMap();
    #document;
    #rowBindings = new WeakMap();
    #options = null;
    #levels = [];
    #previous = null;
    #stopStyle = null;
    /** Bind menu interaction to one explicit browser document. */
    constructor(document) {
        this.#document = document;
    }
    /**
     * Open a fresh menu without inserting floating content inside the originating component.
     *
     * A menu anchored below its trigger is that trigger's toggle: asked to open again from the
     * same trigger while it is open, it closes instead. The rule lives here so that no caller can
     * forget it and leave a button that only ever opens.
     *
     * Opened again at the same spot while it is open, the menu takes the new entries as a refresh
     * rather than a fresh menu, so a feature that re-renders it keeps its hover, focus and any press
     * in flight.
     */
    open(options) {
        const current = this.#options;
        if (current &&
            this.isOpen() &&
            options.anchor.kind === 'below' &&
            current.anchor.kind === 'below' &&
            options.trigger != null &&
            current.trigger === options.trigger) {
            this.close();
            return;
        }
        if (current &&
            this.isOpen() &&
            _a.#active.get(this.#document) === this &&
            current.label === options.label &&
            sameFloatingAnchor(current.anchor, options.anchor)) {
            const root = this.#levels[0]?.surface.element;
            for (const name of Object.keys(current.attributes ?? {}))
                if (!(name in (options.attributes ?? {})))
                    root?.removeAttribute(name);
            for (const [name, value] of Object.entries(options.attributes ?? {}))
                if (root?.getAttribute(name) !== value)
                    root?.setAttribute(name, value);
            this.#options = options;
            this.updateEntries(options.entries);
            return;
        }
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
        const root = this.#levels[0];
        if (root && !root.rows.some((row) => row === this.#document.activeElement))
            (firstEnabledRow(root) ?? root.surface.element).focus();
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
        ensureContextMenuStyles(this.#document);
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
        const bindings = [];
        const rows = this.#renderRows(menu, entries, depth, updates, refresh, bindings);
        const surface = new FloatingSurface(menu);
        const level = {
            surface,
            rows,
            trigger,
            refresh,
            updates,
            presented: entries.map(presentation),
            bindings,
        };
        this.#levels.push(level);
        surface.open({
            anchor,
            anchorElement: depth === 0 ? (this.#options?.trigger ?? null) : trigger,
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
    /**
     * Update checked choices without replacing the floating surface or losing its scroll position.
     *
     * Entries that present the same rows as the open menu keep those rows, so the row under the
     * pointer keeps its hover and a press and its release land on the same row: features may refresh
     * an open menu as often as their state changes. An open submenu takes its trigger's new
     * children the same way, and closes when they no longer build its rows. Changed root rows
     * rebuild the menu.
     */
    updateEntries(entries) {
        const level = this.#levels[0];
        if (!level)
            return;
        if (this.#keepRows(level, entries)) {
            for (let depth = 1; depth < this.#levels.length; depth += 1) {
                const submenu = this.#levels[depth];
                const children = submenu?.trigger
                    ? this.#rowBindings.get(submenu.trigger)?.entry.children
                    : undefined;
                if (!submenu || !children || !this.#keepRows(submenu, children)) {
                    const focused = this.#document.activeElement;
                    const focusInside = this.#levels
                        .slice(depth)
                        .some((current) => focused instanceof Node && current.surface.element.contains(focused));
                    this.#trim(depth);
                    if (focusInside)
                        submenu?.trigger?.focus();
                    break;
                }
            }
            // A row that has focus keeps it; a menu still focused as a whole hands it to its first row.
            const focused = this.#document.activeElement;
            const whole = this.#levels.find((current) => current.surface.element === focused);
            if (whole)
                firstEnabledRow(whole)?.focus();
            return;
        }
        const active = this.#document.activeElement;
        const index = active instanceof HTMLElement ? level.rows.indexOf(active) : -1;
        this.#trim(1);
        level.surface.element.replaceChildren();
        level.updates.length = 0;
        level.bindings.length = 0;
        level.presented.splice(0, level.presented.length, ...entries.map(presentation));
        level.rows.splice(0, level.rows.length, ...this.#renderRows(level.surface.element, entries, 0, level.updates, level.refresh, level.bindings));
        (level.rows[Math.max(0, index)] ?? level.surface.element).focus();
        level.surface.reposition();
    }
    /** Rebind one level's rows to entries that build them unchanged; report whether they did. */
    #keepRows(level, entries) {
        const presented = entries.map(presentation);
        if (presented.length !== level.presented.length ||
            presented.some((value, index) => value !== level.presented[index]) ||
            entries.some((entry) => entry.input))
            return false;
        entries.forEach((entry, index) => {
            const binding = level.bindings[index];
            if (binding)
                binding.entry = entry;
        });
        level.refresh();
        return true;
    }
    /** Build rows for one level from the current feature-owned choices. */
    #renderRows(menu, entries, depth, updates, refresh, bindings) {
        const rows = [];
        for (const initial of entries) {
            let entry = initial;
            if (entry.input || entry.section || entry.separator)
                bindings.push(null);
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
            const binding = { entry };
            bindings.push(binding);
            const row = createMenuItem(this.#document);
            this.#rowBindings.set(row, binding);
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
            const children = () => binding.entry.children ?? [];
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
                entry = binding.entry;
                if (entry.disabled)
                    return;
                if (entry.children) {
                    if (this.#levels[depth + 1]?.trigger !== row)
                        this.#openLevel(children(), { kind: entry.submenuPlacement ?? 'submenu', bounds: row.getBoundingClientRect() }, row, depth + 1, entry.opened);
                    row.setAttribute('aria-expanded', 'true');
                    const submenu = this.#levels[depth + 1];
                    if (submenu)
                        (firstEnabledRow(submenu) ?? submenu.surface.element).focus();
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
                entry = binding.entry;
                if (entry.disabled)
                    return;
                if (entry.children && this.#levels[depth + 1]?.trigger !== row) {
                    this.#openLevel(children(), { kind: entry.submenuPlacement ?? 'submenu', bounds: row.getBoundingClientRect() }, row, depth + 1, entry.opened);
                    row.setAttribute('aria-expanded', 'true');
                }
                else if (!entry.children)
                    this.#trim(depth + 1);
            });
            // Refreshes may arrive every frame: write only what changed, so observers see no churn.
            const update = () => {
                entry = binding.entry;
                if (label.textContent !== entry.label)
                    label.textContent = entry.label;
                const disabled = entry.disabled ?? false;
                if (row.getAttribute('aria-disabled') !== String(disabled))
                    row.setAttribute('aria-disabled', String(disabled));
                row.toggleAttribute('disabled', disabled);
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
