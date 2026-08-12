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
/** Own the Cube browser version combobox DOM and interaction state. */
import { $el } from '/scripts/ui.js';
/** Render and coordinate the browser's accessible version combobox. */
export class CubeBrowserVersionView {
    options;
    elements = null;
    open = false;
    highlightedIndex = -1;
    inputDirty = false;
    renderSignature = '';
    constructor(options) {
        this.options = options;
    }
    /** Build and bind the complete version-control subtree. */
    build() {
        const listboxId = 'sugarcubes-browser-version-listbox';
        const input = $el('input.sugarcubes-browser__version-input', {
            type: 'text',
            title: 'Spawn version',
            'aria-label': 'Spawn version',
            'aria-autocomplete': 'list',
            'aria-controls': listboxId,
            'aria-expanded': 'false',
            role: 'combobox',
            autocomplete: 'off',
            disabled: true,
        });
        const toggle = $el('button.sugarcubes-browser__version-toggle', {
            type: 'button',
            title: 'Show versions',
            'aria-label': 'Show versions',
            disabled: true,
        });
        const listbox = $el('ul.sugarcubes-browser__version-listbox', {
            id: listboxId,
            role: 'listbox',
            hidden: true,
        });
        const control = $el('div', { className: 'sugarcubes-browser__version-control sugarcubes-browser__action-hidden' }, [
            $el('span.sugarcubes-browser__version-prefix', 'v'),
            $el('div.sugarcubes-browser__version-input-shell', [input, toggle]),
            listbox,
        ]);
        this.elements = { control, input, toggle, listbox };
        input.addEventListener('focus', () => this.openCombobox());
        input.addEventListener('click', () => this.openCombobox());
        input.addEventListener('input', () => this.handleInput());
        input.addEventListener('keydown', (event) => this.handleKeydown(event));
        input.addEventListener('blur', () => {
            this.options.windowRef?.setTimeout?.(() => this.commit({ close: true }), 0);
        });
        toggle.addEventListener('mousedown', (event) => event.preventDefault());
        toggle.addEventListener('click', () => {
            if (input.disabled)
                return;
            const wasOpen = this.open;
            input.focus();
            if (wasOpen)
                this.closeCombobox();
            else
                this.openCombobox();
        });
        this.options.documentRef?.addEventListener?.('click', (event) => {
            const target = event.target instanceof Node ? event.target : null;
            if (!control.contains(target))
                this.commit({ close: true });
        });
        return this.elements;
    }
    /** Render current revision state into the combobox. */
    render(state, selected) {
        const elements = this.elements;
        if (!elements)
            return;
        const { control, input, toggle, listbox } = elements;
        if (!state) {
            this.reset();
            control.classList.add('sugarcubes-browser__action-hidden');
            return;
        }
        const options = state.versionOptions;
        const hide = !selected || state.editing || (!options.length && !state.revisionsLoading);
        control.classList.toggle('sugarcubes-browser__action-hidden', hide);
        control.classList.toggle('is-error', Boolean(state.versionError));
        control.classList.toggle('is-loading', Boolean(state.revisionsLoading));
        control.classList.toggle('is-single', options.length <= 1);
        if (hide) {
            this.reset();
            listbox.replaceChildren();
            listbox.hidden = true;
            input.value = '';
            input.setAttribute('aria-expanded', 'false');
            input.removeAttribute('aria-activedescendant');
            return;
        }
        const signature = `${state.selected || ''}|${options.map((item) => `${item.value}:${item.revisionRef}`).join('|')}`;
        if (this.renderSignature !== signature) {
            this.reset();
            this.renderSignature = signature;
        }
        const disabled = Boolean(state.busy || state.revisionsLoading || state.versionError);
        input.disabled = disabled;
        toggle.disabled = disabled;
        input.title = state.versionError || 'Spawn version';
        toggle.title = state.versionError || 'Show versions';
        if (!this.inputDirty)
            input.value = this.selectedValue();
        if (disabled || !options.length)
            this.open = false;
        if (this.open && this.highlightedIndex < 0)
            this.updateHighlight(input.value);
        const visible = this.visibleEntries(options, input.value);
        if (this.open &&
            visible.length &&
            !visible.some((entry) => entry.index === this.highlightedIndex)) {
            this.highlightedIndex = visible[0]?.index ?? -1;
        }
        listbox.replaceChildren(...visible.map(({ option, index }) => this.buildOption(option, index)));
        listbox.hidden = !this.open || disabled || !visible.length;
        input.setAttribute('aria-expanded', listbox.hidden ? 'false' : 'true');
        if (!listbox.hidden && this.highlightedIndex >= 0) {
            input.setAttribute('aria-activedescendant', `${listbox.id}-option-${this.highlightedIndex}`);
        }
        else {
            input.removeAttribute('aria-activedescendant');
        }
    }
    openCombobox() {
        const input = this.elements?.input;
        const options = this.options.getState()?.versionOptions || [];
        if (!input || input.disabled || !options.length)
            return;
        this.open = true;
        this.updateHighlight(input.value);
        this.render(this.options.getState(), this.options.getSelected());
    }
    closeCombobox() {
        this.open = false;
        const elements = this.elements;
        if (!elements)
            return;
        elements.input.setAttribute('aria-expanded', 'false');
        elements.input.removeAttribute('aria-activedescendant');
        elements.listbox.hidden = true;
    }
    reset() {
        this.open = false;
        this.highlightedIndex = -1;
        this.inputDirty = false;
    }
    handleInput() {
        const input = this.elements?.input;
        if (!input)
            return;
        this.inputDirty = true;
        this.open = true;
        this.updateHighlight(input.value);
        this.render(this.options.getState(), this.options.getSelected());
    }
    handleKeydown(event) {
        if (!this.options.getState()?.versionOptions.length)
            return;
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault();
            this.moveHighlight(event.key === 'ArrowDown' ? 1 : -1);
        }
        else if (event.key === 'Enter' || event.key === 'Tab') {
            if (event.key === 'Enter')
                event.preventDefault();
            this.commit({ close: true });
        }
        else if (event.key === 'Escape') {
            event.preventDefault();
            this.restoreInput();
            this.closeCombobox();
        }
    }
    updateHighlight(value) {
        const state = this.options.getState();
        const options = state?.versionOptions || [];
        const closest = this.options.getHandlers().onVersionClosest?.(value);
        const closestIndex = closest
            ? options.findIndex((item) => item.value === closest.value && item.revisionRef === closest.revisionRef)
            : -1;
        const selectedIndex = options.findIndex((item) => item.value === state?.selectedVersion);
        this.highlightedIndex =
            closestIndex >= 0
                ? closestIndex
                : selectedIndex >= 0
                    ? selectedIndex
                    : options.length
                        ? 0
                        : -1;
    }
    visibleEntries(options, value) {
        const entries = options.map((option, index) => ({ option, index }));
        const query = normalizeVersionQuery(value).toLowerCase();
        if (!this.inputDirty || !query)
            return entries;
        const matches = entries.filter(({ option }) => normalizeVersionQuery(option.value).toLowerCase().includes(query));
        if (matches.length)
            return matches;
        const closest = this.options.getHandlers().onVersionClosest?.(value);
        const index = closest
            ? options.findIndex((item) => item.value === closest.value && item.revisionRef === closest.revisionRef)
            : options.findIndex((item) => item.value === this.options.getState()?.selectedVersion);
        const entry = entries[index >= 0 ? index : 0];
        return entry ? [entry] : [];
    }
    moveHighlight(direction) {
        const options = this.options.getState()?.versionOptions || [];
        const visible = this.visibleEntries(options, this.elements?.input.value || '');
        if (!visible.length)
            return;
        const current = visible.findIndex((entry) => entry.index === this.highlightedIndex);
        const next = current >= 0
            ? (current + direction + visible.length) % visible.length
            : direction > 0
                ? 0
                : visible.length - 1;
        this.open = true;
        this.highlightedIndex = visible[next]?.index ?? -1;
        this.render(this.options.getState(), this.options.getSelected());
    }
    restoreInput() {
        if (!this.elements)
            return;
        this.elements.input.value = this.selectedValue();
        this.inputDirty = false;
        this.updateHighlight(this.elements.input.value);
    }
    commit({ close = false } = {}) {
        const state = this.options.getState();
        const input = this.elements?.input;
        const options = state?.versionOptions || [];
        if (!input || input.disabled || !options.length) {
            if (close)
                this.closeCombobox();
            return;
        }
        const highlighted = options[this.highlightedIndex] || null;
        const typed = this.inputDirty
            ? input.value
            : highlighted?.value || input.value || this.selectedValue();
        const committed = this.options.getHandlers().onVersionCommit?.(typed) || highlighted;
        input.value = committed?.value || this.selectedValue() || options[0]?.value || '';
        this.inputDirty = false;
        this.updateHighlight(input.value);
        if (close)
            this.closeCombobox();
        else
            this.render(state, this.options.getSelected());
    }
    selectedValue() {
        const state = this.options.getState();
        return (state?.versionOptions.find((option) => option.value === state.selectedVersion)?.value ||
            state?.versionOptions[0]?.value ||
            '');
    }
    buildOption(option, index) {
        const listbox = this.elements?.listbox;
        const state = this.options.getState();
        const child = $el('li', {
            id: `${listbox?.id || 'sugarcubes-browser-version-listbox'}-option-${index}`,
            className: 'sugarcubes-browser__version-option',
            role: 'option',
            'aria-selected': option.value === state?.selectedVersion ? 'true' : 'false',
            textContent: option.value,
        });
        child.dataset.version = option.value;
        child.classList.toggle('is-highlighted', index === this.highlightedIndex);
        child.addEventListener('mousedown', (event) => event.preventDefault());
        child.addEventListener('mouseover', () => {
            this.highlightedIndex = index;
            this.render(this.options.getState(), this.options.getSelected());
        });
        child.addEventListener('click', () => {
            if (!this.elements)
                return;
            this.elements.input.value = option.value;
            this.inputDirty = true;
            this.highlightedIndex = index;
            this.commit({ close: true });
            this.elements.input.focus();
        });
        return child;
    }
}
function normalizeVersionQuery(value) {
    return typeof value === 'string' ? value.trim().replace(/^v/i, '').trim() : '';
}
