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
/** Add a picked Cube right after another, through the same chain the Add cubes dialog uses. */
import type { ComfyCubeRuntime } from '../cube/ComfyCubeRuntime.js';
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubePickerPlacementAdapter } from './CubePickerPlacementAdapter.js';

export interface CubePickerInsertAfterServiceOptions {
  placement: Pick<CubePickerPlacementAdapter, 'prepare'>;
  getRuntime(): Pick<ComfyCubeRuntime, 'chain' | 'construction'>;
  createId(): string;
}

/** Turn one picked catalog type into a new Cube after the source in the stack. */
export class CubePickerInsertAfterService {
  readonly #options: CubePickerInsertAfterServiceOptions;

  /** Bind the picker's prepared payloads to the live chain. */
  constructor(options: CubePickerInsertAfterServiceOptions) {
    this.#options = options;
  }

  /** Insert one Cube after the source instance and return its node. */
  insert(sourceInstanceId: string, type: string): CubeNode {
    const runtime = this.#options.getRuntime();
    const prepared = this.#options.placement.prepare(type);
    const instanceId = this.#options.createId();
    const size = runtime.construction.previewSize(
      prepared.payload,
      prepared.descriptor.inputs.length > 0,
      { instanceId },
    );
    return runtime.chain.insertAfter(sourceInstanceId, {
      instanceId,
      alias: prepared.descriptor.displayName,
      payload: prepared.payload,
      size: [size[0], size[1]],
    });
  }
}
