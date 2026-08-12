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
import {
  resolveBrowserApi,
  type BrowserActions,
  type BrowserAdapter,
  type BrowserApi,
  type BrowserStorage,
  type BrowserToast,
} from './CubeBrowserContracts.js';
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
import type { CubeLibraryEntry } from './CubeBrowserStore.js';

export type { BrowserApi } from './CubeBrowserContracts.js';
export type { BrowserActions } from './CubeBrowserContracts.js';

interface BrowserControllerOptions {
  adapter?: BrowserAdapter | null;
  api?: Partial<BrowserApi> | null;
  storage?: BrowserStorage | null;
  toast?: BrowserToast | null;
  events?: unknown;
  scheduler?: unknown;
}

interface BrowserConfigureOptions {
  actions?: BrowserActions;
  helpers?: Parameters<CubePreviewRenderer['setHelpers']>[0];
  placement?: Parameters<CubePreviewRenderer['setPlacementActions']>[0];
}

/** Coordinate Cube browser lifecycle and delegate each use case to its authoritative owner. */
export class CubeBrowserController {
  private readonly adapter: BrowserAdapter | null;
  toast: BrowserToast | null;
  readonly store = new CubeBrowserStore();
  readonly view: CubeBrowserView;
  private readonly preview: CubePreviewRenderer;
  private readonly preferences: CubeBrowserPreferences;
  private readonly catalog: CubeBrowserCatalog;
  private readonly previewCoordinator: CubeBrowserPreviewCoordinator;
  private readonly revisions: CubeBrowserRevisionCoordinator;
  private readonly editor: CubeBrowserMetadataEditor;
  private readonly mutations: CubeBrowserMutationCommands;
  private readonly placement: CubeBrowserPlacementCommands;
  private readonly viewHost: CubeBrowserViewHost;
  actions: BrowserActions = {};
  private initialized = false;

