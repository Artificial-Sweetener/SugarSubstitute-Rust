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
/** Adapt feature-owned checked choices to the universal Fluent menu. */
import { ContextMenu } from '../menus/ContextMenu.js';
/** Preserve feature selectors and selection policy while delegating every popup concern. */
export class ComfyToolboxCheckMenuPresenter {
    #featureMenuAttribute;
    #featureItemAttribute;
    #menu;
    #anchor = null;
    #signature = '';
    /** Bind public feature markers without duplicating menu rendering or placement. */
    constructor(options) {
        this.#featureMenuAttribute = options.featureMenuAttribute;
        this.#featureItemAttribute = options.featureItemAttribute;
        this.#menu = new ContextMenu(options.document);
    }
    /** Toggle one feature's checked menu at its toolbar control. */
    toggle(anchor, model) {
        if (this.isOpen()) {
            this.close();
            return;
        }
        this.#anchor = anchor;
        this.refresh(model);
    }
    /** Reconcile changed choices while retaining the authoritative feature callback. */
    refresh(model) {
        const anchor = this.#anchor;
        if (!anchor)
            return;
        const items = model.items();
        if (model.selectionMode === 'single' && items.filter((item) => item.checked).length !== 1) {
            this.close();
            throw new Error('Single-selection toolbox menus require exactly one checked item.');
        }
        const signature = JSON.stringify(items);
        if (signature === this.#signature && this.isOpen())
            return;
        if (!items.length) {
            this.close();
            return;
        }
        const entries = items.map((item) => ({
            label: item.label,
            checked: item.checked,
            selection: model.selectionMode,
            attributes: {
                'data-sugarcubes-toolbox-check-menu-item': item.id,
                [this.#featureItemAttribute]: item.id,
            },
            keepOpen: !model.closeOnSelect,
            select: () => {
                model.select(item.id);
                if (!model.closeOnSelect)
                    this.refresh(model);
            },
        }));
        if (this.#menu.isOpen()) {
            this.#menu.updateEntries(entries);
            this.#signature = signature;
            return;
        }
        this.#menu.open({
            label: model.ariaLabel,
            trigger: anchor,
            anchor: { kind: 'below', bounds: anchor.getBoundingClientRect() },
            attributes: { 'data-sugarcubes-toolbox-check-menu': '', [this.#featureMenuAttribute]: '' },
            entries,
            closed: () => {
                this.#signature = '';
                anchor.setAttribute('aria-expanded', 'false');
            },
        });
        this.#signature = signature;
        anchor.setAttribute('aria-expanded', 'true');
    }
    /** Report whether the shared menu currently owns a visible tree. */
    isOpen() {
        return this.#menu.isOpen();
    }
    /** Dismiss through the shared owner and clear the originating toolbar control. */
    close = () => {
        this.#menu.close();
        this.#anchor = null;
        this.#signature = '';
    };
    /** Release the feature's transient menu. */
    dispose() {
        this.close();
    }
}
