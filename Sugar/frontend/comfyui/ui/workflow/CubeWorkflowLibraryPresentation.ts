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
/** Project transient workflow-library policy into native Cube presentation. */

import type { CubeIdentitySource } from '../cube/CubeIdentityPresentation.js';
import {
  isDraftCubeNode,
  requireCubeIdentity,
  type CubeNode,
} from '../cube/node/ComfyCubeNodeFactory.js';
import type { UnknownRecord } from '../types/common.js';
import type { CubeWorkflowLibraryState } from './CubeWorkflowLibraryState.js';

interface WritableCubeCatalog {
  canWriteCube(cubeId: string): Promise<boolean>;
}

/** Keep library badges and edit decisions derived rather than serialized. */
export class CubeWorkflowLibraryPresentation {
  readonly #state: CubeWorkflowLibraryState;
  readonly #writableCubes: WritableCubeCatalog;

  /** Bind transient classification and the existing catalog permission fallback. */
  constructor(state: CubeWorkflowLibraryState, writableCubes: WritableCubeCatalog) {
    this.#state = state;
    this.#writableCubes = writableCubes;
  }

  /** Resolve only the machine-local class label used by native Cube faces. */
  resolveSource(metadata: UnknownRecord): CubeIdentitySource | null {
    const classification = this.#classification(metadata);
    return classification ? { libraryClass: classification.primaryClass } : null;
  }

  /** Permit definition editing for drafts or a verified writable source. */
  async canEdit(node: CubeNode): Promise<boolean> {
    if (isDraftCubeNode(node)) return true;
    const identity = requireCubeIdentity(node);
    const classifiedAccess = this.#classification(identity)?.access;
    if (classifiedAccess === 'writable') return true;
    const cubeId = readString(identity.cube_id);
    return cubeId ? this.#writableCubes.canWriteCube(cubeId) : false;
  }

  /** Find the current instance classification from one native identity record. */
  #classification(metadata: UnknownRecord) {
    const instanceId = readString(metadata.instance_id);
    return instanceId ? this.#state.read(instanceId) : null;
  }
}

/** Normalize optional identity text before calling a policy owner. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
