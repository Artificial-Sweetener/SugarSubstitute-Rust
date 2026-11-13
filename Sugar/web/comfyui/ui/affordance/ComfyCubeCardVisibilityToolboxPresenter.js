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
/** Present Cube optional-card visibility through the shared toolbox check menu. */
const BUTTON_ATTRIBUTE = 'data-sugarcubes-card-visibility-button';
/** Own the eye action and map optional-card state into the shared multiple-selection menu. */
export class ComfyCubeCardVisibilityToolboxPresenter {
    #document;
    #owner;
    #tooltip;
    #menu;
    #button = null;
    #node = null;
    /** Bind the focused host presentation to renderer-neutral reveal state. */
    constructor(options) {
        this.#document = options.document;
        this.#owner = options.owner;
        this.#tooltip = options.tooltip;
        this.#menu = options.menu;
    }
    /** Place one eye action after its anchor when the Cube has optional cards. */
    present(anchor, node) {
        if (this.#owner.list(node).length === 0) {
            this.clear();
            return;
        }
        if (this.#node && this.#node !== node)
            this.#menu.close();
        this.#node = node;
        const button = this.#button ?? this.#createButton();
        const classSource = anchor instanceof HTMLButtonElement
            ? anchor
            : anchor.querySelector('button');
        button.className = classSource?.className ?? '';
        button.setAttribute('aria-expanded', String(this.#menu.isOpen()));
        if (button.previousElementSibling !== anchor)
            anchor.after(button);
        if (this.#menu.isOpen())
            this.#menu.refresh(this.#menuModel(node));
    }
    /** Remove the eye action and close its transient menu. */
    clear() {
        this.#menu.close();
        this.#tooltip.clear();
        this.#button?.remove();
        this.#button = null;
        this.#node = null;
    }
    /** Release every owned DOM and menu collaborator. */
    dispose() {
        this.clear();
        this.#menu.dispose();
    }
    /** Create the eye action once so mutation-driven refreshes remain idempotent. */
    #createButton() {
        const button = this.#document.createElement('button');
        button.type = 'button';
        button.setAttribute(BUTTON_ATTRIBUTE, '');
        button.setAttribute('aria-label', 'Manage optional nodes');
        button.setAttribute('aria-haspopup', 'menu');
        button.setAttribute('aria-expanded', 'false');
        const icon = this.#document.createElement('i');
        icon.className = 'pi pi-eye';
        icon.setAttribute('aria-hidden', 'true');
        button.append(icon);
        button.addEventListener('click', this.#toggleMenu);
        this.#tooltip.bind(button, 'Manage optional nodes');
        this.#button = button;
        return button;
    }
    /** Toggle the shared menu against the current Cube. */
    #toggleMenu = (event) => {
        event.preventDefault();
        event.stopPropagation();
        if (!this.#button || !this.#node)
            return;
        this.#menu.toggle(this.#button, this.#menuModel(this.#node));
    };
    /** Project live optional-card state into a multiple-selection checked menu. */
    #menuModel(node) {
        return {
            ariaLabel: 'Manage optional nodes',
            selectionMode: 'multiple',
            closeOnSelect: false,
            items: () => this.#owner
                .list(node)
                .map((entry) => ({ id: entry.id, label: entry.label, checked: entry.revealed })),
            select: (id) => {
                const entry = this.#owner.list(node).find((candidate) => candidate.id === id);
                if (entry)
                    this.#owner.setRevealed(node, id, !entry.revealed);
            },
        };
    }
}
