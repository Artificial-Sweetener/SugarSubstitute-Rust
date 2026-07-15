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
 * Own the SugarCubes cube browser layer in `frontend/comfyui/ui/browser/CubeBrowserController.js`.
 */

import { CubeBrowserStore } from './CubeBrowserStore.js';
import { CubeBrowserView } from './CubeBrowserView.js';
import { CubePreviewRenderer } from './CubePreviewRenderer.js';
import { injectBrowserStyles } from './BrowserStyles.js';
import { deriveCubeIdFromDefaultAlias, normalizeDefaultAliasTitle } from '../core/CubeId.js';
import {
  deriveTargetModelCubeId,
  deriveTargetModelFromCubeId,
  normalizeSupportedModels,
  normalizeTargetModel,
} from '../core/ModelTargets.js';
import {
  CURRENT_REVISION_REF,
  formatCubeVersionLabel,
  isCurrentRevisionRef,
  normalizeCubeVersion,
  normalizeRevisionRef,
} from '../core/CubeDefinitionKey.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type {
  CubeAuthorGroup,
  CubeEditDraft,
  CubeLibraryEntry,
  CubeRevision,
  CubeVersionOption,
} from './CubeBrowserStore.js';

interface BrowserAdapter {
  getDocument?(): Document | null;
  getWindow?(): Window | null;
  getFetch?(): typeof fetch | null;
  getConsole?(): Console | null;
  getLiteGraph?(): LiteGraphHost | null;
}

