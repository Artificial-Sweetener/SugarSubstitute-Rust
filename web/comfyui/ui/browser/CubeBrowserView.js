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
/** Compose the Cube browser shell, catalog list, details, metadata, and version views. */
import { CubeBrowserCatalogListView } from './CubeBrowserCatalogListView.js';
import { CubeBrowserDetailView } from './CubeBrowserDetailView.js';
import { CubeBrowserMetadataView } from './CubeBrowserMetadataView.js';
import { CubeBrowserVersionView } from './CubeBrowserVersionView.js';
import { CubeBrowserViewBuilder } from './CubeBrowserViewBuilder.js';
/** Coordinate focused browser presentation owners behind the established view API. */
export class CubeBrowserView {
    documentRef;
    windowRef;
    elements = {};
    handlers = {};
    state = null;
    metadata;
    versions;
    list;
    builder;
    detail = null;
    constructor({ adapter = null } = {}) {
        this.documentRef = adapter?.getDocument?.() || null;
        this.windowRef = adapter?.getWindow?.() || null;
        this.metadata = new CubeBrowserMetadataView({
            documentRef: this.documentRef,
            getElements: () => this.elements,
            getHandlers: () => this.handlers,
        });
        this.versions = new CubeBrowserVersionView({
            documentRef: this.documentRef,
            windowRef: this.windowRef,
            getState: () => this.state,
            getSelected: () => this.getSelectedCube(),
            getHandlers: () => this.handlers,
        });
        this.list = new CubeBrowserCatalogListView({
            documentRef: this.documentRef,
            getElements: () => this.elements,
        });
        this.builder = new CubeBrowserViewBuilder({
            documentRef: this.documentRef,
            windowRef: this.windowRef,
            versions: this.versions,
            getHandlers: () => this.handlers,
            isActive: () => this.isActive(),
        });
    }
    get editInputs() {
        return this.metadata.getEditInputs();
    }
    set editInputs(value) {
        if (value === null)
            this.metadata.dispose();
    }
    setHandlers(handlers = {}) {
        this.handlers = handlers;
    }
    build() {
        this.elements = this.builder.build();
        this.detail = new CubeBrowserDetailView({
            documentRef: this.documentRef,
            elements: this.elements,
            metadata: this.metadata,
            versions: this.versions,
        });
        return this.elements;
    }
    mount(container) {
        if (container && this.elements.dialog)
            container.replaceChildren(this.elements.dialog);
    }
    focusSearch() {
        this.elements.searchInput?.focus();
        this.elements.searchInput?.select?.();
    }
    scrollIntoView() {
        this.elements.dialog?.scrollIntoView?.({ block: 'start', behavior: 'smooth' });
    }
    update(state) {
        this.state = state;
        if (!state)
            return;
        this.list.render(state);
        this.detail?.render(state);
    }
    getEditInputs() {
        return this.metadata.getEditInputs();
    }
    getPreviewElements() {
        return {
            canvas: this.elements.previewCanvas ?? null,
            container: this.elements.previewContainer ?? null,
            status: this.elements.previewStatus ?? null,
        };
    }
    isActive() {
        return Boolean(this.elements.dialog?.isConnected);
    }
    getSelectedCube() {
        return (this.state?.filtered.find((cube) => cube.cube_id?.trim() === this.state?.selected) || null);
    }
}
