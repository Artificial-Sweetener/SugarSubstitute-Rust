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
/** Coordinate Add Cube candidates, menu presentation, and insert-after selection. */

import type { ComfyCubeRuntime } from '../cube/ComfyCubeRuntime.js';
import type {
  CubeFaceActionAnchor,
  CubeFaceChromeMetadata,
} from '../surface/CubeFaceChromeActions.js';
import type { CubeAddCandidateCatalog } from './CubeAddCandidateCatalog.js';
import type { ComfyCubeAddMenuPresenter } from './ComfyCubeAddMenuPresenter.js';
import type { CubePickerCatalogRegistry } from './CubePickerCatalogRegistry.js';
import type { CubePickerInsertAfterService } from './CubePickerInsertAfterService.js';

export interface CubeAddMenuControllerOptions {
  registry: CubePickerCatalogRegistry;
  candidates: CubeAddCandidateCatalog;
  insertion: CubePickerInsertAfterService;
  presenter: ComfyCubeAddMenuPresenter;
  getRuntime(): ComfyCubeRuntime;
  reportError(summary: string, detail: string): void;
}

/** Own the renderer-neutral Add Cube use-case entry point. */
export class CubeAddMenuController {
  readonly #registry: CubePickerCatalogRegistry;
  readonly #candidates: CubeAddCandidateCatalog;
  readonly #insertion: CubePickerInsertAfterService;
  readonly #presenter: ComfyCubeAddMenuPresenter;
  readonly #getRuntime: () => ComfyCubeRuntime;
  readonly #reportError: (summary: string, detail: string) => void;

  /** Bind catalog discovery, presentation, and graph mutation collaborators. */
  constructor(options: CubeAddMenuControllerOptions) {
    this.#registry = options.registry;
    this.#candidates = options.candidates;
    this.#insertion = options.insertion;
    this.#presenter = options.presenter;
    this.#getRuntime = options.getRuntime;
    this.#reportError = options.reportError;
  }

  /** Toggle compatible Cube choices for one live source instance. */
  open(metadata: CubeFaceChromeMetadata, anchor: CubeFaceActionAnchor): void {
    try {
      const instanceId = readInstanceId(metadata);
      const source = this.#getRuntime().nodes.get(instanceId);
      if (!source) throw new Error(`Cube instance '${instanceId}' is no longer available.`);
      const groups = this.#candidates.candidates(source, this.#registry.entries());
      this.#presenter.toggle(anchor, {
        groups,
        search: (query) => this.#candidates.search(groups, query),
        select: (type) => {
          try {
            this.#insertion.insert(instanceId, type);
          } catch (error: unknown) {
            this.#reportError('Add Cube failed', readErrorMessage(error));
          }
        },
      });
    } catch (error: unknown) {
      this.#reportError('Add Cube unavailable', readErrorMessage(error));
    }
  }

  /** Close renderer-owned transient state before a node-style transition. */
  close(): void {
    this.#presenter.close();
  }

  /** Release every Add Cube presentation resource. */
  dispose(): void {
    this.#presenter.dispose();
  }
}

/** Require the stable instance identity carried by every Cube face. */
function readInstanceId(metadata: CubeFaceChromeMetadata): string {
  const instanceId = typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() : '';
  if (!instanceId) throw new TypeError('Cube face has no stable instance identity.');
  return instanceId;
}

/** Normalize one caught error without exposing unsafe dynamic values. */
function readErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message.trim();
  return String(error || 'Unknown Add Cube failure');
}
