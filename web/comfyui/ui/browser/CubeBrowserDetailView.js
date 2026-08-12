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
/** Own Cube browser detail selection and action presentation. */
import { createCubeIconElement } from '../core/CubeIconResolver.js';
/** Render the selected Cube detail pane and its action availability. */
export class CubeBrowserDetailView {
    options;
    constructor(options) {
        this.options = options;
    }
    /** Synchronize selected metadata, version controls, icon, and actions. */
    render(state) {
        const selected = getSelectedCube(state);
        if (!selected) {
            this.renderEmpty();
            this.updateActionState(state, null);
            return;
        }
        if (state.editing)
            this.options.metadata.renderEditor(selected, state);
        else
            this.options.metadata.renderReadOnly(selected);
        this.updateIcon(selected);
        this.options.versions.render(state, selected);
        this.updateFavorite(state, selected);
        this.updateEditingActions(state, selected);
        this.updateActionState(state, selected);
    }
    renderEmpty() {
        const elements = this.options.elements;
        if (!elements.detailTitle || !elements.detailMeta || !elements.detailDescription)
            return;
        elements.detailTitle.textContent = 'Select a cube to see details';
        if (elements.detailIcon) {
            elements.detailIcon.replaceChildren();
            elements.detailIcon.className = 'sugarcubes-cube-icon is-generic';
        }
        elements.detailMeta.replaceChildren();
        elements.detailDescription.textContent = '';
        this.options.metadata.dispose();
        elements.favoriteButton?.classList.add('is-empty');
        if (elements.favoriteButton) {
            elements.favoriteButton.textContent = '\u2606';
            elements.favoriteButton.disabled = true;
        }
        elements.promoteButton?.classList.add('sugarcubes-browser__action-hidden');
        elements.editSaveButton?.classList.add('sugarcubes-browser__action-hidden');
        elements.editCancelButton?.classList.add('sugarcubes-browser__action-hidden');
        elements.placeButton?.classList.remove('sugarcubes-browser__action-placeholder');
        this.options.versions.render(null, null);
    }
    updateFavorite(state, selected) {
        const button = this.options.elements.favoriteButton;
        if (!button)
            return;
        const favorite = state.favorites.has(getCubeSelectionKey(selected));
        if (state.editing) {
            button.disabled = true;
            button.classList.toggle('sugarcubes-browser__action-placeholder', !favorite);
            button.classList.add('is-locked');
            button.classList.remove('is-empty');
            button.textContent = '\u2605';
            button.title = favorite ? 'Favourite' : '';
        }
        else {
            button.disabled = false;
            button.classList.remove('sugarcubes-browser__action-placeholder', 'is-locked');
            button.classList.toggle('is-empty', !favorite);
            button.textContent = favorite ? '\u2605' : '\u2606';
            button.title = favorite ? 'Remove favourite' : 'Mark as favourite';
        }
    }
    updateEditingActions(state, selected) {
        const elements = this.options.elements;
        const writable = Boolean(selected.is_writable);
        if (elements.deleteButton) {
            elements.deleteButton.disabled = state.busy || !state.selected || !writable;
            elements.deleteButton.classList.toggle('sugarcubes-browser__action-hidden', state.editing || !writable);
        }
        if (elements.promoteButton) {
            const promotable = writable && isPersonalCubeEntry(selected);
            elements.promoteButton.disabled = state.busy || !state.selected || !promotable;
            elements.promoteButton.classList.toggle('sugarcubes-browser__action-hidden', state.editing || !promotable);
        }
        if (elements.editButton) {
            elements.editButton.disabled = state.busy || !state.selected || !writable;
            elements.editButton.classList.toggle('sugarcubes-browser__action-hidden', state.editing || !writable);
        }
        if (elements.editSaveButton) {
            elements.editSaveButton.disabled =
                state.busy ||
                    !state.selected ||
                    !writable ||
                    (state.editing && this.options.metadata.isIdentityInvalid());
            elements.editSaveButton.classList.toggle('sugarcubes-browser__action-hidden', !state.editing);
        }
        if (elements.editCancelButton) {
            elements.editCancelButton.disabled = state.busy || !state.selected || !writable;
            elements.editCancelButton.classList.toggle('sugarcubes-browser__action-hidden', !state.editing);
        }
        if (elements.placeButton) {
            elements.placeButton.disabled = state.busy || !state.selected;
            elements.placeButton.classList.toggle('sugarcubes-browser__action-placeholder', state.editing);
        }
    }
    updateActionState(state, selected) {
        const elements = this.options.elements;
        const writable = Boolean(selected?.is_writable);
        const disabled = state.busy || !state.selected;
        if (elements.placeButton)
            elements.placeButton.disabled = disabled;
        if (elements.deleteButton)
            elements.deleteButton.disabled = disabled || !writable;
        if (elements.promoteButton)
            elements.promoteButton.disabled = disabled || !writable || !isPersonalCubeEntry(selected);
        if (elements.editButton)
            elements.editButton.disabled = disabled || !writable;
        if (elements.editSaveButton)
            elements.editSaveButton.disabled = disabled || !writable;
        if (elements.editCancelButton)
            elements.editCancelButton.disabled = disabled || !writable;
    }
    updateIcon(selected) {
        const current = this.options.elements.detailIcon;
        const documentRef = this.options.documentRef;
        if (!current || !documentRef)
            return;
        const icon = createCubeIconElement(documentRef, {
            icon: selected.icon,
            cube_id: selected.cube_id,
            default_alias: selected.default_alias || selected.display_name || selected.name || '',
        }, 'sugarcubes-cube-icon sugarcubes-browser__detail-icon');
        if (!icon)
            return;
        current.replaceWith(icon);
        this.options.elements.detailIcon = icon;
    }
}
function getSelectedCube(state) {
    return state.filtered.find((cube) => getCubeSelectionKey(cube) === state.selected) || null;
}
function getCubeSelectionKey(cube) {
    return typeof cube?.cube_id === 'string' ? cube.cube_id.trim() : '';
}
function isPersonalCubeEntry(cube) {
    return (cube?.cube_id || '').trim().toLowerCase().startsWith('local/personal/');
}
