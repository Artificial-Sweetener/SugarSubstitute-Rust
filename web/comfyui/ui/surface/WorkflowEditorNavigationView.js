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
/** Present editor ancestry through the selected control provider and literal host labels. */
import { createButton } from './appearance/ControlFactory.js';
import { subscribeVisualStyle } from './appearance/VisualStyleBoundary.js';
import { applyOwnedTheme } from './appearance/ThemeService.js';
/** Retain back controls and breadcrumb identity through unrelated native surface updates. */
export class WorkflowEditorNavigationView {
    document;
    element;
    #trail;
    #signature = '';
    #items = [];
    #dark = null;
    #stopStyle;
    /** Bind provider-owned actions to the host's existing navigation commands. */
    constructor(document, back) {
        this.document = document;
        this.element = document.createElement('nav');
        this.element.className = 'sc-editor-navigation';
        this.element.setAttribute('aria-label', 'Workflow editor navigation');
        this.element.setAttribute('data-sc-style-theme-root', '');
        const backButton = this.#back(back);
        backButton.classList.add('sc-editor-bar-back');
        this.#trail = document.createElement('ol');
        this.element.append(backButton, this.#trail);
        this.#stopStyle = subscribeVisualStyle(document, () => {
            const replacement = this.#back(back);
            replacement.classList.add('sc-editor-bar-back');
            this.element.querySelector('.sc-editor-bar-back')?.replaceWith(replacement);
            this.#signature = '';
            const dark = this.#dark ?? true;
            this.#dark = null;
            this.update(this.#items, dark);
        });
    }
    /** Reconcile only changed labels and delegate selection through the latest native path. */
    update(items, dark) {
        this.#items = items;
        if (this.#dark !== dark) {
            this.#dark = dark;
            applyOwnedTheme(this.element, dark);
        }
        const signature = JSON.stringify(items.map(({ label, current }) => [label, current]));
        if (signature === this.#signature)
            return;
        this.#signature = signature;
        this.#trail.replaceChildren(...items.map((item, index) => {
            const entry = this.document.createElement('li');
            const control = item.current
                ? this.document.createElement('span')
                : createButton(this.document);
            control.textContent = item.label;
            control.title = item.label;
            if (item.current)
                control.setAttribute('aria-current', 'page');
            else {
                control.setAttribute('appearance', 'subtle');
                control.addEventListener('click', () => this.#items[index]?.select());
            }
            entry.append(control);
            return entry;
        }));
    }
    /** Detach the trail and forget borrowed breadcrumb entries until the next nested path. */
    clear() {
        this.element.remove();
        this.#items = [];
        this.#signature = '';
        this.#trail.replaceChildren();
    }
    /** Release owned controls without touching native navigation state. */
    dispose() {
        this.#stopStyle();
        this.clear();
    }
    /** Keep the parent action alongside the breadcrumb trail with provider-owned keyboard and focus behavior. */
    #back(action) {
        const button = createButton(this.document);
        button.setAttribute('appearance', 'subtle');
        button.setAttribute('aria-label', 'Back to parent graph');
        button.title = 'Back to parent graph';
        const icon = this.document.createElement('span');
        icon.className = 'icon-[lucide--arrow-left]';
        icon.setAttribute('aria-hidden', 'true');
        button.append(icon);
        button.addEventListener('click', action);
        return button;
    }
}
