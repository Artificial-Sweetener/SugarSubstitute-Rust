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
/** Own current workflow source membership and scoped authored display metadata. */

import type { WorkflowOutputSource } from '../../execution/domain/WorkflowOutput.js';
import type { WorkflowSessionContext } from '../../execution/domain/WorkflowSession.js';
import type { WorkflowSessionId } from '../../execution/domain/ExecutionIdentity.js';

interface SourceInventory {
  readonly generation: number;
  sources: readonly WorkflowOutputSource[];
}

/** Distinguish topology replacement from metadata refresh without retaining live host objects. */
export class CanvasSourceInventory {
  readonly #sessions = new Map<WorkflowSessionId, SourceInventory>();

  /** Replace one named graph inventory at an explicit topology/load boundary. */
  replace(context: WorkflowSessionContext, sources: readonly WorkflowOutputSource[]): void {
    const seen = new Set<string>();
    const ordered = [...sources]
      .sort((a, b) => a.order - b.order)
      .map((source) => {
        if (seen.has(source.id)) throw new TypeError('Canvas source identities must be unique.');
        seen.add(source.id);
        return Object.freeze({ ...source, breadcrumb: Object.freeze([...source.breadcrumb]) });
      });
    this.#sessions.set(context.sessionId, {
      generation: context.generation,
      sources: Object.freeze(ordered),
    });
  }

  /** Update only known source labels; metadata events cannot enroll new graph candidates. */
  refreshMetadata(context: WorkflowSessionContext, changed: readonly WorkflowOutputSource[]): void {
    const inventory = this.#sessions.get(context.sessionId);
    if (!inventory || inventory.generation !== context.generation) return;
    const updates = new Map(changed.map((source) => [source.id, source]));
    inventory.sources = Object.freeze(
      inventory.sources.map((source) => {
        const next = updates.get(source.id);
        if (!next) return source;
        return Object.freeze({
          ...source,
          label: next.label,
          breadcrumb: Object.freeze([...next.breadcrumb]),
        });
      }),
    );
  }

  /** Read only the matching committed graph generation. */
  read(context: WorkflowSessionContext): readonly WorkflowOutputSource[] {
    const inventory = this.#sessions.get(context.sessionId);
    return inventory?.generation === context.generation ? inventory.sources : [];
  }

  /** Retire metadata when its browser workflow binding closes. */
  remove(id: WorkflowSessionId): void {
    this.#sessions.delete(id);
  }

  /** Release the integration's lightweight metadata at teardown. */
  dispose(): void {
    this.#sessions.clear();
  }
}
