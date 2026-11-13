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
/** Present a Cube version control matching Comfy's native color-picker affordance. */
const CONTROL_ATTRIBUTE = 'data-sugarcubes-version-control';
const BUTTON_ATTRIBUTE = 'data-sugarcubes-version-button';
/** Own only the color-control-shaped button while delegating choices to Comfy's native menu. */
export class ComfyCubeVersionToolboxPresenter {
    #document;
    #tooltip;
    #menu;
    #control = null;
    #button = null;
    #model = null;
    /** Bind focused host presentation collaborators. */
    constructor(options) {
        this.#document = options.document;
        this.#tooltip = options.tooltip;
        this.#menu = options.menu;
    }
    /** Place one control after the supplied host action and reconcile asynchronous state. */
    present(anchor, model) {
        this.#model = model;
        const control = this.#control ?? this.#createControl();
        const button = this.#button;
        if (!button)
            throw new Error('Cube version button is unavailable.');
        button.className = anchor.className;
        button.disabled = model.loading || model.busy || Boolean(model.error);
        button.setAttribute('aria-busy', String(model.loading || model.busy));
        button.setAttribute('aria-label', `Switch Cube version, current v${model.currentVersion}`);
        button.setAttribute('aria-expanded', String(this.#menu.isOpen()));
        if (control.previousElementSibling !== anchor)
            anchor.after(control);
        if (this.#menu.isOpen())
            this.#menu.refresh(this.#menuModel(model));
        return control;
    }
    /** Remove every owned version surface without mutating the host anchor. */
    clear() {
        this.#menu.close();
        this.#tooltip.clear();
        this.#control?.remove();
        this.#control = null;
        this.#button = null;
        this.#model = null;
    }
    /** Release owned DOM and native menu state. */
    dispose() {
        this.clear();
        this.#menu.dispose();
    }
    /** Create the exact wrapper and inner icon grouping used by Comfy's color control. */
    #createControl() {
        const control = this.#document.createElement('div');
        control.className = 'relative';
        control.setAttribute(CONTROL_ATTRIBUTE, '');
        const button = this.#document.createElement('button');
        button.type = 'button';
        button.setAttribute(BUTTON_ATTRIBUTE, '');
        button.setAttribute('aria-haspopup', 'menu');
        button.setAttribute('aria-expanded', 'false');
        const icons = this.#document.createElement('div');
        icons.className = 'flex items-center gap-1 px-0';
        const tags = this.#document.createElement('i');
        tags.className = 'pi pi-tags';
        tags.setAttribute('aria-hidden', 'true');
        const chevron = this.#document.createElement('i');
        const hostChevron = this.#document.querySelector('[data-testid="color-picker-button"] > div > i:last-child');
        chevron.className = hostChevron?.className || 'icon-[lucide--chevron-down]';
        if (hostChevron?.style.cssText)
            chevron.style.cssText = hostChevron.style.cssText;
        chevron.setAttribute('aria-hidden', 'true');
        icons.append(tags, chevron);
        button.append(icons);
        control.append(button);
        button.addEventListener('click', this.#toggleMenu);
        this.#tooltip.bind(button, 'Switch Cube version');
        this.#control = control;
        this.#button = button;
        return control;
    }
    /** Toggle Comfy's native combo menu using the currently loaded exact versions. */
    #toggleMenu = (event) => {
        event.preventDefault();
        event.stopPropagation();
        const model = this.#model;
        if (!model || model.loading || model.busy || model.error || model.options.length === 0)
            return;
        if (!this.#button)
            return;
        this.#menu.toggle(this.#button, this.#menuModel(model));
    };
    /** Project exact versions into an exclusive checked-row model. */
    #menuModel(model) {
        return {
            ariaLabel: 'Cube versions',
            selectionMode: 'single',
            closeOnSelect: true,
            items: () => model.options.map((option) => ({
                id: option.value,
                label: option.label,
                checked: option.value === model.currentVersion,
            })),
            select: (version) => {
                if (version !== model.currentVersion)
                    model.select(version);
            },
        };
    }
}
