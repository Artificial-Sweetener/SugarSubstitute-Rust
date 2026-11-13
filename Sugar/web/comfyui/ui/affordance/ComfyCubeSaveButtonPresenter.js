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
/** Present Comfy's native Subgraph publish button as one reversible Cube save action. */
const COMPOSITE_ICON_ATTRIBUTE = 'data-sugarcubes-save-cube-icon';
const SAVE_BADGE_ATTRIBUTE = 'data-sugarcubes-save-cube-badge';
const STYLE_ID = 'sugarcubes-save-cube-button-styles';
const CUBE_ICON_CLASS = 'icon-[lucide--box]';
const SAVE_ICON_CLASS = 'icon-[lucide--save]';
/** Own the accessible label and native-icon composition for the Cube save button. */
export class ComfyCubeSaveButtonPresenter {
    #document;
    #tooltips;
    /** Bind DOM construction and PrimeVue presentation collaborators. */
    constructor(options) {
        this.#document = options.document;
        this.#tooltips = options.tooltips;
        this.#ensureStyles();
    }
    /** Present Cube semantics and return the exact state required for restoration. */
    present(button, sourceIcon, label, state) {
        const presentation = state ?? this.#capture(button, sourceIcon);
        this.#tooltips.present(button, label);
        presentation.sourceIcon.hidden = true;
        presentation.sourceIcon.style.setProperty('display', 'none', 'important');
        if (presentation.compositeIcon.parentElement !== button) {
            button.append(presentation.compositeIcon);
        }
        return presentation;
    }
    /** Restore Comfy's original icon and tooltip presentation exactly. */
    restore(button, state) {
        this.#tooltips.restore(button, state.tooltip);
        state.sourceIcon.hidden = state.sourceIconHidden;
        if (state.sourceIconDisplay) {
            state.sourceIcon.style.setProperty('display', state.sourceIconDisplay, state.sourceIconDisplayPriority);
        }
        else {
            state.sourceIcon.style.removeProperty('display');
        }
        state.compositeIcon.remove();
    }
    /** Capture host state before creating SugarCubes-owned presentation DOM. */
    #capture(button, sourceIcon) {
        return {
            tooltip: this.#tooltips.capture(button),
            sourceIcon,
            sourceIconHidden: sourceIcon.hidden,
            sourceIconDisplay: sourceIcon.style.getPropertyValue('display'),
            sourceIconDisplayPriority: sourceIcon.style.getPropertyPriority('display'),
            compositeIcon: this.#createCompositeIcon(),
        };
    }
    /** Compose two icon masks already shipped by Comfy's native Lucide library. */
    #createCompositeIcon() {
        const root = this.#document.createElement('span');
        root.setAttribute(COMPOSITE_ICON_ATTRIBUTE, '');
        root.setAttribute('aria-hidden', 'true');
        root.style.position = 'relative';
        root.style.display = 'inline-block';
        root.style.width = '1rem';
        root.style.height = '1rem';
        root.style.flex = '0 0 1rem';
        const cube = this.#document.createElement('i');
        cube.className = CUBE_ICON_CLASS;
        cube.style.position = 'absolute';
        cube.style.inset = '0';
        cube.style.width = '100%';
        cube.style.height = '100%';
        const badge = this.#document.createElement('span');
        badge.setAttribute(SAVE_BADGE_ATTRIBUTE, '');
        badge.style.position = 'absolute';
        badge.style.right = '-0.1875rem';
        badge.style.bottom = '-0.1875rem';
        badge.style.display = 'inline-flex';
        badge.style.alignItems = 'center';
        badge.style.justifyContent = 'center';
        badge.style.width = '0.75rem';
        badge.style.height = '0.75rem';
        badge.style.borderRadius = '0.125rem';
        const save = this.#document.createElement('i');
        save.className = SAVE_ICON_CLASS;
        save.style.width = '0.625rem';
        save.style.height = '0.625rem';
        badge.append(save);
        root.append(cube, badge);
        return root;
    }
    /** Install the theme-aware occlusion plate used behind the foreground disk. */
    #ensureStyles() {
        if (this.#document.getElementById(STYLE_ID))
            return;
        const style = this.#document.createElement('style');
        style.id = STYLE_ID;
        style.textContent = `
      [${SAVE_BADGE_ATTRIBUTE}] {
        background: var(--comfy-menu-bg, rgb(24 24 27));
      }
    `;
        this.#document.head.append(style);
    }
}
