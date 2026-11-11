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
/**
 * Own Comfy Registry metadata persisted on top-level Cube workflow nodes.
 */

import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';

interface WorkflowNodePackRequirement {
  cnrId: string;
  version: string;
}

interface WorkflowNodeWithProperties {
  properties: UnknownRecord;
}

/** Validate, retain, and project the backend-owned SugarCubes package identity. */
export class CubeWorkflowNodePackMetadata {
  readonly #listeners = new Set<() => void>();
  #requirement: WorkflowNodePackRequirement | null = null;

  /** Replace package identity from one trusted SugarCubes status response. */
  updateFromStatus(status: unknown): void {
    if (!isRecord(status) || !isRecord(status.workflowNodePack)) {
      throw new TypeError('SugarCubes workflow node-pack metadata is unavailable.');
    }
    const cnrId = requireText(status.workflowNodePack.cnrId, 'cnrId');
    const version = requireText(status.workflowNodePack.version, 'version');
    const current = this.#requirement;
    if (current?.cnrId === cnrId && current.version === version) return;
    this.#requirement = { cnrId, version };
    for (const listener of this.#listeners) listener();
  }

  /** Write current package metadata without replacing any other node properties. */
  apply(node: WorkflowNodeWithProperties): boolean {
    const requirement = this.#requirement;
    if (!requirement) return false;
    const changed =
      node.properties.cnr_id !== requirement.cnrId || node.properties.ver !== requirement.version;
    node.properties.cnr_id = requirement.cnrId;
    node.properties.ver = requirement.version;
    return changed;
  }

  /** Observe identity availability so previously loaded workflows can reconcile. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
}

/** Require one non-empty backend protocol field. */
function requireText(value: unknown, field: string): string {
  const text = typeof value === 'string' ? value.trim() : '';
  if (!text) throw new TypeError(`SugarCubes workflow node-pack ${field} is required.`);
  return text;
}
