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
/** Own Cube browser import, historical-load, and placement commands. */

import { CURRENT_REVISION_REF, isCurrentRevisionRef } from '../core/CubeDefinitionKey.js';
import type { Vec2 } from '../types/common.js';
import type { BrowserActions, BrowserToast, BusyImportOptions } from './CubeBrowserContracts.js';
import type { CubeBrowserStore, CubeLibraryEntry, CubeVersionOption } from './CubeBrowserStore.js';

interface PlacementOptions {
  store: CubeBrowserStore;
  toast: BrowserToast;
  getActions(): BrowserActions;
  getCube(cubeKey: unknown): CubeLibraryEntry | null;
  getVersionOption(): CubeVersionOption | null;
  close(): void;
  render(): void;
}

/** Execute browser placement commands without owning catalog or view state. */
export class CubeBrowserPlacementCommands {
  constructor(private readonly options: PlacementOptions) {}

  /** Import the selected current Cube revision. */
  importCube(): void {
    const { store } = this.options;
    const selected = this.options.getCube(store.state.selected);
    if (!store.state.selected) {
      this.options.toast.push(
        'warn',
        'Select a cube',
        'Choose a cube from the list before importing.',
      );
      return;
    }
    if (!selected?.cube_id) {
      this.options.toast.push('error', 'Import failed', 'Cube id missing.');
      return;
    }
    store.setBusy(true);
    this.options.render();
    void this.options
      .getActions()
      .importCubeByName?.(selected.cube_id, this.busyOptions())
      ?.then((result) => result?.success && this.options.close())
      ?.finally(() => this.setBusy(false));
  }

  /** Load the selected historical revision, or import the current worktree. */
  async loadSelectedRevision(): Promise<void> {
    const { store } = this.options;
    const selected = this.options.getCube(store.state.selected);
    if (!store.state.selected) {
      this.options.toast.push('warn', 'Select a cube', 'Choose a cube before loading a revision.');
      return;
    }
    if (!selected?.cube_id) {
      this.options.toast.push('error', 'Load failed', 'Cube id missing.');
      return;
    }
    const revisionRef = store.state.selectedRevision || CURRENT_REVISION_REF;
    if (isCurrentRevisionRef(revisionRef)) {
      this.importCube();
      return;
    }
    this.setBusy(true);
    try {
      const result = await this.options
        .getActions()
        .importCubeRevision?.(selected.cube_id, revisionRef, this.busyOptions());
      if (result?.success) this.options.close();
    } finally {
      this.setBusy(false);
    }
  }

  /** Start interactive placement for the selected version. */
  placeCube(): void {
    const { store } = this.options;
    const selected = this.options.getCube(store.state.selected);
    if (!store.state.selected) {
      this.options.toast.push(
        'warn',
        'Select a cube',
        'Choose a cube from the list before placing.',
      );
      return;
    }
    if (!selected?.cube_id) {
      this.options.toast.push('error', 'Placement failed', 'Cube id missing.');
      return;
    }
    const version = this.options.getVersionOption();
    this.options.getActions().startCubePlacement?.(selected.cube_id, {
      closeBrowser: false,
      defaultAlias: selected.display_name?.trim() || selected.name || store.state.selected,
      revisionRef: version?.revisionRef || CURRENT_REVISION_REF,
      version: version?.value || selected.version || '',
    });
  }

  private busyOptions(): BusyImportOptions {
    return {
      dropOrigin: this.options.store.state.dropOrigin as Vec2,
      setBusy: (busy) => {
        this.options.store.setBusy(busy);
        if (!busy) this.options.store.setLoading(false);
        this.options.render();
      },
    };
  }

  private setBusy(value: boolean): void {
    this.options.store.setBusy(value);
    this.options.render();
  }
}
