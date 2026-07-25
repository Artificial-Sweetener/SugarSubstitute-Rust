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
/** Extract only group-era Cubes before Comfy configures a workflow. */

import {
  LegacyCubeWorkflowExtractor,
  type LegacyCubeMigrationBatch,
} from './migration/LegacyCubeWorkflowExtractor.js';

/** Own the serialized workflow phase and its one pending legacy migration batch. */
export class CubeWorkflowPreconfiguration {
  readonly #legacyExtractor: LegacyCubeWorkflowExtractor;
  #legacyBatch: LegacyCubeMigrationBatch | null = null;

  /** Bind the focused legacy extraction collaborator. */
  constructor(legacyExtractor = new LegacyCubeWorkflowExtractor()) {
    this.#legacyExtractor = legacyExtractor;
  }

  /** Detach legacy records before graph construction. */
  prepare(workflow: unknown): number {
    const batch = this.#legacyExtractor.extractInPlace(workflow);
    this.#legacyBatch = batch.plans.length > 0 ? batch : null;
    return batch.plans.length;
  }

  /** Transfer the pending detached migration plan to the initialized runtime. */
  takeLegacyBatch(): LegacyCubeMigrationBatch | null {
    const batch = this.#legacyBatch;
    this.#legacyBatch = null;
    return batch;
  }
}
