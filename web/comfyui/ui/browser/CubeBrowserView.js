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
/**
 * Own the SugarCubes cube browser layer in `frontend/comfyui/ui/browser/CubeBrowserView.js`.
 */
import { $el } from '/scripts/ui.js';
import { deriveCubeIdFromDefaultAlias, normalizeDefaultAliasTitle } from '../core/CubeId.js';
import { createCubeIconElement } from '../core/CubeIconResolver.js';
import { TARGET_MODEL_OPTIONS, deriveTargetModelCubeId, deriveTargetModelFromCubeId, normalizeTargetModel, } from '../core/ModelTargets.js';
function isLocalCubeEntry(cube) {
    const sourceType = typeof cube?.source?.type === 'string' ? cube.source.type.trim().toLowerCase() : '';
    const cubeId = typeof cube?.cube_id === 'string' ? cube.cube_id.trim().toLowerCase() : '';
    return sourceType === 'local' || cubeId.startsWith('local/');
}
function isPersonalCubeEntry(cube) {
    const cubeId = typeof cube?.cube_id === 'string' ? cube.cube_id.trim().toLowerCase() : '';
    return cubeId.startsWith('local/personal/');
}
/**
 * Coordinate cube browser view behavior for the SugarCubes UI.
 */
export class CubeBrowserView {
    documentRef;
    windowRef;
    elements;
    handlers;
    editInputs;
    state;
    editIdentityInvalid;
    versionComboboxOpen;
    versionHighlightedIndex;
    versionInputDirty;
    versionRenderSignature;
    constructor({ adapter = null } = {}) {
        this.documentRef = adapter?.getDocument?.() || null;
        this.windowRef = adapter?.getWindow?.() || null;
        this.elements = {};
        this.handlers = {};
        this.editInputs = null;
        this.state = null;
        this.editIdentityInvalid = false;
        this.versionComboboxOpen = false;
        this.versionHighlightedIndex = -1;
        this.versionInputDirty = false;
        this.versionRenderSignature = '';
    }
    setHandlers(handlers = {}) {
        this.handlers = handlers;
    }
    build() {
        const createIcon = (...classNames) => {
            const icon = this.documentRef?.createElement?.('i') || document.createElement('i');
            icon.classList.add(...classNames);
            return icon;
        };
        const createActionButton = ({ className, title, outlineIcon, filledIcon, disabled = false, }) => $el('button', {
            className,
            type: 'button',
            title,
            disabled,
        }, [createIcon('mdi', outlineIcon), createIcon('mdi', filledIcon)]);
        const dialog = $el('div', { className: 'sugarcubes-browser' });
        const searchInput = $el('input.p-inputtext.p-component', {
            type: 'text',
            placeholder: 'Search cubes...',
            autocomplete: 'off',
        });
        const listHeader = $el('div.sugarcubes-browser__list-header');
        const searchWrap = $el('div.sugarcubes-browser__search', [searchInput]);
        listHeader.append(searchWrap);
        const listBody = $el('div', { className: 'sugarcubes-browser__list-body' });
        const emptyState = $el('div.sugarcubes-browser__empty', 'No cubes found. Try exporting one.');
        const listContainer = $el('div.sugarcubes-browser__list', [listHeader, listBody, emptyState]);
        const detailTitle = $el('h3', 'Select a cube to see details');
        const detailIcon = $el('span.sugarcubes-cube-icon.is-generic');
        const detailMeta = $el('div.sugarcubes-browser__meta');
        const detailDescription = $el('pre');
        const previewCanvas = $el('canvas', { className: 'sugarcubes-browser__preview-canvas' });
        const previewStatus = $el('div.sugarcubes-browser__preview-status', '');
        const previewContainer = $el('div.sugarcubes-browser__preview', [previewCanvas, previewStatus]);
        const favoriteButton = $el('button', {
            className: 'sugarcubes-browser__favorite is-empty',
            type: 'button',
            title: 'Mark as favourite',
            textContent: '\u2606',
        });
        const editButton = createActionButton({
            className: 'sugarcubes-browser__edit',
            title: 'Edit cube metadata',
            outlineIcon: 'mdi-pencil-outline',
            filledIcon: 'mdi-pencil',
            disabled: true,
        });
        const editSaveButton = createActionButton({
            className: 'sugarcubes-browser__edit-save sugarcubes-browser__action-hidden',
            title: 'Save changes',
            outlineIcon: 'mdi-content-save-outline',
            filledIcon: 'mdi-content-save',
            disabled: true,
        });
        const editCancelButton = createActionButton({
            className: 'sugarcubes-browser__edit-cancel sugarcubes-browser__action-hidden',
            title: 'Cancel edits',
            outlineIcon: 'mdi-close-circle-outline',
            filledIcon: 'mdi-close-circle',
            disabled: true,
        });
        const deleteButton = createActionButton({
            className: 'sugarcubes-browser__delete',
            title: 'Delete cube',
            outlineIcon: 'mdi-trash-can-outline',
            filledIcon: 'mdi-trash-can',
            disabled: true,
        });
        const promoteButton = createActionButton({
            className: 'sugarcubes-browser__promote',
            title: 'Move to cube pack',
            outlineIcon: 'mdi-package-up',
            filledIcon: 'mdi-package-up',
            disabled: true,
        });
        const placeButton = createActionButton({
            className: 'sugarcubes-browser__place',
            title: 'Place',
            outlineIcon: 'mdi-arrow-right-bold-outline',
            filledIcon: 'mdi-arrow-right-bold',
            disabled: true,
        });
        const versionListboxId = 'sugarcubes-browser-version-listbox';
        const versionPrefix = $el('span.sugarcubes-browser__version-prefix', 'v');
        const versionInput = $el('input.sugarcubes-browser__version-input', {
            type: 'text',
            title: 'Spawn version',
            'aria-label': 'Spawn version',
            'aria-autocomplete': 'list',
            'aria-controls': versionListboxId,
            'aria-expanded': 'false',
            role: 'combobox',
            autocomplete: 'off',
            disabled: true,
        });
        const versionToggle = $el('button.sugarcubes-browser__version-toggle', {
            type: 'button',
            title: 'Show versions',
            'aria-label': 'Show versions',
            disabled: true,
        });
        const versionInputShell = $el('div.sugarcubes-browser__version-input-shell', [
            versionInput,
            versionToggle,
        ]);
        const versionListbox = $el('ul.sugarcubes-browser__version-listbox', {
            id: versionListboxId,
            role: 'listbox',
            hidden: true,
        });
        const versionControl = $el('div', {
            className: 'sugarcubes-browser__version-control sugarcubes-browser__action-hidden',
        }, [versionPrefix, versionInputShell, versionListbox]);
        const detailTitleGroup = $el('div.sugarcubes-browser__detail-title', [
            favoriteButton,
            detailIcon,
            detailTitle,
            versionControl,
        ]);
        const detailActions = $el('div.sugarcubes-browser__detail-actions', [
            editButton,
            editSaveButton,
            promoteButton,
            deleteButton,
            editCancelButton,
            placeButton,
        ]);
        const detailHeader = $el('div.sugarcubes-browser__detail-header', [
            detailTitleGroup,
            detailActions,
        ]);
        const detailBody = $el('div.sugarcubes-browser__detail-body', [detailMeta, detailDescription]);
        const detailContainer = $el('div.sugarcubes-browser__detail', [detailHeader, detailBody]);
        const detailStack = $el('div', {
            style: {
                display: 'flex',
                flexDirection: 'column',
                gap: '12px',
                minWidth: 0,
                flex: '0 1 auto',
            },
        }, [detailContainer, previewContainer]);
        const content = $el('div.sugarcubes-browser__content', [listContainer, detailStack]);
        dialog.append(content);
        placeButton.addEventListener('click', () => this.handlers.onPlace?.());
        favoriteButton.addEventListener('click', () => this.handlers.onFavoriteToggle?.());
        editButton.addEventListener('click', () => this.handlers.onEditToggle?.());
        editSaveButton.addEventListener('click', () => this.handlers.onEditSave?.());
        editCancelButton.addEventListener('click', () => this.handlers.onEditCancel?.());
        deleteButton.addEventListener('click', () => this.handlers.onDelete?.());
        promoteButton.addEventListener('click', () => this.handlers.onPromote?.());
        versionInput.addEventListener('focus', () => this.openVersionCombobox());
        versionInput.addEventListener('click', () => this.openVersionCombobox());
        versionInput.addEventListener('input', () => this.handleVersionInput());
        versionInput.addEventListener('keydown', (event) => this.handleVersionKeydown(event));
        versionInput.addEventListener('blur', () => {
            this.windowRef?.setTimeout?.(() => this.commitVersionCombobox({ close: true }), 0);
        });
        versionToggle.addEventListener('mousedown', (event) => event.preventDefault());
        versionToggle.addEventListener('click', () => {
            if (versionInput.disabled) {
                return;
            }
            const wasOpen = this.versionComboboxOpen;
            versionInput.focus();
            if (wasOpen) {
                this.closeVersionCombobox();
            }
            else {
                this.openVersionCombobox();
            }
        });
        searchInput.addEventListener('input', (event) => {
            const target = event.target instanceof HTMLInputElement ? event.target : null;
            this.handlers.onSearchChange?.(target?.value || '');
        });
        this.documentRef?.addEventListener?.('click', (event) => {
            const target = event.target instanceof Node ? event.target : null;
            if (versionControl && !versionControl.contains(target)) {
                this.commitVersionCombobox({ close: true });
            }
        });
        listBody.addEventListener('click', (event) => {
            const target = event.target instanceof Element ? event.target : null;
            const header = target?.closest('[data-author]');
            if (header) {
                this.handlers.onToggleAuthorGroup?.(header.dataset.author || '');
                return;
            }
            const row = target?.closest('[data-cube]');
            if (!row) {
                return;
            }
            this.handlers.onSelect?.(row.dataset.cube || '', { focus: true });
        });
        listBody.addEventListener('dblclick', (event) => {
            const target = event.target instanceof Element ? event.target : null;
            const row = target?.closest('[data-cube]');
            if (!row) {
                return;
            }
            this.handlers.onSelect?.(row.dataset.cube || '', { focus: false });
            this.handlers.onImport?.();
        });
        this.windowRef?.addEventListener?.('keydown', (event) => {
            if (!this.isActive()) {
                return;
            }
            const activeElement = this.documentRef?.activeElement || null;
            const isEditableElement = (activeElement instanceof HTMLInputElement ||
                activeElement instanceof HTMLTextAreaElement ||
                activeElement instanceof HTMLSelectElement) &&
                !('readOnly' in activeElement && activeElement.readOnly);
            if (isEditableElement) {
                if (event.key === 'Enter' && activeElement === searchInput) {
                    event.preventDefault();
                    this.handlers.onImport?.();
                }
                return;
            }
            if (event.key === 'ArrowDown') {
                event.preventDefault();
                this.handlers.onMoveSelection?.(1);
                return;
            }
            if (event.key === 'ArrowUp') {
                event.preventDefault();
                this.handlers.onMoveSelection?.(-1);
                return;
            }
            if (event.key === 'Enter' && activeElement === searchInput) {
                event.preventDefault();
                this.handlers.onImport?.();
            }
        });
        this.elements = {
            dialog,
            listBody,
            detailContainer,
            detailTitle,
            detailIcon,
            detailMeta,
            detailDescription,
            previewContainer,
            previewCanvas,
            previewStatus,
            favoriteButton,
            deleteButton,
            promoteButton,
            editButton,
            editSaveButton,
            editCancelButton,
            placeButton,
            versionControl,
            versionInput,
            versionToggle,
            versionListbox,
            searchInput,
            emptyState,
        };
        return this.elements;
    }
    mount(container) {
        const { dialog } = this.elements;
        if (container && dialog) {
            container.replaceChildren(dialog);
            return;
        }
    }
    focusSearch() {
        const searchInput = this.elements.searchInput;
        if (!searchInput) {
            return;
        }
        searchInput.focus();
        searchInput.select?.();
    }
    scrollIntoView() {
        this.elements.dialog?.scrollIntoView?.({ block: 'start', behavior: 'smooth' });
    }
    update(state) {
        this.state = state;
        if (!state) {
            return;
        }
        this.updateSearch(state);
        this.updateList(state);
        this.updateSelectionStyles(state);
        this.updateDetails(state);
        this.updateActionState(state);
    }
    updateSearch(state) {
        const searchInput = this.elements.searchInput;
        if (!searchInput) {
            return;
        }
        const nextValue = state.searchQuery || '';
        if (searchInput.value !== nextValue) {
            searchInput.value = nextValue;
        }
    }
    updateEmptyState(state) {
        const emptyState = this.elements.emptyState;
        if (!emptyState) {
            return;
        }
        if (state.loading) {
            emptyState.textContent = 'Loading cubes...';
            return;
        }
        if (state.error) {
            emptyState.textContent = state.error;
            return;
        }
        if (!state.filtered.length) {
            emptyState.textContent = 'No cubes found. Try exporting one.';
        }
    }
    updateList(state) {
        const listBody = this.elements.listBody;
        if (!listBody || !this.documentRef) {
            return;
        }
        listBody.replaceChildren();
        if (!state.filtered.length) {
            if (this.elements.emptyState) {
                this.elements.emptyState.style.display = state.loading ? 'none' : 'block';
            }
            this.updateEmptyState(state);
            return;
        }
        if (this.elements.emptyState) {
            this.elements.emptyState.style.display = 'none';
        }
        const fragment = this.documentRef.createDocumentFragment();
        const grouped = Array.isArray(state.grouped) ? state.grouped : [];
        const openGroups = state.authorGroupsOpen instanceof Set ? state.authorGroupsOpen : new Set();
        for (const group of grouped) {
            const header = this.documentRef.createElement('button');
            header.type = 'button';
            header.className = 'sugarcubes-browser__author-toggle';
            header.dataset.author = group.key;
            header.setAttribute('aria-expanded', openGroups.has(group.key) ? 'true' : 'false');
            const label = this.documentRef.createElement('span');
            label.className = 'sugarcubes-browser__pack-label';
            const packName = this.documentRef.createElement('span');
            packName.className = 'sugarcubes-browser__pack-name';
            packName.textContent = group.label;
            label.appendChild(packName);
            if (group.authorLabel) {
                const authorName = this.documentRef.createElement('span');
                authorName.className = 'sugarcubes-browser__pack-author';
                authorName.textContent = group.authorLabel;
                label.appendChild(authorName);
            }
            const count = this.documentRef.createElement('span');
            count.className = 'sugarcubes-browser__author-count';
            count.textContent = String(group.cubes.length);
            header.appendChild(label);
            header.appendChild(count);
            fragment.appendChild(header);
            const list = this.documentRef.createElement('div');
            list.className = 'sugarcubes-browser__author-list';
            list.dataset.authorList = group.key;
            if (!openGroups.has(group.key)) {
                list.classList.add('is-collapsed');
            }
            for (const cube of group.cubes) {
                const row = this.documentRef.createElement('div');
                row.className = 'sugarcubes-browser__cube-row';
                row.dataset.cube = getCubeSelectionKey(cube);
                const dirty = this.isCubeDirty(cube, state);
                if (state.selected === getCubeSelectionKey(cube)) {
                    row.classList.add('is-selected');
                }
                if (dirty) {
                    row.classList.add('is-dirty');
                }
                const title = this.documentRef.createElement('div');
                title.className = 'sugarcubes-browser__cube-title';
                const icon = createCubeIconElement(this.documentRef, {
                    icon: cube.icon,
                    cube_id: cube.cube_id,
                    default_alias: cube.default_alias || cube.display_name || cube.name || '',
                });
                if (icon) {
                    title.appendChild(icon);
                }
                const titleText = this.documentRef.createElement('span');
                titleText.className = 'sugarcubes-browser__cube-title-text';
                titleText.textContent = cube.default_alias || cube.display_name || cube.name || '';
                title.appendChild(titleText);
                if (dirty) {
                    const dirtyDot = this.documentRef.createElement('span');
                    dirtyDot.className = 'sugarcubes-browser__dirty-dot';
                    dirtyDot.title = 'Unsaved changes';
                    title.appendChild(dirtyDot);
                }
                const favorite = this.documentRef.createElement('span');
                favorite.className = 'sugarcubes-browser__favorite-indicator';
                favorite.textContent = state.favorites.has(getCubeSelectionKey(cube)) ? '\u2605' : '';
                row.appendChild(title);
                row.appendChild(favorite);
                list.appendChild(row);
            }
            fragment.appendChild(list);
        }
        listBody.appendChild(fragment);
        this.updateEmptyState(state);
    }
    updateSelectionStyles(state) {
        const listBody = this.elements.listBody;
        if (!listBody) {
            return;
        }
        const rows = listBody.querySelectorAll('[data-cube]');
        rows.forEach((row) => {
            if (row.dataset.cube === state.selected) {
                row.classList.add('is-selected');
                row.scrollIntoView({ block: 'nearest' });
            }
            else {
                row.classList.remove('is-selected');
            }
        });
    }
    updateDetails(state) {
        const detailTitle = this.elements.detailTitle;
        const detailMeta = this.elements.detailMeta;
        const detailDescription = this.elements.detailDescription;
        const detailIcon = this.elements.detailIcon;
        const favoriteButton = this.elements.favoriteButton;
        const deleteButton = this.elements.deleteButton;
        const promoteButton = this.elements.promoteButton;
        const editButton = this.elements.editButton;
        const editSaveButton = this.elements.editSaveButton;
        const editCancelButton = this.elements.editCancelButton;
        const placeButton = this.elements.placeButton;
        const versionControl = this.elements.versionControl;
        if (!detailTitle || !detailMeta || !detailDescription) {
            return;
        }
        const selected = state.filtered.find((cube) => getCubeSelectionKey(cube) === state.selected);
        if (!selected) {
            detailTitle.textContent = 'Select a cube to see details';
            if (detailIcon) {
                detailIcon.replaceChildren();
                detailIcon.className = 'sugarcubes-cube-icon is-generic';
            }
            detailMeta.replaceChildren();
            detailDescription.textContent = '';
            this.editInputs = null;
            if (favoriteButton) {
                favoriteButton.classList.add('is-empty');
                favoriteButton.textContent = '\u2606';
                favoriteButton.disabled = true;
            }
            if (deleteButton) {
                deleteButton.disabled = true;
            }
            if (promoteButton) {
                promoteButton.disabled = true;
                promoteButton.classList.add('sugarcubes-browser__action-hidden');
            }
            if (editButton) {
                editButton.disabled = true;
            }
            if (editSaveButton) {
                editSaveButton.disabled = true;
                editSaveButton.classList.add('sugarcubes-browser__action-hidden');
            }
            if (editCancelButton) {
                editCancelButton.disabled = true;
                editCancelButton.classList.add('sugarcubes-browser__action-hidden');
            }
            if (placeButton) {
                placeButton.disabled = true;
                placeButton.classList.remove('sugarcubes-browser__action-placeholder');
            }
            if (versionControl) {
                versionControl.classList.add('sugarcubes-browser__action-hidden');
            }
            return;
        }
        if (state.editing) {
            this.renderCubeMetadataEditor(selected, state);
        }
        else {
            this.renderCubeMetadataReadOnly(selected, state);
        }
        this.updateDetailIcon(selected);
        this.renderVersionControl(state, selected);
        const selectedWritable = Boolean(selected?.is_writable);
        if (favoriteButton) {
            const isFav = state.favorites.has(getCubeSelectionKey(selected));
            if (state.editing) {
                favoriteButton.disabled = true;
                favoriteButton.classList.toggle('sugarcubes-browser__action-placeholder', !isFav);
                favoriteButton.classList.add('is-locked');
                favoriteButton.classList.remove('is-empty');
                favoriteButton.textContent = '\u2605';
                favoriteButton.title = isFav ? 'Favourite' : '';
            }
            else {
                favoriteButton.disabled = false;
                favoriteButton.classList.remove('sugarcubes-browser__action-placeholder', 'is-locked');
                favoriteButton.classList.toggle('is-empty', !isFav);
                favoriteButton.textContent = isFav ? '\u2605' : '\u2606';
                favoriteButton.title = isFav ? 'Remove favourite' : 'Mark as favourite';
            }
        }
        if (deleteButton) {
            deleteButton.disabled = state.busy || !state.selected || !selectedWritable;
            deleteButton.classList.toggle('sugarcubes-browser__action-hidden', state.editing || !selectedWritable);
        }
        if (promoteButton) {
            const promotable = selectedWritable && isPersonalCubeEntry(selected);
            promoteButton.disabled = state.busy || !state.selected || !promotable;
            promoteButton.classList.toggle('sugarcubes-browser__action-hidden', state.editing || !promotable);
        }
        if (editButton) {
            editButton.disabled = state.busy || !state.selected || !selectedWritable;
            editButton.classList.toggle('sugarcubes-browser__action-hidden', state.editing || !selectedWritable);
        }
        if (editSaveButton) {
            editSaveButton.disabled =
                state.busy ||
                    !state.selected ||
                    !selectedWritable ||
                    (state.editing && this.editIdentityInvalid);
            editSaveButton.classList.toggle('sugarcubes-browser__action-hidden', !state.editing);
        }
        if (editCancelButton) {
            editCancelButton.disabled = state.busy || !state.selected || !selectedWritable;
            editCancelButton.classList.toggle('sugarcubes-browser__action-hidden', !state.editing);
        }
        if (placeButton) {
            placeButton.disabled = state.busy || !state.selected;
            placeButton.classList.toggle('sugarcubes-browser__action-placeholder', state.editing);
        }
    }
    getSelectedCubeFromState(state = this.state) {
        if (!state) {
            return null;
        }
        return state.filtered.find((cube) => getCubeSelectionKey(cube) === state.selected) || null;
    }
    openVersionCombobox() {
        const input = this.elements.versionInput;
        const options = Array.isArray(this.state?.versionOptions) ? this.state.versionOptions : [];
        if (!input || input.disabled || !options.length) {
            return;
        }
        this.versionComboboxOpen = true;
        this.updateVersionHighlight(input.value);
        this.renderVersionControl(this.state, this.getSelectedCubeFromState());
    }
    closeVersionCombobox() {
        this.versionComboboxOpen = false;
        const input = this.elements.versionInput;
        const listbox = this.elements.versionListbox;
        if (input) {
            input.setAttribute('aria-expanded', 'false');
            input.removeAttribute('aria-activedescendant');
        }
        if (listbox) {
            listbox.hidden = true;
        }
    }
    resetVersionComboboxState() {
        this.versionComboboxOpen = false;
        this.versionHighlightedIndex = -1;
        this.versionInputDirty = false;
    }
    handleVersionInput() {
        const input = this.elements.versionInput;
        if (!input) {
            return;
        }
        this.versionInputDirty = true;
        this.versionComboboxOpen = true;
        this.updateVersionHighlight(input.value);
        this.renderVersionControl(this.state, this.getSelectedCubeFromState());
    }
    handleVersionKeydown(event) {
        const options = Array.isArray(this.state?.versionOptions) ? this.state.versionOptions : [];
        if (!options.length) {
            return;
        }
        if (event.key === 'ArrowDown') {
            event.preventDefault();
            this.moveVersionHighlight(1);
            return;
        }
        if (event.key === 'ArrowUp') {
            event.preventDefault();
            this.moveVersionHighlight(-1);
            return;
        }
        if (event.key === 'Enter') {
            event.preventDefault();
            this.commitVersionCombobox({ close: true });
            return;
        }
        if (event.key === 'Tab') {
            this.commitVersionCombobox({ close: true });
            return;
        }
        if (event.key === 'Escape') {
            event.preventDefault();
            this.restoreVersionInput();
            this.closeVersionCombobox();
        }
    }
    updateVersionHighlight(inputValue) {
        const options = Array.isArray(this.state?.versionOptions) ? this.state.versionOptions : [];
        if (!options.length) {
            this.versionHighlightedIndex = -1;
            return;
        }
        const closest = this.handlers.onVersionClosest?.(inputValue);
        const closestIndex = closest
            ? options.findIndex((option) => option.value === closest.value && option.revisionRef === closest.revisionRef)
            : -1;
        if (closestIndex >= 0) {
            this.versionHighlightedIndex = closestIndex;
            return;
        }
        const selectedIndex = options.findIndex((option) => option.value === this.state?.selectedVersion);
        this.versionHighlightedIndex = selectedIndex >= 0 ? selectedIndex : 0;
    }
    getVisibleVersionEntries(options, inputValue) {
        const entries = options.map((option, index) => ({ option, index }));
        const query = normalizeVersionQuery(inputValue).toLowerCase();
        if (!this.versionInputDirty || !query) {
            return entries;
        }
        const matchingEntries = entries.filter(({ option }) => normalizeVersionQuery(option.value).toLowerCase().includes(query));
        if (matchingEntries.length) {
            return matchingEntries;
        }
        const closest = this.handlers.onVersionClosest?.(inputValue);
        const closestIndex = closest
            ? options.findIndex((option) => option.value === closest.value && option.revisionRef === closest.revisionRef)
            : -1;
        if (closestIndex >= 0) {
            const closestEntry = entries[closestIndex];
            return closestEntry ? [closestEntry] : [];
        }
        const selectedIndex = options.findIndex((option) => option.value === this.state?.selectedVersion);
        const selectedEntry = entries[selectedIndex >= 0 ? selectedIndex : 0];
        return selectedEntry ? [selectedEntry] : [];
    }
    moveVersionHighlight(direction) {
        const input = this.elements.versionInput;
        const options = Array.isArray(this.state?.versionOptions) ? this.state.versionOptions : [];
        const visibleEntries = this.getVisibleVersionEntries(options, input?.value || '');
        if (!visibleEntries.length) {
            return;
        }
        const currentVisibleIndex = visibleEntries.findIndex((entry) => entry.index === this.versionHighlightedIndex);
        const nextVisibleIndex = currentVisibleIndex >= 0
            ? (currentVisibleIndex + direction + visibleEntries.length) % visibleEntries.length
            : direction > 0
                ? 0
                : visibleEntries.length - 1;
        this.versionComboboxOpen = true;
        const nextEntry = visibleEntries[nextVisibleIndex];
        if (!nextEntry) {
            return;
        }
        this.versionHighlightedIndex = nextEntry.index;
        this.renderVersionControl(this.state, this.getSelectedCubeFromState());
    }
    restoreVersionInput() {
        const input = this.elements.versionInput;
        if (!input) {
            return;
        }
        const selectedValue = this.resolveSelectedVersionValue();
        input.value = selectedValue;
        this.versionInputDirty = false;
        this.updateVersionHighlight(selectedValue);
    }
    commitVersionCombobox({ close = false } = {}) {
        const input = this.elements.versionInput;
        const options = Array.isArray(this.state?.versionOptions) ? this.state.versionOptions : [];
        if (!input || input.disabled || !options.length) {
            if (close) {
                this.closeVersionCombobox();
            }
            return;
        }
        const highlighted = options[this.versionHighlightedIndex] || null;
        const typedValue = this.versionInputDirty
            ? input.value
            : highlighted?.value || input.value || this.resolveSelectedVersionValue();
        const committed = this.handlers.onVersionCommit?.(typedValue) || highlighted || null;
        const committedValue = committed?.value || this.resolveSelectedVersionValue() || options[0]?.value || '';
        input.value = committedValue;
        this.versionInputDirty = false;
        this.updateVersionHighlight(committedValue);
        if (close) {
            this.closeVersionCombobox();
        }
        else {
            this.renderVersionControl(this.state, this.getSelectedCubeFromState());
        }
    }
    resolveSelectedVersionValue() {
        const options = Array.isArray(this.state?.versionOptions) ? this.state.versionOptions : [];
        return (options.find((option) => option.value === this.state?.selectedVersion)?.value ||
            options[0]?.value ||
            '');
    }
    renderVersionControl(state, selected) {
        const control = this.elements.versionControl;
        const input = this.elements.versionInput;
        const toggle = this.elements.versionToggle;
        const listbox = this.elements.versionListbox;
        if (!control || !input || !toggle || !listbox) {
            return;
        }
        if (!state) {
            this.resetVersionComboboxState();
            control.classList.add('sugarcubes-browser__action-hidden');
            return;
        }
        const options = Array.isArray(state.versionOptions) ? state.versionOptions : [];
        const hide = !selected || state.editing || (!options.length && !state.revisionsLoading);
        control.classList.toggle('sugarcubes-browser__action-hidden', hide);
        control.classList.toggle('is-error', Boolean(state.versionError));
        control.classList.toggle('is-loading', Boolean(state.revisionsLoading));
        control.classList.toggle('is-single', options.length <= 1);
        if (hide) {
            this.resetVersionComboboxState();
            listbox.replaceChildren();
            listbox.hidden = true;
            input.value = '';
            input.setAttribute('aria-expanded', 'false');
            input.removeAttribute('aria-activedescendant');
            return;
        }
        const signature = `${state.selected || ''}|${options
            .map((option) => `${option.value}:${option.revisionRef}`)
            .join('|')}`;
        if (this.versionRenderSignature !== signature) {
            this.resetVersionComboboxState();
            this.versionRenderSignature = signature;
        }
        const disabled = Boolean(state.busy || state.revisionsLoading || state.versionError);
        input.disabled = disabled;
        toggle.disabled = disabled;
        input.title = state.versionError || 'Spawn version';
        toggle.title = state.versionError || 'Show versions';
        if (!this.versionInputDirty) {
            input.value = this.resolveSelectedVersionValue();
        }
        if (disabled || !options.length) {
            this.versionComboboxOpen = false;
        }
        if (this.versionComboboxOpen && this.versionHighlightedIndex < 0) {
            this.updateVersionHighlight(input.value);
        }
        const visibleEntries = this.getVisibleVersionEntries(options, input.value);
        if (this.versionComboboxOpen &&
            visibleEntries.length &&
            !visibleEntries.some((entry) => entry.index === this.versionHighlightedIndex)) {
            this.versionHighlightedIndex = visibleEntries[0]?.index ?? -1;
        }
        const children = visibleEntries.map(({ option, index }) => {
            const optionId = `${listbox.id}-option-${index}`;
            const isHighlighted = index === this.versionHighlightedIndex;
            const isSelected = option.value === state.selectedVersion;
            const child = $el('li', {
                id: optionId,
                className: 'sugarcubes-browser__version-option',
                role: 'option',
                'aria-selected': isSelected ? 'true' : 'false',
                textContent: option.value,
            });
            child.dataset.version = option.value;
            if (isHighlighted) {
                child.classList.add('is-highlighted');
            }
            child.addEventListener('mousedown', (event) => {
                event.preventDefault();
            });
            child.addEventListener('mouseover', () => {
                this.versionHighlightedIndex = index;
                this.renderVersionControl(this.state, this.getSelectedCubeFromState());
            });
            child.addEventListener('click', () => {
                input.value = option.value;
                this.versionInputDirty = true;
                this.versionHighlightedIndex = index;
                this.commitVersionCombobox({ close: true });
                input.focus();
            });
            return child;
        });
        listbox.replaceChildren(...children);
        listbox.hidden = !this.versionComboboxOpen || disabled || !visibleEntries.length;
        input.setAttribute('aria-expanded', listbox.hidden ? 'false' : 'true');
        if (!listbox.hidden && this.versionHighlightedIndex >= 0) {
            input.setAttribute('aria-activedescendant', `${listbox.id}-option-${this.versionHighlightedIndex}`);
        }
        else {
            input.removeAttribute('aria-activedescendant');
        }
    }
    updateActionState(state) {
        const placeButton = this.elements.placeButton;
        const deleteButton = this.elements.deleteButton;
        const promoteButton = this.elements.promoteButton;
        const editButton = this.elements.editButton;
        const editSaveButton = this.elements.editSaveButton;
        const editCancelButton = this.elements.editCancelButton;
        const selected = state.filtered.find((cube) => getCubeSelectionKey(cube) === state.selected);
        const selectedWritable = Boolean(selected?.is_writable);
        const disabled = state.busy || !state.selected;
        if (placeButton) {
            placeButton.disabled = disabled;
        }
        if (deleteButton) {
            deleteButton.disabled = disabled || !selectedWritable;
        }
        if (promoteButton) {
            promoteButton.disabled = disabled || !selectedWritable || !isPersonalCubeEntry(selected);
        }
        if (editButton) {
            editButton.disabled = disabled || !selectedWritable;
        }
        if (editSaveButton) {
            editSaveButton.disabled = disabled || !selectedWritable;
        }
        if (editCancelButton) {
            editCancelButton.disabled = disabled || !selectedWritable;
        }
    }
    isCubeDirty(cube, state) {
        const cubeId = typeof cube?.cube_id === 'string' ? cube.cube_id.trim() : '';
        return Boolean(cubeId && state.dirtyCubeIds?.has(cubeId));
    }
    updateDetailIcon(selected) {
        const detailIcon = this.elements.detailIcon;
        if (!detailIcon || !this.documentRef) {
            return;
        }
        const icon = createCubeIconElement(this.documentRef, {
            icon: selected?.icon,
            cube_id: selected?.cube_id,
            default_alias: selected?.default_alias || selected?.display_name || selected?.name || '',
        }, 'sugarcubes-cube-icon sugarcubes-browser__detail-icon');
        if (!icon) {
            return;
        }
        detailIcon.replaceWith(icon);
        this.elements.detailIcon = icon;
    }
    /**
     * Render the catalog metadata in the read-only detail pane.
     */
    renderCubeMetadataReadOnly(selected, _state) {
        const detailMeta = this.elements.detailMeta;
        const detailDescription = this.elements.detailDescription;
        const detailTitle = this.elements.detailTitle;
        if (!detailMeta || !detailDescription) {
            return;
        }
        if (detailTitle) {
            detailTitle.textContent =
                selected.default_alias || selected.display_name || selected.name || '';
        }
        const rows = [];
        const defaultAlias = selected.default_alias || selected.display_name || selected.name || '';
        if (defaultAlias) {
            rows.push(`Default Alias: ${defaultAlias}`);
        }
        if (isLocalCubeEntry(selected)) {
            rows.push('local');
        }
        else if (selected.author) {
            rows.push(`Author: ${selected.author}`);
        }
        const versionUpdated = [];
        if (selected.version) {
            versionUpdated.push(`Version: ${selected.version}`);
        }
        if (selected.mtime) {
            versionUpdated.push(`Updated: ${new Date(selected.mtime).toLocaleString()}`);
        }
        if (versionUpdated.length) {
            rows.push(versionUpdated.join(' | '));
        }
        if (selected.target_model) {
            rows.push(`Target Model: ${selected.target_model}`);
        }
        if (selected.cube_id) {
            rows.push(`ID: ${selected.cube_id}`);
        }
        if (selected.lineage && typeof selected.lineage === 'object') {
            const lineage = selected.lineage;
            const parts = [];
            if (lineage.name) {
                parts.push(lineage.name);
            }
            if (lineage.version) {
                parts.push(`v${lineage.version}`);
            }
            if (lineage.id) {
                parts.push(`(${lineage.id})`);
            }
            if (lineage.author) {
                parts.push(`by ${lineage.author}`);
            }
            if (lineage.forked_at) {
                parts.push(`forked ${lineage.forked_at}`);
            }
            rows.push(`Forked from: ${parts.join(' ')}`.trim());
        }
        if (Array.isArray(selected.supported_models) && selected.supported_models.length) {
            rows.push(`Models: ${selected.supported_models.join(', ')}`);
        }
        if (selected.author_url) {
            rows.push(`Website: ${selected.author_url}`);
        }
        if (Array.isArray(selected.tags) && selected.tags.length) {
            rows.push(`Tags: ${selected.tags.join(', ')}`);
        }
        detailMeta.replaceChildren(...rows.map((row) => $el('div', row)));
        detailDescription.textContent = selected.description || '';
        this.editInputs = null;
    }
    renderCubeMetadataEditor(selected, state) {
        const detailMeta = this.elements.detailMeta;
        const detailDescription = this.elements.detailDescription;
        const detailTitle = this.elements.detailTitle;
        if (!detailMeta || !detailDescription) {
            return;
        }
        if (detailTitle) {
            const nameInput = $el('input', {
                type: 'text',
                value: draftNameFromState(state, selected),
                className: 'sugarcubes-browser__title-input',
            });
            detailTitle.replaceChildren(nameInput);
            detailTitle.title = 'Rename this cube';
            this.editInputs = this.editInputs || {};
            this.editInputs.name = nameInput;
        }
        const draft = state.editDraft || {
            name: draftNameFromState(state, selected),
            original_name: draftNameFromState(state, selected),
            description: selected.description || '',
            current_cube_id: selected.cube_id || '',
            derived_cube_id: deriveEditableCubeId(selected.cube_id, draftNameFromState(state, selected))
                .value,
            cube_id: selected.cube_id || '',
            version: selected.version || '',
            author_url: selected.author_url || '',
            tags: Array.isArray(selected.tags) ? selected.tags : [],
            target_model: selected.target_model || deriveTargetModelFromCubeIdSafe(selected.cube_id),
            supported_models: Array.isArray(selected.supported_models) ? selected.supported_models : [],
        };
        const derivedIdentity = deriveEditableCubeId(selected.cube_id, draft.name || '', draft.target_model || '');
        draft.derived_cube_id = derivedIdentity.value;
        this.editIdentityInvalid = Boolean(derivedIdentity.error);
        const inputs = this.editInputs || {};
        const fields = [
            { key: 'current_cube_id', label: 'ID', readOnly: true },
            {
                key: 'derived_cube_id',
                label: 'New ID',
                readOnly: true,
                title: derivedIdentity.error || 'Derived from Default Alias',
            },
            { key: 'version', label: 'Version' },
        ];
        const trailingFields = [
            { key: 'author_url', label: 'Website' },
            { key: 'tags', label: 'Tags' },
        ];
        detailMeta.replaceChildren();
        for (const field of fields) {
            const draftValue = draft[field.key];
            const value = Array.isArray(draftValue) ? draftValue.join(', ') : String(draftValue || '');
            const input = $el('input', { type: 'text', value });
            if (field.readOnly) {
                input.readOnly = true;
                input.title = field.title || 'Current canonical cube identity';
            }
            const row = $el('div.sugarcubes-browser__edit-field', [$el('label', field.label), input]);
            detailMeta.appendChild(row);
            inputs[field.key] = input;
        }
        const targetModelSelect = this.renderTargetModelEditor(draft.target_model || '');
        detailMeta.appendChild(targetModelSelect.container);
        inputs.target_model = targetModelSelect.input;
        const nameInput = inputs.name;
        const derivedCubeIdInput = inputs.derived_cube_id;
        if (nameInput && derivedCubeIdInput) {
            const updateDerivedIdentity = () => {
                const next = deriveEditableCubeId(selected.cube_id, nameInput.value || '', inputs.target_model?.value || '');
                derivedCubeIdInput.value = next.value;
                derivedCubeIdInput.title = next.error || 'Derived from Default Alias';
                this.editIdentityInvalid = Boolean(next.error);
                if (this.elements.editSaveButton) {
                    this.elements.editSaveButton.disabled =
                        state.busy || !state.selected || !selected.is_writable || this.editIdentityInvalid;
                }
            };
            nameInput.addEventListener('input', updateDerivedIdentity);
            inputs.target_model?.addEventListener('change', updateDerivedIdentity);
        }
        const modelsRow = this.renderSupportedModelsEditor(draft.supported_models || [], state);
        detailMeta.appendChild(modelsRow.container);
        inputs.supported_models = modelsRow;
        for (const field of trailingFields) {
            const draftValue = draft[field.key];
            const value = Array.isArray(draftValue) ? draftValue.join(', ') : String(draftValue || '');
            const input = $el('input', { type: 'text', value });
            const row = $el('div.sugarcubes-browser__edit-field', [$el('label', field.label), input]);
            detailMeta.appendChild(row);
            inputs[field.key] = input;
        }
        const descriptionInput = $el('textarea', {
            className: 'sugarcubes-browser__edit-textarea',
            value: draft.description || '',
            rows: 4,
        });
        const descriptionNodes = [descriptionInput];
        inputs.description = descriptionInput;
        if (selected.is_writable && selected.lineage && typeof selected.lineage === 'object') {
            const clearButton = $el('button.sugarcubes-browser__lineage-clear', {
                type: 'button',
                title: 'Remove fork lineage metadata',
            }, ['Clear lineage']);
            clearButton.addEventListener('click', () => {
                this.handlers.onClearLineage?.(selected);
            });
            descriptionNodes.push(clearButton);
        }
        detailDescription.replaceChildren(...descriptionNodes);
        this.editInputs = inputs;
    }
    renderTargetModelEditor(targetModel) {
        const input = $el('select', {
            className: 'sugarcubes-browser__target-model-select',
        });
        const normalizedTarget = normalizeTargetModel(targetModel);
        const options = normalizedTarget && !TARGET_MODEL_OPTIONS.includes(normalizedTarget)
            ? [normalizedTarget, ...TARGET_MODEL_OPTIONS]
            : TARGET_MODEL_OPTIONS;
        input.replaceChildren(...options.map((value) => $el('option', {
            value,
            textContent: value,
        })));
        input.value = normalizedTarget || '';
        return {
            input,
            container: $el('div.sugarcubes-browser__edit-field', [$el('label', 'Target model'), input]),
        };
    }
    renderSupportedModelsEditor(models, state) {
        const label = $el('label', 'Model(s)');
        const field = $el('div', { className: 'sugarcubes-browser__model-autocomplete' });
        const input = $el('input', {
            className: 'sugarcubes-browser__model-text-input',
            type: 'text',
            placeholder: 'SDXL, Flux .1 D',
            autocomplete: 'off',
            value: Array.isArray(models) ? models.filter(Boolean).join(', ') : '',
        });
        const listbox = $el('div', {
            className: 'sugarcubes-browser__model-suggestions',
            id: 'sugarcubes-browser-model-suggestions',
            role: 'listbox',
            hidden: true,
        });
        field.append(input, listbox);
        const container = $el('div.sugarcubes-browser__edit-field', [label, field]);
        const autocomplete = {
            container,
            input,
            listbox,
            options: Array.isArray(state.modelOptions) ? state.modelOptions.slice() : [],
            suggestions: [],
            highlightedIndex: -1,
            tokenStart: 0,
            tokenEnd: 0,
        };
        const closeSuggestions = () => {
            autocomplete.suggestions = [];
            autocomplete.highlightedIndex = -1;
            autocomplete.listbox.hidden = true;
            autocomplete.listbox.replaceChildren();
            autocomplete.input.removeAttribute('aria-activedescendant');
            autocomplete.input.setAttribute('aria-expanded', 'false');
        };
        const commitSuggestion = (value) => {
            if (!value) {
                return;
            }
            const { tokenStart, tokenEnd } = this.resolveModelTokenBounds(autocomplete.input.value, autocomplete.input.selectionStart ?? autocomplete.input.value.length, autocomplete.input.selectionEnd ?? autocomplete.input.value.length);
            const before = autocomplete.input.value.slice(0, tokenStart);
            const after = autocomplete.input.value.slice(tokenEnd);
            const normalizedBefore = before.endsWith(',') ? `${before} ` : before;
            const normalizedAfter = after.replace(/^\s*/, '');
            autocomplete.input.value = `${normalizedBefore}${value}${normalizedAfter}`;
            const caretPosition = (normalizedBefore + value).length;
            autocomplete.input.setSelectionRange(caretPosition, caretPosition);
            closeSuggestions();
            autocomplete.input.dispatchEvent(new Event('input', { bubbles: true }));
            autocomplete.input.focus();
        };
        const renderSuggestions = ({ preserveHighlight = false, } = {}) => {
            const { input: editorInput } = autocomplete;
            const { tokenStart, tokenEnd, token } = this.resolveModelTokenBounds(editorInput.value, editorInput.selectionStart ?? editorInput.value.length, editorInput.selectionEnd ?? editorInput.value.length);
            autocomplete.tokenStart = tokenStart;
            autocomplete.tokenEnd = tokenEnd;
            const matches = this.buildModelSuggestions(token, autocomplete.options, editorInput.value);
            const previousValue = preserveHighlight && autocomplete.highlightedIndex >= 0
                ? autocomplete.suggestions[autocomplete.highlightedIndex]
                : '';
            autocomplete.suggestions = matches;
            if (!matches.length) {
                autocomplete.highlightedIndex = -1;
            }
            else if (previousValue) {
                const preservedIndex = matches.indexOf(previousValue);
                autocomplete.highlightedIndex = preservedIndex >= 0 ? preservedIndex : 0;
            }
            else if (autocomplete.highlightedIndex >= 0) {
                autocomplete.highlightedIndex = Math.min(autocomplete.highlightedIndex, matches.length - 1);
            }
            else {
                autocomplete.highlightedIndex = 0;
            }
            if (!matches.length) {
                closeSuggestions();
                return;
            }
            const children = matches.map((option, index) => {
                const suggestionId = `sugarcubes-browser-model-suggestion-${index}`;
                const button = $el('button', {
                    id: suggestionId,
                    className: 'sugarcubes-browser__model-suggestion',
                    type: 'button',
                    role: 'option',
                    'aria-selected': index === autocomplete.highlightedIndex ? 'true' : 'false',
                    textContent: option,
                });
                if (index === autocomplete.highlightedIndex) {
                    button.classList.add('is-highlighted');
                    editorInput.setAttribute('aria-activedescendant', suggestionId);
                }
                button.addEventListener('mousedown', (event) => {
                    event.preventDefault();
                    commitSuggestion(option);
                });
                return button;
            });
            autocomplete.listbox.replaceChildren(...children);
            autocomplete.listbox.hidden = false;
            editorInput.setAttribute('aria-expanded', 'true');
        };
        input.addEventListener('input', () => {
            renderSuggestions();
        });
        input.addEventListener('focus', () => {
            renderSuggestions();
        });
        input.addEventListener('blur', () => {
            this.windowRef?.setTimeout?.(() => {
                if (this.documentRef?.activeElement &&
                    autocomplete.listbox.contains(this.documentRef.activeElement)) {
                    return;
                }
                closeSuggestions();
            }, 0);
        });
        input.addEventListener('keydown', (event) => {
            if (!autocomplete.suggestions.length) {
                return;
            }
            if (event.key === 'ArrowDown') {
                event.preventDefault();
                autocomplete.highlightedIndex =
                    (autocomplete.highlightedIndex + 1) % autocomplete.suggestions.length;
                renderSuggestions({ preserveHighlight: true });
                return;
            }
            if (event.key === 'ArrowUp') {
                event.preventDefault();
                autocomplete.highlightedIndex =
                    (autocomplete.highlightedIndex - 1 + autocomplete.suggestions.length) %
                        autocomplete.suggestions.length;
                renderSuggestions({ preserveHighlight: true });
                return;
            }
            if (event.key === 'Enter' || event.key === 'Tab') {
                if (autocomplete.highlightedIndex < 0) {
                    return;
                }
                event.preventDefault();
                commitSuggestion(autocomplete.suggestions[autocomplete.highlightedIndex]);
                return;
            }
            if (event.key === 'Escape') {
                event.preventDefault();
                closeSuggestions();
            }
        });
        input.setAttribute('aria-autocomplete', 'list');
        input.setAttribute('aria-controls', listbox.id);
        input.setAttribute('role', 'combobox');
        input.setAttribute('aria-expanded', 'false');
        return autocomplete;
    }
    buildModelSuggestions(token, options, fullValue) {
        const normalizedToken = typeof token === 'string' ? token.trim().toLowerCase() : '';
        if (!normalizedToken) {
            return [];
        }
        const existingValues = new Set(this.parseModelInputValue(fullValue)
            .map((entry) => entry.toLowerCase())
            .filter((entry) => entry !== normalizedToken));
        return options.filter((option) => {
            const normalizedOption = String(option).toLowerCase();
            if (!normalizedOption.includes(normalizedToken)) {
                return false;
            }
            if (existingValues.has(normalizedOption)) {
                return false;
            }
            return true;
        });
    }
    parseModelInputValue(value) {
        if (typeof value !== 'string') {
            return [];
        }
        return value
            .split(',')
            .map((entry) => entry.trim())
            .filter(Boolean);
    }
    resolveModelTokenBounds(value, selectionStart, selectionEnd) {
        const safeValue = typeof value === 'string' ? value : '';
        const start = typeof selectionStart === 'number' && Number.isInteger(selectionStart)
            ? selectionStart
            : safeValue.length;
        const end = typeof selectionEnd === 'number' && Number.isInteger(selectionEnd) ? selectionEnd : start;
        const tokenStart = safeValue.lastIndexOf(',', Math.max(0, start - 1)) + 1;
        let tokenEnd = safeValue.indexOf(',', end);
        if (tokenEnd < 0) {
            tokenEnd = safeValue.length;
        }
        const rawToken = safeValue.slice(tokenStart, tokenEnd);
        const leadingWhitespace = rawToken.match(/^\s*/)?.[0].length || 0;
        const trailingWhitespace = rawToken.match(/\s*$/)?.[0].length || 0;
        return {
            tokenStart: tokenStart + leadingWhitespace,
            tokenEnd: Math.max(tokenStart + leadingWhitespace, tokenEnd - trailingWhitespace),
            token: rawToken.trim(),
        };
    }
    getEditInputs() {
        return this.editInputs || null;
    }
    getPreviewElements() {
        return {
            canvas: this.elements.previewCanvas ?? null,
            container: this.elements.previewContainer ?? null,
            status: this.elements.previewStatus ?? null,
        };
    }
    isActive() {
        const active = this.documentRef?.activeElement || null;
        return Boolean(this.elements.dialog && active && this.elements.dialog.contains(active));
    }
}
function draftNameFromState(state, selected) {
    if (typeof state?.editDraft?.name === 'string' && state.editDraft.name.trim()) {
        return state.editDraft.name;
    }
    if (typeof selected?.default_alias === 'string' && selected.default_alias.trim()) {
        return selected.default_alias.trim();
    }
    if (typeof selected?.metadata?.default_alias === 'string' &&
        selected.metadata.default_alias.trim()) {
        return selected.metadata.default_alias.trim();
    }
    if (typeof selected?.display_name === 'string' && selected.display_name.trim()) {
        return selected.display_name.trim();
    }
    if (typeof selected?.name === 'string' && selected.name.trim()) {
        return selected.name.trim();
    }
    return '';
}
function deriveEditableCubeId(cubeId, defaultAlias, targetModel = '') {
    try {
        const normalizedAlias = normalizeDefaultAliasTitle(defaultAlias) || defaultAlias;
        const normalizedTarget = normalizeTargetModel(targetModel);
        const value = normalizedTarget
            ? deriveTargetModelCubeId({
                sourceCubeId: cubeId,
                targetModel: normalizedTarget,
                defaultAlias: normalizedAlias,
            })
            : deriveCubeIdFromDefaultAlias(cubeId, normalizedAlias);
        return { value, error: '' };
    }
    catch (error) {
        const fallback = typeof cubeId === 'string' ? cubeId.trim() : '';
        return {
            value: fallback,
            error: (error instanceof Error ? error.message : '') ||
                'Unable to derive cube id from Default Alias',
        };
    }
}
function deriveTargetModelFromCubeIdSafe(cubeId) {
    try {
        return normalizeTargetModel(deriveTargetModelFromCubeId(cubeId));
    }
    catch (_error) {
        return '';
    }
}
function normalizeVersionQuery(value) {
    if (typeof value !== 'string') {
        return '';
    }
    return value.trim().replace(/^v/i, '').trim();
}
function getCubeSelectionKey(cube) {
    const key = typeof cube?.cube_id === 'string' ? cube.cube_id.trim() : '';
    return key || '';
}
