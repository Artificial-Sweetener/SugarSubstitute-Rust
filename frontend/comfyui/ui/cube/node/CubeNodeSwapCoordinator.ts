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
/** Adapt native face arrows to the canonical Cube reorder application service. */
import type {
  CubeFaceChromeMetadata,
  CubeSwapDirection,
} from '../../surface/CubeFaceChromeActions.js';
import type { CubeGraphReorderService } from '../../workflow/application/CubeGraphReorderService.js';

interface CubeNodeSwapCoordinatorOptions {
  reorder: Pick<CubeGraphReorderService, 'canMove' | 'move'>;
  reportError(error: unknown): void;
}

/** Translate face interaction without reproducing graph permissions or geometry. */
export class CubeNodeSwapCoordinator {
  readonly #options: CubeNodeSwapCoordinatorOptions;

  /** Share exactly the same application owner used by stack dragging and keyboard movement. */
  constructor(options: CubeNodeSwapCoordinatorOptions) {
    this.#options = options;
  }

  /** Reflect native order availability for this stable instance. */
  canSwap(metadata: CubeFaceChromeMetadata, direction: CubeSwapDirection): boolean {
    const instanceId = readInstanceId(metadata);
    return instanceId !== null && this.#options.reorder.canMove(instanceId, delta(direction));
  }

  /** Report rejected or stale actions through the existing SugarCubes feedback owner. */
  async swap(metadata: CubeFaceChromeMetadata, direction: CubeSwapDirection): Promise<void> {
    const instanceId = readInstanceId(metadata);
    if (!instanceId || !this.canSwap(metadata, direction)) return;
    try {
      await this.#options.reorder.move(instanceId, delta(direction));
    } catch (error: unknown) {
      this.#options.reportError(error);
    }
  }
}

/** Narrow native chrome identity without inventing an instance from an obsolete group marker. */
function readInstanceId(metadata: CubeFaceChromeMetadata): string | null {
  return typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() || null : null;
}

/** Translate native horizontal arrows into canonical adjacent movement. */
function delta(direction: CubeSwapDirection): -1 | 1 {
  return direction === 'left' ? -1 : 1;
}
