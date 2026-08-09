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
/** Register nested subgraph definitions embedded in an imported Cube payload. */

import { normalizeSubgraphPayload } from '../graph/SubgraphSerialization.js';
import { rebindSubgraphWidgetValues } from '../graph/SubgraphWidgetValueRebinder.js';
import type { ImportPayload } from '../import/PlacementPayload.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';

interface ImportedSubgraph {
  configure?(data: UnknownRecord): unknown;
}

export interface CubeSubgraphRegistrationHost {
  getSubgraph(id: string): ImportedSubgraph | null;
  createSubgraph(data: UnknownRecord): ImportedSubgraph | null;
  createNode(type: string): ComfyNode | null;
  discardSubgraph(id: string): void;
}

interface SubgraphHint {
  fallbackName: string;
  expectedInputNames: string[];
}

/** Own normalization and host registration of Cube-embedded nested subgraphs. */
export class CubeSubgraphRegistrar {
  readonly #host: CubeSubgraphRegistrationHost;

  /** Bind the narrow native graph and node-construction boundary. */
  constructor(host: CubeSubgraphRegistrationHost) {
    this.#host = host;
  }

  /** Synchronize every nested definition and return actionable warnings. */
  register(payload: ImportPayload): string[] {
    const warnings: string[] = [];
    const hints = buildHintLookup(payload);
    for (const entry of payload.subgraphs ?? []) {
      const id = readString(entry.id);
      if (!id) {
        warnings.push('Subgraph entry missing id; skipping.');
        continue;
      }
      try {
        const hint = hints.get(id) ?? { fallbackName: '', expectedInputNames: [] };
        const normalized = normalizeSubgraphPayload(entry, id, hint);
        if (!normalized) {
          warnings.push(`Subgraph '${id}' could not be normalized; skipping.`);
          continue;
        }
        rebindSubgraphWidgetValues(normalized, (type) =>
          type ? this.#host.createNode(type) : null,
        );
        const subgraph = this.#host.getSubgraph(id) ?? this.#host.createSubgraph(normalized);
        if (!subgraph) {
          warnings.push(`Subgraph '${id}' could not be synchronized; skipping.`);
          continue;
        }
        subgraph.configure?.(normalized);
      } catch (error: unknown) {
        warnings.push(`Failed to register subgraph '${id}': ${readErrorMessage(error)}`);
      }
    }
    return warnings;
  }

  /** Discard isolated definitions that did not reach a committed Cube version. */
  discard(ids: readonly string[]): void {
    for (const id of ids) this.#host.discardSubgraph(id);
  }
}

/** Index wrapper-authored names needed to normalize older subgraph payloads. */
function buildHintLookup(payload: ImportPayload): Map<string, SubgraphHint> {
  const lookup = new Map<string, SubgraphHint>();
  for (const entry of payload.nodes ?? []) {
    const type = readString(entry.class_type);
    if (!type) continue;
    const metadata = isRecord(entry.extras?._meta) ? entry.extras._meta : {};
    const existing = lookup.get(type);
    const title = readString(entry.layout?.title) || readString(metadata.title);
    const inputs = isRecord(entry.inputs) ? Object.keys(entry.inputs) : [];
    lookup.set(type, {
      fallbackName: title || existing?.fallbackName || '',
      expectedInputNames: inputs.length > 0 ? inputs : (existing?.expectedInputNames ?? []),
    });
  }
  return lookup;
}

/** Read one non-empty dynamic string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Preserve error context at the host registration boundary. */
function readErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