export interface BrowserApi {
  list(options?: RequestInit): Promise<ApiJsonResult>;
  listRevisions(cubeId: string): Promise<ApiJsonResult>;
  delete(reference?: string | UnknownRecord | null): Promise<ApiJsonResult>;
  load(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  loadRevision(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  rename(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  updateMetadata(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}

interface BrowserStorage {
  readList(key: string): string[];
  writeList(key: string, values: readonly string[]): void;
  readSet(key: string): Set<string>;
  writeSet(key: string, values: ReadonlySet<string>): void;
}

interface BrowserToast {
  push(severity: string, summary: string, detail: string): void;
}

interface ImportResult extends UnknownRecord {
  success?: boolean;
}
interface BusyImportOptions {
  dropOrigin: Vec2;
  setBusy(busy: boolean): void;
}

export interface BrowserActions {
  computeDropOrigin?(): Vec2;
  emitProximityLog?(name: string, detail: UnknownRecord): void;
  onCubesUpdated?(cubes: CubeLibraryEntry[]): void;
  openConfirmDialog?(options: {
    title: string;
    message: string | string[];
    confirmLabel: string;
  }): Promise<boolean> | boolean;
  reconcileCubeIdentity?(options: {
    previousCubeId: string;
    cubeId: string;
    defaultAlias: string;
  }): void;
  importCubeByName?(cubeId: string, options: BusyImportOptions): Promise<ImportResult>;
  importCubeRevision?(
    cubeId: string,
    revisionRef: string,
    options: BusyImportOptions,
  ): Promise<ImportResult>;
  promoteCube?(cube: CubeLibraryEntry): Promise<unknown> | unknown;
  startCubePlacement?(
    cubeId: string,
    options: {
      closeBrowser: boolean;
      defaultAlias: string;
      revisionRef: string;
      version: string;
    },
  ): void;
}

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

interface VersionNormalization {
  options: CubeVersionOption[];
  error: string;
  warning: string;
}

interface VersionScore {
  category: number;
  distance: number;
  index: number;
}
interface SemverParts {
  major: number;
  minor: number;
  patch: number;
}
interface ModelEditor {
  input: HTMLInputElement;
}
interface CubePackGroup {
  key: string;
  label: string;
  authorLabel: string;
}

const unavailableApiCall = async (): Promise<ApiJsonResult> => {
  throw new Error('Cube library API unavailable');
};

const UNAVAILABLE_BROWSER_API: BrowserApi = {
  list: unavailableApiCall,
  listRevisions: unavailableApiCall,
  delete: unavailableApiCall,
  load: unavailableApiCall,
  loadRevision: unavailableApiCall,
  rename: unavailableApiCall,
  updateMetadata: unavailableApiCall,
};

function resolveBrowserApi(api: Partial<BrowserApi> | null | undefined): BrowserApi {
  return {
    list: api?.list?.bind(api) ?? UNAVAILABLE_BROWSER_API.list,
    listRevisions: api?.listRevisions?.bind(api) ?? UNAVAILABLE_BROWSER_API.listRevisions,
    delete: api?.delete?.bind(api) ?? UNAVAILABLE_BROWSER_API.delete,
    load: api?.load?.bind(api) ?? UNAVAILABLE_BROWSER_API.load,
    loadRevision: api?.loadRevision?.bind(api) ?? UNAVAILABLE_BROWSER_API.loadRevision,
    rename: api?.rename?.bind(api) ?? UNAVAILABLE_BROWSER_API.rename,
    updateMetadata: api?.updateMetadata?.bind(api) ?? UNAVAILABLE_BROWSER_API.updateMetadata,
  };
}

const CUBE_LIBRARY_FAVORITES_KEY = 'sugarcubes.favorites';
const CUBE_LIBRARY_RECENTS_KEY = 'sugarcubes.recent';
const CUBE_AUTHOR_GROUPS_KEY = 'sugarcubes.author_groups';
const CUBE_MODEL_LIST_URLS = Object.freeze([
  new URL('../../../models.txt', import.meta.url).toString(),
  '/extensions/ComfyUI-SugarCubes/models.txt',
]);
const FALLBACK_MODEL_OPTIONS = ['Other'];
const REFRESH_TTL_MS = 15000;

/**
 * Coordinate cube browser controller behavior for the SugarCubes UI.
 */
export class CubeBrowserController {
  private readonly adapter: BrowserAdapter | null;
  private readonly api: BrowserApi;
  private readonly storage: BrowserStorage | null;
  toast: BrowserToast | null;
  readonly store: CubeBrowserStore;
  readonly view: CubeBrowserView;
  private readonly preview: CubePreviewRenderer;
  private fetchController: AbortController | null;
  private refreshPromise: Promise<CubeLibraryEntry[]> | null;
  private triggerButton: { element?: HTMLElement } | null;
  private mount: HTMLElement | null;
  actions: BrowserActions;
  private initialized: boolean;
  private viewBuilt: boolean;
  private readonly authorIndex: Map<string, string>;
  private readonly cubeIdIndex: Map<string, CubeLibraryEntry>;

  constructor({
    adapter = null,
    api,
    storage = null,
    toast = null,
  }: BrowserControllerOptions = {}) {
    this.adapter = adapter;
    this.api = resolveBrowserApi(api);
    this.storage = storage;
    this.toast = toast;
    this.store = new CubeBrowserStore();
    this.view = new CubeBrowserView({ adapter });
    this.preview = new CubePreviewRenderer({ adapter });
    this.fetchController = null;
    this.refreshPromise = null;
    this.triggerButton = null;
    this.mount = null;
    this.actions = {};
    this.initialized = false;
    this.viewBuilt = false;
    this.authorIndex = new Map();
    this.cubeIdIndex = new Map();
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
    const key = typeof cubeId === 'string' ? cubeId.trim() : '';
    if (!key) {
      return null;
    }
    return this.cubeIdIndex.get(key) || null;
  }

  getCubeKey(cube: CubeLibraryEntry | null | undefined): string {
    const key = typeof cube?.cube_id === 'string' ? cube.cube_id.trim() : '';
    return key || '';
  }

  getCubeBySelectionKey(
    cubeKey: unknown,
    entries: readonly CubeLibraryEntry[] = this.store.state.filtered,
  ): CubeLibraryEntry | null {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    if (!key) {
      return null;
    }
    const cached = this.cubeIdIndex.get(key);
    if (cached) {
      return cached;
    }
    const list = Array.isArray(entries) ? entries : [];
    return (
      list.find((cube) => {
        if (this.getCubeKey(cube) === key) {
          return true;
        }
        const relativePath =
          typeof cube?.relative_path === 'string' ? cube.relative_path.trim() : '';
        return relativePath === key;
      }) || null
    );
  }

  isWritableCube(cube: CubeLibraryEntry | null | undefined): boolean {
    return Boolean(cube?.is_writable);
  }

  setDirtyCubeIds(dirtyCubeIds: Set<string> | null | undefined): void {
    this.store.setDirtyCubeIds(dirtyCubeIds);
    this.render();
  }

  setBusy(value: unknown): void {
    this.store.setBusy(Boolean(value));
    this.render();
  }

  async ensureInitialized(): Promise<void> {
    if (this.initialized) {
      return;
    }
    const favorites = this.readListStorage(CUBE_LIBRARY_FAVORITES_KEY);
    const recents = this.readListStorage(CUBE_LIBRARY_RECENTS_KEY);
    const storedGroups = this.readSetStorage(CUBE_AUTHOR_GROUPS_KEY);
    this.store.setFavorites(favorites);
    this.store.setRecents(recents);
    this.store.setModelOptions(FALLBACK_MODEL_OPTIONS.slice());
    this.store.setAuthorGroupsOpen(storedGroups);
    this.store.setDropOrigin([0, 0]);
    injectBrowserStyles(this.adapter?.getDocument?.());
    this.initialized = true;
    await this.loadSupportedModels().catch(() => {});
  }

  ensureView({ mount = null }: { mount?: HTMLElement | null } = {}): boolean {
    if (!this.viewBuilt) {
      if (!mount) {
        return false;
      }
      this.bindHandlers();
      this.view.build();
      this.view.mount(mount);
      this.viewBuilt = true;
      this.mount = mount || null;
      const previewElements = this.view.getPreviewElements();
      this.preview.attach(previewElements);
      this.store.setInitialized(true);
      try {
        this.actions.emitProximityLog?.('cube-browser-initialized', {
          favorites: this.store.state.favorites.size,
          recents: this.store.state.recents.length,
        });
      } catch (_error) {
        // ignore logging failures
      }
      return true;
    }
    if (mount && this.view.elements?.dialog) {
      if (mount !== this.mount || this.view.elements.dialog.parentElement !== mount) {
        mount.replaceChildren(this.view.elements.dialog);
      }
      this.mount = mount;
    }
    return true;
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
      onSelect: (cubeKey, options) => this.selectCube(cubeKey, options),
      onToggleAuthorGroup: (key) => this.toggleAuthorGroup(key),
      onImport: () => this.loadSelectedRevision(),
      onMoveSelection: (delta) => this.moveSelection(delta),
      onClearLineage: (selected) => this.clearLineage(selected),
      onVersionSelect: (version) => this.selectVersion(version),
      onVersionCommit: (version) => this.commitVersionInput(version),
      onVersionClosest: (version) =>
        this.findClosestVersionOption(version, this.store.state.versionOptions),
    });
  }

  mountEmbedded(container: HTMLElement | null | undefined): void {
    if (!container) {
      return;
    }
    void this.ensureInitialized().then(() => {
      if (!this.ensureView({ mount: container })) {
        return;
      }
      this.render();
      this.refresh({ force: false }).catch(() => {});
    });
  }

  open({ triggerButton }: { triggerButton?: { element?: HTMLElement } | null } = {}): boolean {
    const canOpen = this.viewBuilt;
    void this.ensureInitialized().then(() => {
      if (!this.ensureView()) {
        return;
      }
      this.triggerButton = triggerButton || null;
      this.store.setDropOrigin(this.actions.computeDropOrigin?.() || [0, 0]);
      this.view.scrollIntoView();
      this.view.focusSearch();
      const shouldRefresh =
        !this.store.state.cubes.length ||
        Date.now() - this.store.state.lastFetched > REFRESH_TTL_MS;
      if (shouldRefresh) {
        this.refresh({ force: true }).catch(() => {});
      } else {
        this.applyFilters();
        this.render();
      }
    });
    return canOpen;
  }

  openForEdit({ cubeId }: { cubeId?: string; defaultAlias?: string } = {}): boolean {
    void this.ensureInitialized().then(() => {
      if (!this.ensureView()) {
        return;
      }
      this.store.setDropOrigin(this.actions.computeDropOrigin?.() || [0, 0]);
      this.view.scrollIntoView();
      this.view.focusSearch();
      const shouldRefresh =
        !this.store.state.cubes.length ||
        Date.now() - this.store.state.lastFetched > REFRESH_TTL_MS;
      const finalizeSelection = () => {
        const selectedCube = this.getCubeBySelectionKey(cubeId, this.store.state.cubes);
        const selectionKey = this.getCubeKey(selectedCube);
        if (!selectionKey) {
          return;
        }
        this.selectCube(selectionKey, { focus: false, silent: true });
        const selected = this.getCubeBySelectionKey(selectionKey);
        if (selected) {
          this.store.setEditing(true, this.buildEditDraft(selected));
          this.render();
        }
      };
      if (shouldRefresh) {
        this.refresh({ force: true })
          .then(finalizeSelection)
          .catch(() => {});
      } else {
        this.applyFilters();
        this.render();
        finalizeSelection();
      }
    });
    return this.viewBuilt;
  }

  close(): void {
    if (this.triggerButton?.element) {
      this.triggerButton.element.focus?.();
    }
  }

  async refresh({ force = false }: { force?: boolean } = {}): Promise<CubeLibraryEntry[]> {
    if (this.refreshPromise) {
      return this.refreshPromise;
    }
    const refreshPromise = this.performRefresh({ force });
    this.refreshPromise = refreshPromise;
    try {
      return await refreshPromise;
    } finally {
      if (this.refreshPromise === refreshPromise) {
        this.refreshPromise = null;
      }
    }
  }

  private async performRefresh({ force }: { force: boolean }): Promise<CubeLibraryEntry[]> {
    await this.ensureInitialized();
    if (!force && this.store.state.cubes.length) {
      this.applyFilters();
      this.render();
      return this.store.state.cubes;
    }

    if (this.fetchController) {
      try {
        this.fetchController.abort();
      } catch (_error) {
        // ignore
      }
    }
    const controller = new AbortController();
    this.fetchController = controller;
    this.store.setLoading(true);
    this.store.setError(null);
    this.render();

    try {
      const { response, data } = await this.api.list({ signal: controller.signal });
      if (!response.ok) {
        const message = readApiErrorMessage(data, response.statusText || 'Failed to load cubes');
        this.store.setError(message);
        this.toast?.push('error', 'Cube library unavailable', message);
        this.store.setLoading(false);
        this.render();
        return this.store.state.cubes;
      }
      const cubes = Array.isArray(data.cubes)
        ? data.cubes
            .map((cube) => this.normalizeCatalogCube(cube))
            .filter((cube): cube is CubeLibraryEntry => cube !== null)
        : [];
      this.store.setCubes(cubes);
      this.cubeIdIndex.clear();
      for (const cube of cubes) {
        const key = this.getCubeKey(cube);
        if (key) {
          this.cubeIdIndex.set(key, cube);
        }
      }
      this.store.setLastFetched(Date.now());
      this.actions.onCubesUpdated?.(cubes);
      this.applyFilters();
      if (!this.store.state.selected && this.store.state.filtered.length) {
        this.selectCube(this.getCubeKey(this.store.state.filtered[0]), {
          focus: false,
          silent: true,
        });
      } else {
        this.render();
      }
      this.store.setLoading(false);
      this.render();
      return cubes;
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        return this.store.state.cubes;
      }
      const message = readErrorMessage(error);
      this.store.setError(message);
      this.toast?.push('error', 'Cube library unavailable', message);
      this.store.setLoading(false);
      this.render();
      return this.store.state.cubes;
    }
  }

  normalizeCatalogCube(cube: unknown): CubeLibraryEntry | null {
    if (!isRecord(cube)) {
      return null;
    }
    const targetModel = normalizeTargetModel(cube.target_model);
    return {
      ...cube,
      target_model: targetModel,
      supported_models: normalizeSupportedModels(cube.supported_models, {
        targetModel,
      }),
    };
  }

  applyFilters(): void {
    const query = (this.store.state.searchQuery || '').trim().toLowerCase();
    const filtered = this.store.state.cubes.filter((cube) => {
      if (!cube || typeof cube !== 'object') {
        return false;
      }
      if (!query) {
        return true;
      }
      const description = (cube.description || '').toLowerCase();
      const tags = Array.isArray(cube.tags) ? cube.tags.join(' ').toLowerCase() : '';
      const models = Array.isArray(cube.supported_models)
        ? cube.supported_models.join(' ').toLowerCase()
        : '';
      const targetModel = (cube.target_model || '').toLowerCase();
      const defaultAlias = (cube.default_alias || '').toLowerCase();
      const displayName = (cube.display_name || '').toLowerCase();
      const name = (cube.name || '').toLowerCase();
      const author = (cube.author || '').toLowerCase();
      const authorUrl = (cube.author_url || '').toLowerCase();
      const cubeId = (cube.cube_id || '').toLowerCase();
      const version = (cube.version || '').toLowerCase();
      const haystack = [
        defaultAlias,
        displayName,
        name,
        description,
        tags,
        targetModel,
        models,
        author,
        authorUrl,
        cubeId,
        version,
      ]
        .filter(Boolean)
        .join(' ');
      return haystack.includes(query);
    });
    const sorted = this.sortFavoritesFirst(filtered);
    const grouped = this.buildAuthorGroups(sorted);
    this.store.setFiltered(sorted);
    this.store.setGrouped(grouped);
    this.ensureAuthorGroupDefaults(grouped);
  }

  sortFavoritesFirst(list: readonly CubeLibraryEntry[] | null | undefined): CubeLibraryEntry[] {
    if (!Array.isArray(list) || !list.length) {
      return Array.isArray(list) ? list : [];
    }
    const favorites: CubeLibraryEntry[] = [];
    const rest: CubeLibraryEntry[] = [];
    for (const cube of list) {
      if (this.store.state.favorites.has(this.getCubeKey(cube))) {
        favorites.push(cube);
      } else {
        rest.push(cube);
      }
    }
    const compare = (left: CubeLibraryEntry, right: CubeLibraryEntry): number =>
      (
        [
          ['target_model', ''],
          ['default_alias', ''],
          ['display_name', ''],
          ['name', ''],
          ['cube_id', ''],
        ] as const
      ).reduce((result, [key, fallback]) => {
        if (result) {
          return result;
        }
        return String(left?.[key] || fallback).localeCompare(
          String(right?.[key] || fallback),
          undefined,
          {
            sensitivity: 'base',
          },
        );
      }, 0);
    favorites.sort(compare);
    rest.sort(compare);
    return favorites.concat(rest);
  }

  updateSearch(value: unknown): void {
    this.store.setSearchQuery(value || '');
    this.applyFilters();
    this.render();
  }

  toggleAuthorGroup(key: string): void {
    if (!key) {
      return;
    }
    const open = new Set(this.store.state.authorGroupsOpen);
    if (open.has(key)) {
      open.delete(key);
    } else {
      open.add(key);
    }
    this.store.setAuthorGroupsOpen(open);
    this.store.setAuthorGroupsTouched(true);
    this.writeSetStorage(CUBE_AUTHOR_GROUPS_KEY, open);
    this.render();
  }

  ensureAuthorGroupOpen(cubeKey: string | null): void {
    if (!cubeKey) {
      return;
    }
    const key = this.authorIndex.get(cubeKey);
    if (!key) {
      return;
    }
    const open = new Set(this.store.state.authorGroupsOpen);
    if (!open.has(key)) {
      open.add(key);
      this.store.setAuthorGroupsOpen(open);
      this.store.setAuthorGroupsTouched(true);
      this.writeSetStorage(CUBE_AUTHOR_GROUPS_KEY, open);
    }
  }

  ensureAuthorGroupDefaults(grouped: readonly CubeAuthorGroup[]): void {
    if (this.store.state.authorGroupsTouched) {
      return;
    }
    const keys = new Set(grouped.map((group) => group.key));
    if (!keys.size) {
      return;
    }
    this.store.setAuthorGroupsOpen(keys);
  }

  buildAuthorGroups(list: readonly CubeLibraryEntry[]): CubeAuthorGroup[] {
    const groups = new Map<string, CubeAuthorGroup>();
    this.authorIndex.clear();
    for (const cube of list) {
      if (!cube || typeof cube !== 'object') {
        continue;
      }
      const pack = deriveCubePackGroup(cube);
      const entry = groups.get(pack.key) || { ...pack, cubes: [] };
      entry.cubes.push(cube);
      groups.set(pack.key, entry);
      const cubeKey = this.getCubeKey(cube);
      if (cubeKey) {
        this.authorIndex.set(cubeKey, pack.key);
      }
    }
    return Array.from(groups.values()).sort((a, b) => a.label.localeCompare(b.label));
  }

  selectCube(cubeKey: unknown, options: { focus?: boolean; silent?: boolean } = {}): void {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    this.store.setSelected(key || null);
    this.store.setSelectedRevision(CURRENT_REVISION_REF);
    this.store.setRevisions([], null);
    this.store.setRevisionsLoading(false);
    this.store.resetVersionState();
    this.store.setEditing(false, null);
    this.view.editInputs = null;
    const selected = this.getCubeBySelectionKey(key);
    this.applyFallbackVersionOptions(selected);
    if (options.focus) {
      this.view.elements?.placeButton?.focus?.();
    }
    if (!options.silent) {
      this.rememberRecentCube(this.store.state.selected);
    }
    this.ensureAuthorGroupOpen(this.store.state.selected);
    this.applyFilters();
    this.render();
    void this.requestPreview(this.store.state.selected);
    this.requestRevisions(this.store.state.selected).catch(() => {});
  }

  async requestRevisions(cubeKey: unknown): Promise<void> {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    if (!key) {
      this.store.setRevisions([], null);
      this.store.setSelectedRevision(CURRENT_REVISION_REF);
      this.store.resetVersionState();
      this.render();
      return;
    }
    const selected = this.getCubeBySelectionKey(key);
    const cubeId = selected?.cube_id || '';
    if (!cubeId) {
      this.store.setRevisions([], null);
      this.store.setSelectedRevision(CURRENT_REVISION_REF);
      this.store.resetVersionState();
      this.render();
      return;
    }
    this.store.setRevisionsLoading(true);
    this.render();
    try {
      const { response, data } = await this.api.listRevisions(cubeId);
      if (!response.ok || hasApiError(data)) {
        const message = readApiErrorMessage(
          data,
          response.statusText || 'Failed to load revisions',
        );
        this.store.setRevisions([], cubeId);
        this.store.setSelectedRevision(CURRENT_REVISION_REF);
        if (this.isDuplicateVersionHistoryError(data)) {
          this.applyFallbackVersionOptions(selected);
          this.store.setVersionError(null);
          this.toast?.push(
            'warn',
            'Revision history normalized',
            'Duplicate historical versions were ignored; the current version remains available.',
          );
          return;
        }
        this.store.setVersionError(message);
        this.toast?.push('warn', 'Revision history unavailable', message);
        return;
      }
      const currentSelected = this.getCubeBySelectionKey(this.store.state.selected);
      if (currentSelected?.cube_id !== cubeId) {
        return;
      }
      const revisions = Array.isArray(data?.revisions) ? data.revisions : [];
      const normalized = this.normalizeVersionOptions(revisions, selected);
      if (normalized.error) {
        this.store.setRevisions([], cubeId);
        this.store.setVersionOptions([]);
        this.store.setVersionError(normalized.error);
        this.store.setSelectedRevision(CURRENT_REVISION_REF);
        this.toast?.push('warn', 'Revision history unavailable', normalized.error);
        return;
      }
      if (normalized.warning) {
        this.toast?.push('warn', 'Revision history normalized', normalized.warning);
      }
      this.store.setRevisions(revisions, cubeId);
      this.store.setVersionOptions(normalized.options);
      this.store.setVersionError(null);
      const activeRevision = normalized.options.some(
        (entry) => entry?.revisionRef === this.store.state.selectedRevision,
      )
        ? this.store.state.selectedRevision
        : CURRENT_REVISION_REF;
      this.store.setSelectedRevision(activeRevision);
      const activeOption =
        normalized.options.find((entry) => entry.revisionRef === activeRevision) ||
        normalized.options[0] ||
        null;
      if (activeOption) {
        this.store.setSelectedVersion(activeOption.value);
      }
    } catch (error) {
      const message = readErrorMessage(error);
      this.store.setRevisions([], cubeId);
      this.store.setSelectedRevision(CURRENT_REVISION_REF);
      this.store.setVersionError(message);
      this.toast?.push('warn', 'Revision history unavailable', message);
    } finally {
      this.store.setRevisionsLoading(false);
      this.render();
      void this.requestPreview(this.store.state.selected);
    }
  }

  selectRevision(revisionRef: unknown): void {
    const normalized = normalizeRevisionRef(revisionRef);
    this.store.setSelectedRevision(normalized);
    const option = this.store.state.versionOptions.find(
      (entry) => entry.revisionRef === normalized,
    );
    if (option) {
      this.store.setSelectedVersion(option.value);
    }
    this.render();
    void this.requestPreview(this.store.state.selected);
  }

  applyFallbackVersionOptions(selected: CubeLibraryEntry | null | undefined): void {
    const version = normalizeCubeVersion(selected?.version);
    if (!version) {
      return;
    }
    this.store.setVersionOptions([
      {
        label: formatCubeVersionLabel(version),
        value: version,
        revisionRef: CURRENT_REVISION_REF,
        current: true,
        raw: null,
      },
    ]);
    this.store.setSelectedVersion(version);
  }

  normalizeVersionOptions(
    revisions: readonly CubeRevision[] | null | undefined,
    selected: CubeLibraryEntry | null | undefined,
  ): VersionNormalization {
    const options: CubeVersionOption[] = [];
    const seen = new Map<string, string>();
    const duplicateVersions = new Set<string>();
    for (const entry of Array.isArray(revisions) ? revisions : []) {
      const version = normalizeCubeVersion(entry?.version);
      if (!version) {
        continue;
      }
      const revisionRef = normalizeRevisionRef(entry?.revision_ref);
      if (seen.has(version)) {
        duplicateVersions.add(version);
        continue;
      }
      seen.set(version, revisionRef);
      options.push({
        label: formatCubeVersionLabel(version),
        value: version,
        revisionRef,
        current: Boolean(entry?.current) || isCurrentRevisionRef(revisionRef),
        raw: entry,
      });
    }
    if (!options.length) {
      const version = normalizeCubeVersion(selected?.version);
      if (version) {
        options.push({
          label: formatCubeVersionLabel(version),
          value: version,
          revisionRef: CURRENT_REVISION_REF,
          current: true,
          raw: null,
        });
      }
    }
    const duplicateLabels = Array.from(duplicateVersions)
      .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
      .map((version) => formatCubeVersionLabel(version));
    const warning = duplicateLabels.length
      ? `Ignored duplicate revision entries for ${duplicateLabels.join(', ')}.`
      : '';
    return { options, error: '', warning };
  }

  isDuplicateVersionHistoryError(data: unknown): boolean {
    if (!isRecord(data) || !isRecord(data.error) || !isRecord(data.error.details)) {
      return false;
    }
    return (
      data.error.message === 'Cube history contains duplicate version entries' &&
      Array.isArray(data.error.details.duplicates)
    );
  }

  selectVersion(version: unknown): void {
    const normalized = normalizeCubeVersion(version);
    const option = this.store.state.versionOptions.find((entry) => entry.value === normalized);
    if (!option) {
      return;
    }
    this.selectRevision(option.revisionRef);
  }

  commitVersionInput(version: unknown): CubeVersionOption | null {
    const options = Array.isArray(this.store.state.versionOptions)
      ? this.store.state.versionOptions
      : [];
    if (!options.length) {
      return null;
    }
    const selectedFallback =
      options.find((entry) => entry.value === this.store.state.selectedVersion) ||
      options.find((entry) => entry.revisionRef === this.store.state.selectedRevision) ||
      options[0] ||
      null;
    const option = this.findClosestVersionOption(version, options) || selectedFallback;
    if (!option) {
      return null;
    }
    this.selectRevision(option.revisionRef);
    return option;
  }

  findClosestVersionOption(
    typedValue: unknown,
    options: readonly CubeVersionOption[] = [],
  ): CubeVersionOption | null {
    const list = Array.isArray(options) ? options : [];
    const scored = list
      .map((option, index) => ({
        option,
        score: this.scoreVersionOption(typedValue, option, index),
      }))
      .filter(
        (entry): entry is { option: CubeVersionOption; score: VersionScore } =>
          entry.score !== null,
      );
    if (!scored.length) {
      return null;
    }
    scored.sort((left, right) => {
      if (left.score.category !== right.score.category) {
        return left.score.category - right.score.category;
      }
      if (left.score.distance !== right.score.distance) {
        return left.score.distance - right.score.distance;
      }
      return left.score.index - right.score.index;
    });
    return scored[0]?.option ?? null;
  }

  scoreVersionOption(
    typedValue: unknown,
    option: CubeVersionOption,
    index: number,
  ): VersionScore | null {
    const typed = normalizeCubeVersion(typedValue);
    const value = normalizeCubeVersion(option?.value);
    if (!typed || !value) {
      return null;
    }
    const typedLower = typed.toLowerCase();
    const valueLower = value.toLowerCase();
    if (typedLower === valueLower) {
      return { category: 0, distance: 0, index };
    }
    if (valueLower.startsWith(typedLower)) {
      return { category: 1, distance: valueLower.length - typedLower.length, index };
    }
    const substringIndex = valueLower.indexOf(typedLower);
    if (substringIndex >= 0) {
      return {
        category: 2,
        distance: substringIndex + Math.max(0, valueLower.length - typedLower.length),
        index,
      };
    }
    const typedParts = this.parseSemverParts(typedLower);
    const valueParts = this.parseSemverParts(valueLower);
    if (!typedParts || !valueParts) {
      return null;
    }
    const distance =
      Math.abs(valueParts.major - typedParts.major) * 1000000 +
      Math.abs(valueParts.minor - typedParts.minor) * 1000 +
      Math.abs(valueParts.patch - typedParts.patch);
    return { category: 3, distance, index };
  }

  parseSemverParts(value: unknown): SemverParts | null {
    const text = normalizeCubeVersion(value);
    const match = /^(\d+)(?:\.(\d+))?(?:\.(\d+))?$/.exec(text);
    if (!match) {
      return null;
    }
    return {
      major: Number(match[1] || 0),
      minor: Number(match[2] || 0),
      patch: Number(match[3] || 0),
    };
  }

  render(): void {
    const selectedEntry = this.getCubeBySelectionKey(this.store.state.selected);
    const displayName =
      typeof selectedEntry?.display_name === 'string' && selectedEntry.display_name.trim()
        ? selectedEntry.display_name.trim()
        : selectedEntry?.name || null;
    this.preview.setContext({
      selected: displayName,
      selectedId: selectedEntry?.cube_id || null,
      busy: this.store.state.busy,
    });
    this.view.update(this.store.state);
    if (!selectedEntry) {
      this.preview.update({ name: null, payload: null, loading: false, error: null });
      return;
    }
    void this.requestPreview(this.getCubeKey(selectedEntry));
  }

  toggleFavorite(): void {
    const cubeKey = this.store.state.selected;
    if (!cubeKey) {
      return;
    }
    const favorites = new Set(this.store.state.favorites);
    if (favorites.has(cubeKey)) {
      favorites.delete(cubeKey);
    } else {
      favorites.add(cubeKey);
    }
    this.store.setFavorites(favorites);
    this.writeListStorage(CUBE_LIBRARY_FAVORITES_KEY, Array.from(favorites));
    this.applyFilters();
    this.render();
  }

  toggleEdit(): void {
    if (this.store.state.editing) {
      this.cancelEdit();
      return;
    }
    const selected = this.getCubeBySelectionKey(this.store.state.selected);
    if (!selected) {
      return;
    }
    if (!this.isWritableCube(selected)) {
      const reason =
        typeof selected?.write_block_reason === 'string' && selected.write_block_reason.trim()
          ? selected.write_block_reason.trim()
          : 'This cube is read-only.';
      this.toast?.push('warn', 'Read-only cube', reason);
      return;
    }
    this.store.setEditing(true, this.buildEditDraft(selected));
    this.view.editInputs = null;
    this.render();
  }

  cancelEdit(): void {
    this.store.setEditing(false, null);
    this.view.editInputs = null;
    this.render();
  }

  async saveEdit(): Promise<void> {
    const selected = this.getCubeBySelectionKey(this.store.state.selected);
    const inputs = this.view.getEditInputs();
    if (!selected || !inputs) {
      return;
    }
    const newName = normalizeDefaultAliasTitle(
      String(inputs.name?.value || '')
        .split('/')
        .pop(),
    );
    const originalEditableName =
      typeof this.store.state.editDraft?.original_name === 'string' &&
      this.store.state.editDraft.original_name.trim()
        ? normalizeDefaultAliasTitle(this.store.state.editDraft.original_name.split('/').pop())
        : normalizeDefaultAliasTitle(this.resolveEditableCubeTitle(selected).split('/').pop());
    const description = inputs.description?.value ?? '';
    const version = inputs.version?.value?.trim() || '';
    const selectedTargetModel =
      normalizeTargetModel(inputs.target_model?.value) ||
      this.deriveTargetModelFromCubeIdSafe(selected.cube_id);
    const routeAlias = selectedTargetModel
      ? `${selectedTargetModel}/${newName || originalEditableName}`
      : newName || originalEditableName;
    const metadata: UnknownRecord = {
      default_alias: routeAlias,
      author_url: inputs.author_url?.value ?? '',
      tags: this.parseCommaList(inputs.tags?.value ?? ''),
      supported_models: normalizeSupportedModels(
        this.getSupportedModelValues(inputs.supported_models),
        { targetModel: selectedTargetModel },
      ),
    };
    if (selectedTargetModel) {
      metadata.target_model = selectedTargetModel;
    }
    let activeCubeId = selected.cube_id || '';
    this.store.setBusy(true);
    this.render();
    try {
      const targetCubeId = selectedTargetModel
        ? deriveTargetModelCubeId({
            sourceCubeId: selected.cube_id || '',
            targetModel: selectedTargetModel,
            defaultAlias: newName || originalEditableName,
          })
        : deriveCubeIdFromDefaultAlias(selected.cube_id || '', newName || originalEditableName);
      if (targetCubeId && targetCubeId !== selected.cube_id) {
        const confirmed = await this.actions.openConfirmDialog?.({
          title: 'Rename SugarCube?',
          message: [
            'Changing the default alias renames this cube and updates its canonical ID.',
            `Version ${version || selected.version || 'history'} and local flavors will be preserved.`,
          ],
          confirmLabel: 'Rename',
        });
        if (!confirmed) {
          return;
        }
        const { response: renameResponse, data: renameData } = await this.api.rename(
          JSON.stringify({
            cube_id: selected.cube_id || '',
            default_alias: routeAlias,
            target_cube_id: targetCubeId,
          }),
          { headers: { 'Content-Type': 'application/json' } },
        );
        if (!renameResponse.ok || hasApiError(renameData)) {
          const message = readApiErrorMessage(
            renameData,
            renameResponse.statusText || 'Rename failed',
          );
          this.toast?.push('error', 'Rename failed', message);
          return;
        }
        const renamedCube = isRecord(renameData.cube) ? renameData.cube : null;
        activeCubeId =
          (typeof renamedCube?.cube_id === 'string' ? renamedCube.cube_id : '') || targetCubeId;
        this.actions.reconcileCubeIdentity?.({
          previousCubeId: selected.cube_id || '',
          cubeId: activeCubeId,
          defaultAlias:
            (typeof renamedCube?.default_alias === 'string' ? renamedCube.default_alias : '') ||
            routeAlias,
        });
      }
      const payload: UnknownRecord = {
        cube_id: activeCubeId,
        description,
        metadata,
      };
      if (version) {
        payload.version = version;
      }
      const { response, data } = await this.api.updateMetadata(JSON.stringify(payload), {
        headers: { 'Content-Type': 'application/json' },
      });
      if (!response.ok || hasApiError(data)) {
        const message = readApiErrorMessage(data, response.statusText || 'Update failed');
        this.toast?.push('error', 'Metadata update failed', message);
        return;
      }
      this.store.setEditing(false, null);
      this.view.editInputs = null;
      await this.refresh({ force: true });
      const refreshedCube = this.getCubeBySelectionKey(activeCubeId, this.store.state.cubes);
      this.selectCube(this.getCubeKey(refreshedCube) || activeCubeId, {
        focus: false,
        silent: true,
      });
    } catch (error) {
      const message = readErrorMessage(error);
      this.toast?.push('error', 'Metadata update failed', message);
    } finally {
      this.store.setBusy(false);
      this.render();
    }
  }

  async clearLineage(selected: CubeLibraryEntry | null | undefined): Promise<void> {
    if (!selected?.cube_id) {
      return;
    }
    if (!this.isWritableCube(selected)) {
      this.toast?.push(
        'warn',
        'Read-only cube',
        selected?.write_block_reason || 'This cube is read-only.',
      );
      return;
    }
    const confirmed = await this.actions.openConfirmDialog?.({
      title: 'Clear lineage?',
      message: 'This removes the fork lineage metadata from this cube.',
      confirmLabel: 'Clear',
    });
    if (!confirmed) {
      return;
    }
    this.store.setBusy(true);
    this.render();
    try {
      const { response, data } = await this.api.updateMetadata(
        JSON.stringify({
          cube_id: selected.cube_id,
          metadata: { lineage: null },
        }),
        { headers: { 'Content-Type': 'application/json' } },
      );
      if (!response.ok || hasApiError(data)) {
        const message = readApiErrorMessage(data, response.statusText || 'Update failed');
        this.toast?.push('error', 'Lineage update failed', message);
        return;
      }
      await this.refresh({ force: true });
      this.selectCube(selected.cube_id, { focus: false, silent: true });
    } catch (error) {
      const message = readErrorMessage(error);
      this.toast?.push('error', 'Lineage update failed', message);
    } finally {
      this.store.setBusy(false);
      this.render();
    }
  }

  async requestDelete(): Promise<void> {
    const cubeKey = this.store.state.selected || '';
    if (!cubeKey) {
      this.toast?.push('warn', 'Select a cube', 'Choose a cube before deleting.');
      return;
    }
    const selected = this.getCubeBySelectionKey(cubeKey);
    const cubeId = selected?.cube_id || '';
    if (selected && !this.isWritableCube(selected)) {
      this.toast?.push(
        'warn',
        'Read-only cube',
        selected.write_block_reason || 'This cube is read-only.',
      );
      return;
    }
    const deleteReference = cubeId ? { cube_id: cubeId } : null;
    if (!deleteReference) {
      this.toast?.push('error', 'Delete failed', 'Cube id missing.');
      return;
    }
    const displayName = selected?.display_name || selected?.name || cubeKey;
    const confirmed = await this.actions.openConfirmDialog?.({
      title: 'Delete SugarCube?',
      message: [`Delete SugarCube "${displayName}"?`, 'This cannot be undone.'],
      confirmLabel: 'Delete',
    });
    if (!confirmed) {
      return;
    }
    this.store.setBusy(true);
    this.render();
    try {
      const { response, data } = await this.api.delete(deleteReference);
      if (!response.ok || hasApiError(data)) {
        const message = readApiErrorMessage(data, response.statusText || 'Failed to delete cube');
        this.toast?.push('error', 'Delete failed', message);
        return;
      }
      this.removeCubeFromLists(cubeKey);
      this.toast?.push('success', 'Cube deleted', `Removed ${displayName}.`);
      this.refresh({ force: true }).catch(() => {});
    } catch (error) {
      const message = readErrorMessage(error);
      this.toast?.push('error', 'Delete failed', message);
    } finally {
      this.store.setBusy(false);
      this.render();
    }
  }

  async requestPromotion(): Promise<void> {
    const selected = this.getCubeBySelectionKey(this.store.state.selected);
    if (!selected) {
      this.toast?.push('warn', 'Select a cube', 'Choose a personal cube before moving it.');
      return;
    }
    this.store.setBusy(true);
    this.render();
    try {
      await this.actions.promoteCube?.(selected);
    } finally {
      this.store.setBusy(false);
      this.render();
    }
  }

  removeCubeFromLists(cubeKey: unknown): void {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    if (!key) {
      return;
    }
    const cubes = this.store.state.cubes.filter((cube) => this.getCubeKey(cube) !== key);
    const filtered = this.store.state.filtered.filter((cube) => this.getCubeKey(cube) !== key);
    const favorites = new Set(this.store.state.favorites);
    const recents = this.store.state.recents.filter((entry) => entry !== key);
    this.store.setCubes(cubes);
    this.store.setFiltered(filtered);
    if (this.store.state.selected === key) {
      this.store.setSelected(this.getCubeKey(filtered[0]) || null);
    }
    if (favorites.delete(key)) {
      this.writeListStorage(CUBE_LIBRARY_FAVORITES_KEY, Array.from(favorites));
    }
    if (recents.length !== this.store.state.recents.length) {
      this.store.setRecents(recents);
      this.writeListStorage(CUBE_LIBRARY_RECENTS_KEY, recents);
    }
    this.render();
  }

  importCube(): void {
    const cubeKey = this.store.state.selected;
    if (!cubeKey) {
      this.toast?.push('warn', 'Select a cube', 'Choose a cube from the list before importing.');
      return;
    }
    const selected = this.getCubeBySelectionKey(cubeKey);
    const cubeId = selected?.cube_id || '';
    if (!cubeId) {
      this.toast?.push('error', 'Import failed', 'Cube id missing.');
      return;
    }
    this.store.setBusy(true);
    this.render();
    void this.actions
      .importCubeByName?.(cubeId, {
        dropOrigin: this.store.state.dropOrigin,
        setBusy: (busy) => {
          this.store.setBusy(busy);
          if (!busy) {
            this.store.setLoading(false);
          }
          this.render();
        },
      })
      ?.then((result) => {
        if (result?.success) {
          this.close();
        }
      })
      ?.finally(() => {
        this.store.setBusy(false);
        this.render();
      });
  }

  async loadSelectedRevision(): Promise<void> {
    const cubeKey = this.store.state.selected;
    if (!cubeKey) {
      this.toast?.push('warn', 'Select a cube', 'Choose a cube before loading a revision.');
      return;
    }
    const selected = this.getCubeBySelectionKey(cubeKey);
    const cubeId = selected?.cube_id || '';
    const revisionRef = this.store.state.selectedRevision || CURRENT_REVISION_REF;
    if (!cubeId) {
      this.toast?.push('error', 'Load failed', 'Cube id missing.');
      return;
    }
    if (isCurrentRevisionRef(revisionRef)) {
      this.importCube();
      return;
    }
    this.store.setBusy(true);
    this.render();
    try {
      const result = await this.actions.importCubeRevision?.(cubeId, revisionRef, {
        dropOrigin: this.store.state.dropOrigin,
        setBusy: (busy) => {
          this.store.setBusy(busy);
          this.render();
        },
      });
      if (!result?.success) {
        return;
      }
      this.close();
    } finally {
      this.store.setBusy(false);
      this.render();
    }
  }

  placeCube(): void {
    if (!this.store.state.selected) {
      this.toast?.push('warn', 'Select a cube', 'Choose a cube from the list before placing.');
      return;
    }
    const cubeKey = this.store.state.selected;
    const selected = this.getCubeBySelectionKey(cubeKey);
    const cubeId = selected?.cube_id || '';
    const displayName =
      typeof selected?.display_name === 'string' && selected.display_name.trim()
        ? selected.display_name.trim()
        : selected?.name || cubeKey;
    if (!cubeId) {
      this.toast?.push('error', 'Placement failed', 'Cube id missing.');
      return;
    }
    const versionOption = this.getSelectedVersionOption();
    this.actions.startCubePlacement?.(cubeId, {
      closeBrowser: false,
      defaultAlias: displayName,
      revisionRef: versionOption?.revisionRef || CURRENT_REVISION_REF,
      version: versionOption?.value || selected?.version || '',
    });
  }

  getSelectedVersionOption(): CubeVersionOption | null {
    return (
      this.store.state.versionOptions.find(
        (entry) => entry.revisionRef === this.store.state.selectedRevision,
      ) ||
      this.store.state.versionOptions.find(
        (entry) => entry.value === this.store.state.selectedVersion,
      ) ||
      null
    );
  }

  moveSelection(delta: number): void {
    if (!this.store.state.filtered.length) {
      return;
    }
    const currentIndex = this.store.state.filtered.findIndex(
      (cube) => this.getCubeKey(cube) === this.store.state.selected,
    );
    let nextIndex = currentIndex + delta;
    if (nextIndex < 0) {
      nextIndex = this.store.state.filtered.length - 1;
    } else if (nextIndex >= this.store.state.filtered.length) {
      nextIndex = 0;
    }
    const nextCube = this.store.state.filtered[nextIndex];
    if (nextCube) {
      this.selectCube(this.getCubeKey(nextCube), { focus: false, silent: true });
    }
  }

  rememberRecentCube(cubeKey: string | null): void {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    if (!key) {
      return;
    }
    const recents = this.store.state.recents.filter((entry) => entry !== key);
    recents.unshift(key);
    if (recents.length > 12) {
      recents.length = 12;
    }
    this.store.setRecents(recents);
    this.writeListStorage(CUBE_LIBRARY_RECENTS_KEY, recents);
  }

  async requestPreview(cubeKey: string | null): Promise<void> {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    if (!key) {
      this.preview.update({
        name: null,
        requestKey: null,
        payload: null,
        loading: false,
        error: null,
      });
      return;
    }
    const selected = this.getCubeBySelectionKey(key);
    const cubeId = selected?.cube_id || '';
    const revisionRef =
      typeof this.store.state.selectedRevision === 'string' && this.store.state.selectedRevision
        ? this.store.state.selectedRevision
        : 'WORKTREE';
    const previewKey = `${key}::${revisionRef}`;
    const previewState = this.preview.getRequestState(previewKey);
    if (previewState === 'loading') {
      return;
    }
    if (previewState === 'ready') {
      this.preview.render();
      return;
    }
    if (!cubeId) {
      this.preview.update({
        name: key,
        requestKey: previewKey,
        payload: null,
        loading: false,
        error: 'Cube id missing.',
      });
      return;
    }
    const requestId = (this.preview.requestId || 0) + 1;
    this.preview.requestId = requestId;
    this.preview.update({
      name: key,
      requestKey: previewKey,
      payload: null,
      loading: true,
      error: null,
    });
    try {
      const requestBody = isCurrentRevisionRef(revisionRef)
        ? JSON.stringify({ cube_id: cubeId, origin: { x: 0, y: 0 } })
        : JSON.stringify({
            cube_id: cubeId,
            revision_ref: revisionRef,
            origin: { x: 0, y: 0 },
          });
      const loader = isCurrentRevisionRef(revisionRef)
        ? this.api.load.bind(this.api)
        : this.api.loadRevision.bind(this.api);
      const { response, data } = await loader(requestBody, {
        headers: { 'Content-Type': 'application/json' },
      });
      if (requestId !== this.preview.requestId) {
        return;
      }
      if (!response.ok || hasApiError(data)) {
        const message = readApiErrorMessage(data, response.statusText || 'Preview failed');
        this.preview.update({
          name: key,
          requestKey: previewKey,
          payload: null,
          loading: false,
          error: message,
        });
        return;
      }
      this.preview.update({
        name: key,
        requestKey: previewKey,
        payload: data,
        loading: false,
        error: null,
      });
    } catch (error) {
      if (requestId !== this.preview.requestId) {
        return;
      }
      const message = readErrorMessage(error);
      this.preview.update({
        name: key,
        requestKey: previewKey,
        payload: null,
        loading: false,
        error: message,
      });
    }
  }

  buildEditDraft(selected: CubeLibraryEntry): CubeEditDraft {
    const tags = Array.isArray(selected.tags) ? selected.tags : [];
    const supportedModels = Array.isArray(selected.supported_models)
      ? selected.supported_models
      : [];
    const editableName = this.resolveEditableCubeTitle(selected);
    const targetModel =
      normalizeTargetModel(selected.target_model) ||
      this.deriveTargetModelFromCubeIdSafe(selected.cube_id);
    let derivedCubeId = selected.cube_id || '';
    try {
      derivedCubeId = targetModel
        ? deriveTargetModelCubeId({
            sourceCubeId: selected.cube_id || '',
            targetModel,
            defaultAlias: normalizeDefaultAliasTitle(editableName) || editableName,
          })
        : deriveCubeIdFromDefaultAlias(
            selected.cube_id || '',
            normalizeDefaultAliasTitle(editableName) || editableName,
          );
    } catch (_error) {
      derivedCubeId = selected.cube_id || '';
    }
    return {
      name: editableName,
      original_name: editableName,
      description: selected.description || '',
      current_cube_id: selected.cube_id || '',
      derived_cube_id: derivedCubeId,
      cube_id: selected.cube_id || '',
      version: selected.version || '',
      author_url: selected.author_url || '',
      tags: tags.slice(),
      target_model: targetModel,
      supported_models: supportedModels.slice(),
    };
  }

  deriveTargetModelFromCubeIdSafe(cubeId: unknown): string {
    try {
      return normalizeTargetModel(deriveTargetModelFromCubeId(cubeId));
    } catch (_error) {
      return '';
    }
  }

  parseCommaList(value: unknown): string[] {
    if (typeof value !== 'string') {
      return [];
    }
    return value
      .split(',')
      .map((entry) => entry.trim())
      .filter(Boolean);
  }

  getSupportedModelValues(editor: ModelEditor | null | undefined): string[] {
    if (!editor?.input || typeof editor.input.value !== 'string') {
      return [];
    }
    return this.parseCommaList(editor.input.value);
  }

  resolveEditableCubeTitle(selected: CubeLibraryEntry | null | undefined): string {
    if (!selected || typeof selected !== 'object') {
      return '';
    }
    if (typeof selected.default_alias === 'string' && selected.default_alias.trim()) {
      return selected.default_alias.trim();
    }
    if (
      typeof selected.metadata?.default_alias === 'string' &&
      selected.metadata.default_alias.trim()
    ) {
      return selected.metadata.default_alias.trim();
    }
    if (typeof selected.name === 'string' && selected.name.trim()) {
      return selected.name.trim();
    }
    return '';
  }

  async loadSupportedModels(): Promise<void> {
    try {
      const fetchRef = this.adapter?.getFetch?.();
      if (!fetchRef) {
        return;
      }
      const text = await this.fetchSupportedModelList(fetchRef);
      if (!text) {
        return;
      }
      const options = normalizeSupportedModels(
        text
          .split(/\r?\n/)
          .map((line) => line.trim())
          .filter(Boolean),
      );
      if (!options.length) {
        return;
      }
      this.store.setModelOptions(options);
      this.render();
    } catch (_error) {
      // ignore model list fetch failures
    }
  }

  async fetchSupportedModelList(fetchRef: typeof fetch): Promise<string> {
    for (const url of CUBE_MODEL_LIST_URLS) {
      const response = await fetchRef(url);
      if (!response?.ok) {
        continue;
      }
      return response.text();
    }
    return '';
  }

  readListStorage(key: string): string[] {
    try {
      if (!this.storage) {
        return [];
      }
      return this.storage.readList(key);
    } catch (_error) {
      return [];
    }
  }

  writeListStorage(key: string, values: readonly string[]): void {
    try {
      if (!this.storage) {
        return;
      }
      this.storage.writeList(key, values);
    } catch (_error) {
      // ignore storage failures
    }
  }

  readSetStorage(key: string): Set<string> {
    try {
      if (!this.storage) {
        return new Set();
      }
      return this.storage.readSet(key);
    } catch (_error) {
      return new Set();
    }
  }

  writeSetStorage(key: string, setValue: ReadonlySet<string>): void {
    try {
      if (!this.storage) {
        return;
      }
      this.storage.writeSet(key, setValue);
    } catch (_error) {
      // ignore storage failures
    }
  }
}

/**
 * Return the browser list group label with pack name ahead of author.
 */
function deriveCubePackGroup(cube: CubeLibraryEntry): CubePackGroup {
  const source = isRecord(cube.source) ? cube.source : {};
  const sourceType = normalizeText(source.type);
  const owner = normalizeText(cube?.owner) || normalizeText(source.owner);
  const repo = normalizeText(cube?.repo) || normalizeText(source.repo);
  const namespace = normalizeText(cube?.namespace) || normalizeText(source.namespace);
  const repoRef = normalizeText(source.repo_ref);
  if (repo && owner) {
    return {
      key: `github:${owner.toLowerCase()}/${repo.toLowerCase()}`,
      label: repo,
      authorLabel: owner,
    };
  }
  if (repoRef.includes('/')) {
    const [repoOwner, repoName] = repoRef.split('/', 2).map((part) => part.trim());
    if (repoOwner && repoName) {
      return {
        key: `github:${repoOwner.toLowerCase()}/${repoName.toLowerCase()}`,
        label: repoName,
        authorLabel: repoOwner,
      };
    }
  }
  if (sourceType === 'local' || namespace) {
    const localAuthor = namespace || 'local';
    return {
      key: `local:${localAuthor.toLowerCase()}`,
      label: 'local',
      authorLabel: namespace,
    };
  }
  return deriveLegacyAuthorPackGroup(cube);
}

/**
 * Preserve useful grouping for older payloads that only expose author text.
 */
function deriveLegacyAuthorPackGroup(cube: CubeLibraryEntry): CubePackGroup {
  const author = normalizeText(cube?.author);
  if (author.includes('/')) {
    const [owner, repo] = author.split('/', 2).map((part) => part.trim());
    if (owner && repo) {
      return {
        key: `legacy:${owner.toLowerCase()}/${repo.toLowerCase()}`,
        label: repo,
        authorLabel: owner,
      };
    }
  }
  const label = author || 'Unknown';
  return {
    key: `legacy:${label.toLowerCase()}`,
    label,
    authorLabel: '',
  };
}

/**
 * Normalize optional payload text before deriving browser group labels.
 */
function normalizeText(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Return whether an API payload contains a structured error. */
function hasApiError(data: UnknownRecord): boolean {
  return isRecord(data.error);
}

/** Read an API error message without trusting the response payload shape. */
function readApiErrorMessage(data: UnknownRecord, fallback: string): string {
  const error = isRecord(data.error) ? data.error : null;
  return (typeof error?.message === 'string' && error.message.trim()) || fallback;
}

/** Normalize an unknown thrown value into an actionable message. */
function readErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
