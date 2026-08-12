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
/** Own Cube browser catalog list projection and selection styling. */
import { createCubeIconElement } from '../core/CubeIconResolver.js';
/** Render grouped catalog rows into the browser list surface. */
export class CubeBrowserCatalogListView {
    options;
    constructor(options) {
        this.options = options;
    }
    /** Synchronize search input, grouped rows, empty state, and selection. */
    render(state) {
        const elements = this.options.getElements();
        if (elements.searchInput && elements.searchInput.value !== (state.searchQuery || '')) {
            elements.searchInput.value = state.searchQuery || '';
        }
        const listBody = elements.listBody;
        const documentRef = this.options.documentRef;
        if (!listBody || !documentRef)
            return;
        listBody.replaceChildren();
        if (!state.filtered.length) {
            if (elements.emptyState)
                elements.emptyState.style.display = state.loading ? 'none' : 'block';
            this.renderEmptyState(state);
            return;
        }
        if (elements.emptyState)
            elements.emptyState.style.display = 'none';
        const fragment = documentRef.createDocumentFragment();
        const openGroups = state.authorGroupsOpen instanceof Set ? state.authorGroupsOpen : new Set();
        for (const group of state.grouped) {
            const header = documentRef.createElement('button');
            header.type = 'button';
            header.className = 'sugarcubes-browser__author-toggle';
            header.dataset.author = group.key;
            header.setAttribute('aria-expanded', openGroups.has(group.key) ? 'true' : 'false');
            const label = documentRef.createElement('span');
            label.className = 'sugarcubes-browser__pack-label';
            const packName = documentRef.createElement('span');
            packName.className = 'sugarcubes-browser__pack-name';
            packName.textContent = group.label;
            label.appendChild(packName);
            if (group.authorLabel) {
                const author = documentRef.createElement('span');
                author.className = 'sugarcubes-browser__pack-author';
                author.textContent = group.authorLabel;
                label.appendChild(author);
            }
            const count = documentRef.createElement('span');
            count.className = 'sugarcubes-browser__author-count';
            count.textContent = String(group.cubes.length);
            header.append(label, count);
            fragment.appendChild(header);
            const list = documentRef.createElement('div');
            list.className = 'sugarcubes-browser__author-list';
            list.dataset.authorList = group.key;
            if (!openGroups.has(group.key))
                list.classList.add('is-collapsed');
            for (const cube of group.cubes)
                list.appendChild(this.buildRow(cube, state));
            fragment.appendChild(list);
        }
        listBody.appendChild(fragment);
        this.renderEmptyState(state);
        this.renderSelection(state);
    }
    buildRow(cube, state) {
        const documentRef = this.options.documentRef ?? document;
        const key = getCubeSelectionKey(cube);
        const cubeId = typeof cube.cube_id === 'string' ? cube.cube_id.trim() : '';
        const dirty = Boolean(cubeId && state.dirtyCubeIds.has(cubeId));
        const row = documentRef.createElement('div');
        row.className = 'sugarcubes-browser__cube-row';
        row.dataset.cube = key;
        row.classList.toggle('is-selected', state.selected === key);
        row.classList.toggle('is-dirty', dirty);
        const title = documentRef.createElement('div');
        title.className = 'sugarcubes-browser__cube-title';
        const icon = createCubeIconElement(documentRef, {
            icon: cube.icon,
            cube_id: cube.cube_id,
            default_alias: cube.default_alias || cube.display_name || cube.name || '',
        });
        if (icon)
            title.appendChild(icon);
        const text = documentRef.createElement('span');
        text.className = 'sugarcubes-browser__cube-title-text';
        text.textContent = cube.default_alias || cube.display_name || cube.name || '';
        title.appendChild(text);
        if (dirty) {
            const dot = documentRef.createElement('span');
            dot.className = 'sugarcubes-browser__dirty-dot';
            dot.title = 'Unsaved changes';
            title.appendChild(dot);
        }
        const favorite = documentRef.createElement('span');
        favorite.className = 'sugarcubes-browser__favorite-indicator';
        favorite.textContent = state.favorites.has(key) ? '\u2605' : '';
        row.append(title, favorite);
        return row;
    }
    renderSelection(state) {
        this.options
            .getElements()
            .listBody?.querySelectorAll('[data-cube]')
            .forEach((row) => {
            row.classList.toggle('is-selected', row.dataset.cube === state.selected);
            if (row.dataset.cube === state.selected)
                row.scrollIntoView({ block: 'nearest' });
        });
    }
    renderEmptyState(state) {
        const empty = this.options.getElements().emptyState;
        if (!empty)
            return;
        empty.textContent = state.loading
            ? 'Loading cubes...'
            : state.error ||
                (!state.filtered.length ? 'No cubes found. Try exporting one.' : empty.textContent);
    }
}
function getCubeSelectionKey(cube) {
    return typeof cube?.cube_id === 'string' ? cube.cube_id.trim() : '';
}
