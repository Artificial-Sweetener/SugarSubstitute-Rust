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
/** Own Cube browser mounting, focus, and host-open lifecycle. */

import type { BrowserActions } from './CubeBrowserContracts.js';
import type { CubeBrowserStore, CubeLibraryEntry } from './CubeBrowserStore.js';
import type { CubeBrowserView } from './CubeBrowserView.js';
import type { CubePreviewRenderer } from './CubePreviewRenderer.js';

const REFRESH_TTL_MS = 15000;

interface ViewHostOptions {
  store: CubeBrowserStore;
  view: CubeBrowserView;
  preview: CubePreviewRenderer;
  getActions(): BrowserActions;
  initialize(): Promise<void>;
  bindHandlers(): void;
  refresh(force: boolean): Promise<CubeLibraryEntry[]>;
  applyFilters(): void;
  render(): void;
  getCube(cubeKey: unknown, entries?: readonly CubeLibraryEntry[]): CubeLibraryEntry | null;
  getCubeKey(cube: CubeLibraryEntry | null | undefined): string;
  selectCube(cubeKey: string): void;
  beginEdit(cube: CubeLibraryEntry): void;
}

/** Coordinate the browser's embedded host surface and open-for-edit flow. */
export class CubeBrowserViewHost {
  private triggerButton: { element?: HTMLElement } | null = null;
  private mount: HTMLElement | null = null;
  private built = false;

  constructor(private readonly options: ViewHostOptions) {}

  /** Build or remount the browser view. */
  ensureView(mount: HTMLElement | null = null): boolean {
    const { view } = this.options;
    if (!this.built) {
      if (!mount) return false;
      this.options.bindHandlers();
      view.build();
      view.mount(mount);
      this.built = true;
      this.mount = mount;
      this.options.preview.attach(view.getPreviewElements());
      this.options.store.setInitialized(true);
      try {
        this.options.getActions().emitProximityLog?.('cube-browser-initialized', {
          favorites: this.options.store.state.favorites.size,
          recents: this.options.store.state.recents.length,
        });
      } catch (_error) {
        // Host diagnostics must not interrupt browser mounting.
      }
      return true;
    }
    if (mount && view.elements?.dialog) {
      if (mount !== this.mount || view.elements.dialog.parentElement !== mount) {
        mount.replaceChildren(view.elements.dialog);
      }
      this.mount = mount;
    }
    return true;
  }

  /** Mount the browser into its embedded host and populate the catalog. */
  mountEmbedded(container: HTMLElement | null | undefined): void {
    if (!container) return;
    void this.options.initialize().then(() => {
      if (!this.ensureView(container)) return;
      this.options.render();
      void this.options.refresh(false).catch(() => {});
    });
  }

  /** Open and focus the existing embedded browser. */
  open(triggerButton: { element?: HTMLElement } | null): boolean {
    const canOpen = this.built;
    void this.prepareOpen().then(() => {
      this.triggerButton = triggerButton;
    });
    return canOpen;
  }

  /** Open the browser and begin editing the requested catalog Cube. */
  openForEdit(cubeId: string | undefined): boolean {
    void this.prepareOpen().then(async () => {
      if (this.shouldRefresh()) await this.options.refresh(true);
      const selected = this.options.getCube(cubeId, this.options.store.state.cubes);
      const key = this.options.getCubeKey(selected);
      if (!selected || !key) return;
      this.options.selectCube(key);
      this.options.beginEdit(selected);
    });
    return this.built;
  }

  /** Restore focus to the button that opened the browser. */
  close(): void {
    this.triggerButton?.element?.focus?.();
  }

  private async prepareOpen(): Promise<void> {
    await this.options.initialize();
    if (!this.ensureView()) return;
    const { store, view } = this.options;
    store.setDropOrigin(this.options.getActions().computeDropOrigin?.() || [0, 0]);
    view.scrollIntoView();
    view.focusSearch();
    if (this.shouldRefresh()) {
      void this.options.refresh(true).catch(() => {});
    } else {
      this.options.applyFilters();
      this.options.render();
    }
  }

  private shouldRefresh(): boolean {
    const state = this.options.store.state;
    return !state.cubes.length || Date.now() - state.lastFetched > REFRESH_TTL_MS;
  }
}
