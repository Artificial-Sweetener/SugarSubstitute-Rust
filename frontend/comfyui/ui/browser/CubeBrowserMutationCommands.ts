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
/** Own destructive and lineage Cube browser mutation commands. */

import {
  hasApiError,
  readApiErrorMessage,
  readErrorMessage,
  type BrowserActions,
  type BrowserApi,
  type BrowserToast,
} from './CubeBrowserContracts.js';
import type { CubeBrowserStore, CubeLibraryEntry } from './CubeBrowserStore.js';

interface CommandOptions {
  api: BrowserApi;
  store: CubeBrowserStore;
  toast: BrowserToast | null;
  getActions(): BrowserActions;
  getCube(cubeKey: unknown): CubeLibraryEntry | null;
  removeCube(cubeKey: string): void;
  refresh(): Promise<CubeLibraryEntry[]>;
  selectCube(cubeKey: string): void;
  render(): void;
}

/** Execute browser mutations while maintaining busy state and feedback. */
export class CubeBrowserMutationCommands {
  constructor(private readonly options: CommandOptions) {}

  /** Clear lineage metadata from one writable Cube. */
  async clearLineage(selected: CubeLibraryEntry | null | undefined): Promise<void> {
    if (!selected?.cube_id) return;
    if (!selected.is_writable) {
      this.options.toast?.push(
        'warn',
        'Read-only cube',
        selected.write_block_reason || 'This cube is read-only.',
      );
      return;
    }
    const confirmed = await this.options.getActions().openConfirmDialog?.({
      title: 'Clear lineage?',
      message: 'This removes the fork lineage metadata from this cube.',
      confirmLabel: 'Clear',
    });
    if (!confirmed) return;
    await this.withBusy(async () => {
      try {
        const { response, data } = await this.options.api.updateMetadata(
          JSON.stringify({ cube_id: selected.cube_id, metadata: { lineage: null } }),
          { headers: { 'Content-Type': 'application/json' } },
        );
        if (!response.ok || hasApiError(data)) {
          this.options.toast?.push(
            'error',
            'Lineage update failed',
            readApiErrorMessage(data, response.statusText || 'Update failed'),
          );
          return;
        }
        await this.options.refresh();
        this.options.selectCube(selected.cube_id || '');
      } catch (error) {
        this.options.toast?.push('error', 'Lineage update failed', readErrorMessage(error));
      }
    });
  }

  /** Confirm and delete the currently selected writable Cube. */
  async requestDelete(): Promise<void> {
    const cubeKey = this.options.store.state.selected || '';
    if (!cubeKey) {
      this.options.toast?.push('warn', 'Select a cube', 'Choose a cube before deleting.');
      return;
    }
    const selected = this.options.getCube(cubeKey);
    if (selected && !selected.is_writable) {
      this.options.toast?.push(
        'warn',
        'Read-only cube',
        selected.write_block_reason || 'This cube is read-only.',
      );
      return;
    }
    const cubeId = selected?.cube_id || '';
    if (!cubeId) {
      this.options.toast?.push('error', 'Delete failed', 'Cube id missing.');
      return;
    }
    const displayName = selected?.display_name || selected?.name || cubeKey;
    const confirmed = await this.options.getActions().openConfirmDialog?.({
      title: 'Delete SugarCube?',
      message: [`Delete SugarCube "${displayName}"?`, 'This cannot be undone.'],
      confirmLabel: 'Delete',
    });
    if (!confirmed) return;
    await this.withBusy(async () => {
      try {
        const { response, data } = await this.options.api.delete({ cube_id: cubeId });
        if (!response.ok || hasApiError(data)) {
          this.options.toast?.push(
            'error',
            'Delete failed',
            readApiErrorMessage(data, response.statusText || 'Failed to delete cube'),
          );
          return;
        }
        this.options.removeCube(cubeKey);
        this.options.toast?.push('success', 'Cube deleted', `Removed ${displayName}.`);
        void this.options.refresh().catch(() => {});
      } catch (error) {
        this.options.toast?.push('error', 'Delete failed', readErrorMessage(error));
      }
    });
  }

  /** Promote the currently selected personal Cube. */
  async requestPromotion(): Promise<void> {
    const selected = this.options.getCube(this.options.store.state.selected);
    if (!selected) {
      this.options.toast?.push('warn', 'Select a cube', 'Choose a personal cube before moving it.');
      return;
    }
    await this.withBusy(async () => {
      await this.options.getActions().promoteCube?.(selected);
    });
  }

  private async withBusy(action: () => Promise<void>): Promise<void> {
    this.options.store.setBusy(true);
    this.options.render();
    try {
      await action();
    } finally {
      this.options.store.setBusy(false);
      this.options.render();
    }
  }
}
