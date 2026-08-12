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
/** Compose the focused state, revision, mutation, preview, and view owners for Cube browsing. */
import { injectBrowserStyles } from './BrowserStyles.js';
import { CubeBrowserCatalog } from './CubeBrowserCatalog.js';
import { resolveBrowserApi, } from './CubeBrowserContracts.js';
import { CubeBrowserMetadataEditor } from './CubeBrowserMetadataEditor.js';
import { CubeBrowserMutationCommands } from './CubeBrowserMutationCommands.js';
import { CubeBrowserPlacementCommands } from './CubeBrowserPlacementCommands.js';
import { CubeBrowserPreferences } from './CubeBrowserPreferences.js';
import { CubeBrowserPreviewCoordinator } from './CubeBrowserPreviewCoordinator.js';
import { CubeBrowserRevisionCoordinator } from './CubeBrowserRevisionCoordinator.js';
import { CubeBrowserStore } from './CubeBrowserStore.js';
import { CubeBrowserView } from './CubeBrowserView.js';
import { CubeBrowserViewHost } from './CubeBrowserViewHost.js';
import { CubePreviewRenderer } from './CubePreviewRenderer.js';
/** Coordinate Cube browser lifecycle and delegate each use case to its authoritative owner. */
export class CubeBrowserController {
    adapter;
    toast;
    store = new CubeBrowserStore();
    view;
    preview;
    preferences;
    catalog;
    previewCoordinator;
    revisions;
    editor;
    mutations;
    placement;
    viewHost;
    actions = {};
    initialized = false;
    constructor({ adapter = null, api, storage = null, toast = null, } = {}) {
        this.adapter = adapter;
        this.toast = toast;
        const toastRelay = {
            push: (severity, summary, detail) => this.toast?.push(severity, summary, detail),
        };
        const resolvedApi = resolveBrowserApi(api);
        this.view = new CubeBrowserView({ adapter });
        this.preview = new CubePreviewRenderer({ adapter });
        this.preferences = new CubeBrowserPreferences({ store: this.store, storage });
        const getCube = (key, entries) => this.catalog.getCubeBySelectionKey(key, entries);
        const render = () => this.render();
        this.catalog = new CubeBrowserCatalog({
            adapter,
            api: resolvedApi,
            store: this.store,
            preferences: this.preferences,
            toast: toastRelay,
            getActions: () => this.actions,
            render,
            selectFirst: (key) => this.selectCube(key, { focus: false, silent: true }),
        });
        this.previewCoordinator = new CubeBrowserPreviewCoordinator({
            adapter,
            api: resolvedApi,
            store: this.store,
            preview: this.preview,
            getCube,
        });
        this.revisions = new CubeBrowserRevisionCoordinator({
            api: resolvedApi,
            store: this.store,
            toast: toastRelay,
            getCube,
            render,
            requestPreview: (key) => void this.previewCoordinator.request(key),
        });
        this.editor = new CubeBrowserMetadataEditor({
            api: resolvedApi,
            store: this.store,
            view: this.view,
            toast: toastRelay,
            getActions: () => this.actions,
            getCube,
            getCubeKey: (cube) => this.getCubeKey(cube),
            refresh: () => this.refresh({ force: true }),
            selectCube: (key) => this.selectCube(key, { focus: false, silent: true }),
            render,
        });
        this.mutations = new CubeBrowserMutationCommands({
            api: resolvedApi,
            store: this.store,
            toast: toastRelay,
            getActions: () => this.actions,
            getCube,
            removeCube: (key) => this.removeCubeFromLists(key),
            refresh: () => this.refresh({ force: true }),
            selectCube: (key) => this.selectCube(key, { focus: false, silent: true }),
            render,
        });
        this.placement = new CubeBrowserPlacementCommands({
            store: this.store,
            toast: toastRelay,
            getActions: () => this.actions,
            getCube,
            getVersionOption: () => this.revisions.getSelectedVersionOption(),
            close: () => this.close(),
            render,
        });
        this.viewHost = new CubeBrowserViewHost({
            store: this.store,
            view: this.view,
            preview: this.preview,
            getActions: () => this.actions,
            initialize: () => this.ensureInitialized(),
            bindHandlers: () => this.bindHandlers(),
            refresh: (force) => this.refresh({ force }),
            applyFilters: () => this.applyFilters(),
            render,
            getCube,
            getCubeKey: (cube) => this.getCubeKey(cube),
            selectCube: (key) => this.selectCube(key, { focus: false, silent: true }),
            beginEdit: (cube) => this.editor.begin(cube),
        });
    }
    configure({ actions = {}, helpers = {}, placement = {} } = {}) {
        this.actions = actions;
        this.preview.setHelpers(helpers);
        this.preview.setPlacementActions(placement);
    }
    async setup() {
        await this.ensureInitialized();
    }
    dispose() {
        this.preview.dispose();
    }
    getCubes() {
        return this.store.state.cubes;
    }
    getCubeById(cubeId) {
        return this.catalog.getCubeById(cubeId);
    }
    getCubeKey(cube) {
        return this.catalog.getCubeKey(cube);
    }
    getCubeBySelectionKey(cubeKey, entries = this.store.state.filtered) {
        return this.catalog.getCubeBySelectionKey(cubeKey, entries);
    }
    isWritableCube(cube) {
        return Boolean(cube?.is_writable);
    }
    getModelSuggestions() {
        return this.store.state.modelOptions.slice();
    }
    setDirtyCubeIds(ids) {
        this.store.setDirtyCubeIds(ids);
        this.render();
    }
    setBusy(value) {
        this.store.setBusy(Boolean(value));
        this.render();
    }
    async ensureInitialized() {
        if (this.initialized)
            return;
        this.store.setDropOrigin([0, 0]);
        injectBrowserStyles(this.adapter?.getDocument?.());
        this.initialized = true;
        await this.catalog.initialize();
    }
    ensureView({ mount = null } = {}) {
        return this.viewHost.ensureView(mount);
    }
    bindHandlers() {
        this.view.setHandlers({
            onClose: () => this.close(),
            onPlace: () => this.placeCube(),
            onFavoriteToggle: () => this.toggleFavorite(),
            onEditToggle: () => this.toggleEdit(),
            onEditSave: () => this.saveEdit(),
            onEditCancel: () => this.cancelEdit(),
            onDelete: () => this.requestDelete(),
            onPromote: () => this.requestPromotion(),
            onSearchChange: (value) => this.updateSearch(value),
            onSelect: (key, options) => this.selectCube(key, options),
            onToggleAuthorGroup: (key) => this.toggleAuthorGroup(key),
            onImport: () => this.loadSelectedRevision(),
            onMoveSelection: (delta) => this.moveSelection(delta),
            onClearLineage: (selected) => this.clearLineage(selected),
            onVersionSelect: (version) => this.revisions.selectVersion(version),
            onVersionCommit: (version) => this.revisions.commitVersionInput(version),
            onVersionClosest: (version) => this.revisions.findClosestVersionOption(version, this.store.state.versionOptions),
        });
    }
    mountEmbedded(container) {
        this.viewHost.mountEmbedded(container);
    }
    open({ triggerButton } = {}) {
        return this.viewHost.open(triggerButton || null);
    }
    openForEdit({ cubeId } = {}) {
        return this.viewHost.openForEdit(cubeId);
    }
    close() {
        this.viewHost.close();
    }
    async refresh({ force = false } = {}) {
        await this.ensureInitialized();
        return this.catalog.refresh(force);
    }
    applyFilters() {
        this.catalog.applyFilters();
    }
    updateSearch(value) {
        this.store.setSearchQuery(value || '');
        this.applyFilters();
        this.render();
    }
    toggleAuthorGroup(key) {
        if (this.preferences.toggleAuthorGroup(key))
            this.render();
    }
    selectCube(cubeKey, options = {}) {
        const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
        this.store.setSelected(key || null);
        this.revisions.selectCube(this.getCubeBySelectionKey(key));
        this.store.setEditing(false, null);
        this.view.editInputs = null;
        if (options.focus)
            this.view.elements?.placeButton?.focus?.();
        if (!options.silent)
            this.preferences.rememberRecent(this.store.state.selected);
        this.preferences.ensureAuthorGroupOpen(this.catalog.getAuthorGroupKey(this.store.state.selected));
        this.applyFilters();
        this.render();
        void this.previewCoordinator.request(this.store.state.selected);
        void this.revisions.requestRevisions(this.store.state.selected).catch(() => { });
    }
    render() {
        const selected = this.getCubeBySelectionKey(this.store.state.selected);
        const displayName = selected?.display_name?.trim() || selected?.name || null;
        this.preview.setContext({
            selected: displayName,
            selectedId: selected?.cube_id || null,
            busy: this.store.state.busy,
        });
        this.view.update(this.store.state);
        if (!selected) {
            this.preview.update({ name: null, payload: null, loading: false, error: null });
        }
        else {
            void this.previewCoordinator.request(this.getCubeKey(selected));
        }
    }
    toggleFavorite() {
        if (this.preferences.toggleFavorite(this.store.state.selected)) {
            this.applyFilters();
            this.render();
        }
    }
    toggleEdit() {
        this.editor.toggle();
    }
    cancelEdit() {
        this.editor.cancel();
    }
    async saveEdit() {
        await this.editor.save();
    }
    async clearLineage(selected) {
        await this.mutations.clearLineage(selected);
    }
    async requestDelete() {
        await this.mutations.requestDelete();
    }
    async requestPromotion() {
        await this.mutations.requestPromotion();
    }
    removeCubeFromLists(cubeKey) {
        const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
        if (!key)
            return;
        this.catalog.removeCube(key);
        this.preferences.forgetCube(key);
        this.render();
    }
    importCube() {
        this.placement.importCube();
    }
    async loadSelectedRevision() {
        await this.placement.loadSelectedRevision();
    }
    placeCube() {
        this.placement.placeCube();
    }
    moveSelection(delta) {
        const list = this.store.state.filtered;
        if (!list.length)
            return;
        const current = list.findIndex((cube) => this.getCubeKey(cube) === this.store.state.selected);
        const next = (current + delta + list.length) % list.length;
        const cube = list[next];
        if (cube)
            this.selectCube(this.getCubeKey(cube), { focus: false, silent: true });
    }
}
