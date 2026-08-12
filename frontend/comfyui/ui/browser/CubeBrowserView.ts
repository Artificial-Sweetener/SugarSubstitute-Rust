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

import { ModelAutocompleteControl } from '../controls/ModelAutocompleteControl.js';
import { CubeBrowserCatalogListView } from './CubeBrowserCatalogListView.js';
import { CubeBrowserDetailView } from './CubeBrowserDetailView.js';
import { CubeBrowserMetadataView } from './CubeBrowserMetadataView.js';
import { CubeBrowserVersionView } from './CubeBrowserVersionView.js';
import { CubeBrowserViewBuilder } from './CubeBrowserViewBuilder.js';
import type { CubeBrowserState, CubeLibraryEntry, CubeVersionOption } from './CubeBrowserStore.js';

interface BrowserViewAdapter {
  getDocument?(): Document | null;
  getWindow?(): Window | null;
}

export interface BrowserViewHandlers {
  onClose?(): void;
  onPlace?(): void;
  onFavoriteToggle?(): void;
  onEditToggle?(): void;
  onEditSave?(): void;
  onEditCancel?(): void;
  onDelete?(): void;
  onPromote?(): void;
  onSearchChange?(value: string): void;
  onSelect?(cubeKey: string, options: { focus?: boolean }): void;
  onToggleAuthorGroup?(key: string): void;
  onImport?(): void;
  onMoveSelection?(delta: number): void;
  onClearLineage?(selected: CubeLibraryEntry): void;
  onVersionSelect?(version: string): void;
  onVersionCommit?(version: string): CubeVersionOption | null;
  onVersionClosest?(version: string): CubeVersionOption | null;
}

export interface BrowserElements {
  dialog: HTMLDivElement;
  listBody: HTMLDivElement;
  detailContainer: HTMLDivElement;
  detailTitle: HTMLHeadingElement;
  detailIcon: HTMLSpanElement;
  detailMeta: HTMLDivElement;
  detailDescription: HTMLPreElement;
  previewContainer: HTMLDivElement;
  previewCanvas: HTMLCanvasElement;
  previewStatus: HTMLDivElement;
  favoriteButton: HTMLButtonElement;
  deleteButton: HTMLButtonElement;
  promoteButton: HTMLButtonElement;
  editButton: HTMLButtonElement;
  editSaveButton: HTMLButtonElement;
  editCancelButton: HTMLButtonElement;
  placeButton: HTMLButtonElement;
  versionControl: HTMLDivElement;
  versionInput: HTMLInputElement;
  versionToggle: HTMLButtonElement;
  versionListbox: HTMLUListElement;
  searchInput: HTMLInputElement;
  emptyState: HTMLDivElement;
}

export interface BrowserEditInputs {
  [key: string]:
    | HTMLInputElement
    | HTMLTextAreaElement
    | HTMLSelectElement
    | ModelAutocompleteControl
    | undefined;
  name?: HTMLInputElement;
  current_cube_id?: HTMLInputElement;
  derived_cube_id?: HTMLInputElement;
  description?: HTMLTextAreaElement;
  version?: HTMLInputElement;
  author_url?: HTMLInputElement;
  tags?: HTMLInputElement;
  target_model?: HTMLSelectElement;
  supported_models?: ModelAutocompleteControl;
}

/** Coordinate focused browser presentation owners behind the established view API. */
export class CubeBrowserView {
  private readonly documentRef: Document | null;
  private readonly windowRef: Window | null;
  elements: Partial<BrowserElements> = {};
  private handlers: BrowserViewHandlers = {};
  private state: CubeBrowserState | null = null;
  private readonly metadata: CubeBrowserMetadataView;
  private readonly versions: CubeBrowserVersionView;
  private readonly list: CubeBrowserCatalogListView;
  private readonly builder: CubeBrowserViewBuilder;
  private detail: CubeBrowserDetailView | null = null;

  constructor({ adapter = null }: { adapter?: BrowserViewAdapter | null } = {}) {
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

  get editInputs(): BrowserEditInputs | null {
    return this.metadata.getEditInputs();
  }

  set editInputs(value: BrowserEditInputs | null) {
    if (value === null) this.metadata.dispose();
  }

  setHandlers(handlers: BrowserViewHandlers = {}): void {
    this.handlers = handlers;
  }

  build(): Partial<BrowserElements> {
    this.elements = this.builder.build();
    this.detail = new CubeBrowserDetailView({
      documentRef: this.documentRef,
      elements: this.elements,
      metadata: this.metadata,
      versions: this.versions,
    });
    return this.elements;
  }

  mount(container: HTMLElement | null | undefined): void {
    if (container && this.elements.dialog) container.replaceChildren(this.elements.dialog);
  }

  focusSearch(): void {
    this.elements.searchInput?.focus();
    this.elements.searchInput?.select?.();
  }

  scrollIntoView(): void {
    this.elements.dialog?.scrollIntoView?.({ block: 'start', behavior: 'smooth' });
  }

  update(state: CubeBrowserState | null): void {
    this.state = state;
    if (!state) return;
    this.list.render(state);
    this.detail?.render(state);
  }

  getEditInputs(): BrowserEditInputs | null {
    return this.metadata.getEditInputs();
  }

  getPreviewElements(): {
    canvas: HTMLCanvasElement | null;
    container: HTMLDivElement | null;
    status: HTMLDivElement | null;
  } {
    return {
      canvas: this.elements.previewCanvas ?? null,
      container: this.elements.previewContainer ?? null,
      status: this.elements.previewStatus ?? null,
    };
  }

  isActive(): boolean {
    return Boolean(this.elements.dialog?.isConnected);
  }

  private getSelectedCube(): CubeLibraryEntry | null {
    return (
      this.state?.filtered.find((cube) => cube.cube_id?.trim() === this.state?.selected) || null
    );
  }
}
