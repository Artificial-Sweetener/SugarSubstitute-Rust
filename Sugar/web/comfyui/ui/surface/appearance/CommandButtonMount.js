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
/** Keep an owned command on the current visual provider without recreating its state or action. */
import { createButton } from './ControlFactory.js';
import { readOwnedVisualStyle, subscribeVisualStyle } from './VisualStyleBoundary.js';
/** Preserve the command lifetime while replacing only the selected provider's actual button asset. */
export class CommandButtonMount {
    document;
    label;
    action;
    element;
    #stop;
    #disabled = false;
    #style;
    /** Bind one static label and command through the shared control factory. */
    constructor(document, label, action) {
        this.document = document;
        this.label = label;
        this.action = action;
        this.#style = readOwnedVisualStyle(document);
        this.element = this.#create();
        this.#stop = subscribeVisualStyle(document, () => {
            const style = readOwnedVisualStyle(document);
            if (style === this.#style)
                return;
            this.#style = style;
            const previous = this.element, focused = previous.matches(':focus-within');
            this.element = this.#create();
            previous.replaceWith(this.element);
            if (focused)
                this.element.focus({ preventScroll: true });
        });
    }
    /** Apply one semantic disabled state to native and custom-element providers. */
    setDisabled(disabled) {
        this.#disabled = disabled;
        this.element.toggleAttribute('disabled', disabled);
    }
    /** Retire appearance observation with the owning command. */
    dispose() {
        this.#stop();
        this.element.remove();
    }
    /** Let the existing provider own paint, keyboard activation and focus. */
    #create() {
        const button = createButton(this.document);
        button.textContent = this.label;
        button.toggleAttribute('disabled', this.#disabled);
        button.addEventListener('click', () => {
            if (!this.#disabled)
                this.action();
        });
        return button;
    }
}
