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
/** Present the original orb through an official Fluent button and shared menu system. */
import { AppOrbRenderer } from './AppOrbRenderer.js';
import { ContextMenu } from '../../menus/ContextMenu.js';
/** Keep circular pointer interaction, keyboard access and menu lifetime with the owning button. */
export class AppOrbView {
    element;
    #renderer;
    #menu;
    /** Reuse existing application commands; artwork remains separate from menu policy. */
    constructor(document, entries) {
        this.element = document.createElement('fluent-button');
        this.element.className = 'sc-app-orb';
        this.element.setAttribute('appearance', 'transparent');
        this.element.setAttribute('aria-label', 'Application menu');
        this.element.setAttribute('aria-haspopup', 'menu');
        this.#renderer = new AppOrbRenderer(document);
        this.#menu = new ContextMenu(document);
        this.element.append(this.#renderer.canvas);
        this.element.addEventListener('pointerenter', () => this.#renderer.setState('hover'));
        this.element.addEventListener('pointerleave', () => this.#renderer.setState('idle'));
        this.element.addEventListener('pointerdown', () => this.#renderer.setState('pressed'));
        this.element.addEventListener('pointerup', () => this.#renderer.setState('hover'));
        this.element.addEventListener('click', () => {
            if (this.#menu.isOpen()) {
                this.#menu.close();
                return;
            }
            this.element.setAttribute('aria-expanded', 'true');
            this.#menu.open({
                label: 'Application menu',
                trigger: this.element,
                anchor: { kind: 'below', bounds: this.element.getBoundingClientRect() },
                entries: entries(),
                closed: () => this.element.setAttribute('aria-expanded', 'false'),
            });
        });
    }
    /** Prepare a palette change without subscribing to a second accent or theme owner. */
    update(color, dark) {
        return this.#renderer.update(color, dark);
    }
    /** Release decoded artwork and transient menu surfaces on mode exit. */
    dispose() {
        this.#menu.close();
        this.#renderer.dispose();
        this.element.remove();
    }
}