  constructor({
    adapter = null,
    api,
    storage = null,
    toast = null,
  }: BrowserControllerOptions = {}) {
    this.adapter = adapter;
    this.toast = toast;
    const toastRelay: BrowserToast = {
      push: (severity, summary, detail) => this.toast?.push(severity, summary, detail),
    };
    const resolvedApi = resolveBrowserApi(api);
    this.view = new CubeBrowserView({ adapter });
    this.preview = new CubePreviewRenderer({ adapter });
    this.preferences = new CubeBrowserPreferences({ store: this.store, storage });
    const getCube = (key: unknown, entries?: readonly CubeLibraryEntry[]) =>
      this.catalog.getCubeBySelectionKey(key, entries);
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

  configure({ actions = {}, helpers = {}, placement = {} }: BrowserConfigureOptions = {}): void {
    this.actions = actions;
    this.preview.setHelpers(helpers);
    this.preview.setPlacementActions(placement);
  }

  async setup(): Promise<void> {
    await this.ensureInitialized();
  }

  dispose(): void {
    this.preview.dispose();
  }

  getCubes(): CubeLibraryEntry[] {
    return this.store.state.cubes;
  }

  getCubeById(cubeId: unknown): CubeLibraryEntry | null {
    return this.catalog.getCubeById(cubeId);
  }

  getCubeKey(cube: CubeLibraryEntry | null | undefined): string {
    return this.catalog.getCubeKey(cube);
  }

  getCubeBySelectionKey(
    cubeKey: unknown,
    entries: readonly CubeLibraryEntry[] = this.store.state.filtered,
  ): CubeLibraryEntry | null {
    return this.catalog.getCubeBySelectionKey(cubeKey, entries);
  }

  isWritableCube(cube: CubeLibraryEntry | null | undefined): boolean {
    return Boolean(cube?.is_writable);
  }

  getModelSuggestions(): readonly string[] {
    return this.store.state.modelOptions.slice();
  }

  setDirtyCubeIds(ids: Set<string> | null | undefined): void {
    this.store.setDirtyCubeIds(ids);
    this.render();
  }

  setBusy(value: unknown): void {
    this.store.setBusy(Boolean(value));
    this.render();
  }

  async ensureInitialized(): Promise<void> {
    if (this.initialized) return;
    this.store.setDropOrigin([0, 0]);
    injectBrowserStyles(this.adapter?.getDocument?.());
    this.initialized = true;
    await this.catalog.initialize();
  }

  ensureView({ mount = null }: { mount?: HTMLElement | null } = {}): boolean {
    return this.viewHost.ensureView(mount);
  }

  bindHandlers(): void {
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
      onVersionClosest: (version) =>
        this.revisions.findClosestVersionOption(version, this.store.state.versionOptions),
    });
  }

  mountEmbedded(container: HTMLElement | null | undefined): void {
    this.viewHost.mountEmbedded(container);
  }

  open({ triggerButton }: { triggerButton?: { element?: HTMLElement } | null } = {}): boolean {
    return this.viewHost.open(triggerButton || null);
  }

  openForEdit({ cubeId }: { cubeId?: string; defaultAlias?: string } = {}): boolean {
    return this.viewHost.openForEdit(cubeId);
  }

  close(): void {
    this.viewHost.close();
  }

  async refresh({ force = false }: { force?: boolean } = {}): Promise<CubeLibraryEntry[]> {
    await this.ensureInitialized();
    return this.catalog.refresh(force);
  }

  applyFilters(): void {
    this.catalog.applyFilters();
  }

  updateSearch(value: unknown): void {
    this.store.setSearchQuery(value || '');
    this.applyFilters();
    this.render();
  }

  toggleAuthorGroup(key: string): void {
    if (this.preferences.toggleAuthorGroup(key)) this.render();
  }

  selectCube(cubeKey: unknown, options: { focus?: boolean; silent?: boolean } = {}): void {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    this.store.setSelected(key || null);
    this.revisions.selectCube(this.getCubeBySelectionKey(key));
    this.store.setEditing(false, null);
    this.view.editInputs = null;
    if (options.focus) this.view.elements?.placeButton?.focus?.();
    if (!options.silent) this.preferences.rememberRecent(this.store.state.selected);
    this.preferences.ensureAuthorGroupOpen(
      this.catalog.getAuthorGroupKey(this.store.state.selected),
    );
    this.applyFilters();
    this.render();
    void this.previewCoordinator.request(this.store.state.selected);
    void this.revisions.requestRevisions(this.store.state.selected).catch(() => {});
  }

  render(): void {
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
    } else {
      void this.previewCoordinator.request(this.getCubeKey(selected));
    }
  }

  toggleFavorite(): void {
    if (this.preferences.toggleFavorite(this.store.state.selected)) {
      this.applyFilters();
      this.render();
    }
  }

  toggleEdit(): void {
    this.editor.toggle();
  }

  cancelEdit(): void {
    this.editor.cancel();
  }

  async saveEdit(): Promise<void> {
    await this.editor.save();
  }

  async clearLineage(selected: CubeLibraryEntry | null | undefined): Promise<void> {
    await this.mutations.clearLineage(selected);
  }

  async requestDelete(): Promise<void> {
    await this.mutations.requestDelete();
  }

  async requestPromotion(): Promise<void> {
    await this.mutations.requestPromotion();
  }

  removeCubeFromLists(cubeKey: unknown): void {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    if (!key) return;
    this.catalog.removeCube(key);
    this.preferences.forgetCube(key);
    this.render();
  }

  importCube(): void {
    this.placement.importCube();
  }

  async loadSelectedRevision(): Promise<void> {
    await this.placement.loadSelectedRevision();
  }

  placeCube(): void {
    this.placement.placeCube();
  }

  moveSelection(delta: number): void {
    const list = this.store.state.filtered;
    if (!list.length) return;
    const current = list.findIndex((cube) => this.getCubeKey(cube) === this.store.state.selected);
    const next = (current + delta + list.length) % list.length;
    const cube = list[next];
    if (cube) this.selectCube(this.getCubeKey(cube), { focus: false, silent: true });
  }
}
