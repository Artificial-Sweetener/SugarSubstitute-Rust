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
/** Coordinate selection-scoped version availability and switching for the toolbox. */

import type { CubeVersionAvailabilityService } from '../cube/version/CubeVersionAvailabilityService.js';
import type { CubeVersionOption } from '../cube/version/CubeVersionTypes.js';
import { normalizeCubeVersion } from '../core/CubeDefinitionKey.js';
import { requireCubeIdentity, type CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeVersionToolboxModel } from './ComfyCubeVersionToolboxPresenter.js';

export interface CubeVersionToolboxView {
  present(anchor: HTMLButtonElement, model: CubeVersionToolboxModel): HTMLElement;
  clear(): void;
  dispose(): void;
}

export interface CubeVersionSwitcher {
  switch(node: CubeNode, targetVersion: string): Promise<CubeNode>;
}

/** Own asynchronous state and stale-selection protection around the version presenter. */
export class CubeVersionToolboxController {
  readonly #availability: CubeVersionAvailabilityService;
  readonly #switcher: CubeVersionSwitcher;
  readonly #presenter: CubeVersionToolboxView;
  readonly #logger: Pick<Console, 'error'>;
  readonly #reportError: (message: string) => void;
  #anchor: HTMLButtonElement | null = null;
  #node: CubeNode | null = null;
  #options: readonly CubeVersionOption[] = [];
  #loading = false;
  #busy = false;
  #error: string | null = null;
  #generation = 0;

  /** Bind application services and presentation feedback. */
  constructor(options: {
    availability: CubeVersionAvailabilityService;
    switcher: CubeVersionSwitcher;
    presenter: CubeVersionToolboxView;
    logger: Pick<Console, 'error'>;
    reportError(message: string): void;
  }) {
    this.#availability = options.availability;
    this.#switcher = options.switcher;
    this.#presenter = options.presenter;
    this.#logger = options.logger;
    this.#reportError = options.reportError;
  }

  /** Present one selected persisted Cube after the supplied host action. */
  present(anchor: HTMLButtonElement, node: CubeNode): HTMLElement {
    const selectionChanged = this.#node !== node;
    if (selectionChanged) {
      this.#generation += 1;
      this.#node = node;
      this.#options = [];
      this.#loading = false;
      this.#busy = false;
      this.#error = null;
    }
    this.#anchor = anchor;
    const control = this.#render();
    if (selectionChanged) void this.#loadOptions();
    return control;
  }

  /** Clear selection-owned state and invalidate outstanding requests. */
  clear(): void {
    this.#generation += 1;
    this.#anchor = null;
    this.#node = null;
    this.#options = [];
    this.#loading = false;
    this.#busy = false;
    this.#error = null;
    this.#presenter.clear();
  }

  /** Release the focused presenter. */
  dispose(): void {
    this.clear();
    this.#presenter.dispose();
  }

  /** Build one immutable callback model for the current selected node. */
  #render(): HTMLElement {
    if (!this.#anchor || !this.#node) throw new Error('Cube version selection is unavailable.');
    const currentVersion = readCurrentVersion(this.#node);
    const model: CubeVersionToolboxModel = {
      currentVersion,
      options: this.#options,
      loading: this.#loading,
      busy: this.#busy,
      error: this.#error,
      select: (version) => void this.#select(version),
    };
    return this.#presenter.present(this.#anchor, model);
  }

  /** Load options once and ignore completions from an obsolete selection. */
  async #loadOptions(): Promise<void> {
    if (!this.#node || this.#loading || this.#options.length > 0) return;
    const node = this.#node;
    const generation = this.#generation;
    const identity = requireCubeIdentity(node);
    const cubeId = readString(identity.cube_id);
    const currentVersion = readCurrentVersion(node);
    this.#loading = true;
    this.#error = null;
    this.#render();
    try {
      const options = await this.#availability.list(cubeId, currentVersion);
      if (generation !== this.#generation || node !== this.#node) return;
      this.#options = options;
    } catch (error: unknown) {
      if (generation !== this.#generation || node !== this.#node) return;
      this.#error = readError(error);
      this.#logger.error('SugarCubes: Cube version discovery failed.', error);
    } finally {
      if (generation === this.#generation && node === this.#node) {
        this.#loading = false;
        this.#render();
      }
    }
  }

  /** Execute one switch while preventing duplicate mutations and stale UI writes. */
  async #select(version: string): Promise<void> {
    if (!this.#node || this.#busy) return;
    const node = this.#node;
    const generation = this.#generation;
    this.#busy = true;
    this.#error = null;
    this.#render();
    try {
      const replacement = await this.#switcher.switch(node, version);
      if (generation !== this.#generation || node !== this.#node) return;
      this.#busy = false;
      this.#node = replacement;
      this.#render();
      return;
    } catch (error: unknown) {
      const message = readError(error);
      this.#logger.error('SugarCubes: Cube version switch failed.', error);
      this.#reportError(message);
    } finally {
      if (generation === this.#generation && node === this.#node && this.#busy) {
        this.#busy = false;
        this.#render();
      }
    }
  }
}

/** Read one normalized current version from persisted instance identity. */
function readCurrentVersion(node: CubeNode): string {
  return normalizeCubeVersion(requireCubeIdentity(node).cube_version);
}

/** Read one trimmed identity string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Normalize unknown failures for SugarCubes feedback. */
function readError(error: unknown): string {
  return error instanceof Error && error.message ? error.message : 'Cube version switch failed.';
}
