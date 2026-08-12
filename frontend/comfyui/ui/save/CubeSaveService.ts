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
/** Expose the host-facing Cube save application service. */

import { CubeSaveCommandCoordinator } from './CubeSaveCommandCoordinator.js';
import { CubeSaveForkCoordinator } from './CubeSaveForkCoordinator.js';
import { CubeSaveIdentityAssigner } from './CubeSaveIdentityAssigner.js';
import { CubeSaveReconciliationTargets } from './CubeSaveReconciliationTargets.js';
import { CubeSaveSourceCatalog } from './CubeSaveSourceCatalog.js';
import type { CubeSaveDependencies, CubeSaveOutcome, SaveRequest } from './CubeSaveContracts.js';

export type { CubeSaveOutcome, CubeSaveStatus } from './CubeSaveContracts.js';

/** Preserve the public save API while delegating each workflow concern to its owner. */
export class CubeSaveService {
  private readonly commands: CubeSaveCommandCoordinator;

  constructor(dependencies: CubeSaveDependencies) {
    const identities = new CubeSaveIdentityAssigner({
      adapter: dependencies.adapter,
      instanceManager: dependencies.instanceManager ?? null,
      dirtyManager: dependencies.dirtyManager ?? null,
      cubeBrowser: dependencies.cubeBrowser ?? null,
    });
    const sources = new CubeSaveSourceCatalog({
      adapter: dependencies.adapter,
      instanceManager: dependencies.instanceManager ?? null,
      cubeNodeSave: dependencies.cubeNodeSave ?? null,
    });
    const forks = new CubeSaveForkCoordinator({
      dialogs: dependencies.dialogs ?? null,
      cubeNodeSave: dependencies.cubeNodeSave ?? null,
      identities,
      sources,
    });
    this.commands = new CubeSaveCommandCoordinator({
      dependencies,
      identities,
      sources,
      forks,
      reconciliationTargets: new CubeSaveReconciliationTargets(),
    });
  }

  /** Save requested Cubes and report whether their persisted identities were finalized. */
  async save(request: SaveRequest = {}): Promise<CubeSaveOutcome> {
    return this.commands.execute(request);
  }

  /** Preserve the established host-facing implementation command. */
  async saveImplementation(request: SaveRequest = {}): Promise<CubeSaveOutcome> {
    return this.commands.execute(request);
  }
}
