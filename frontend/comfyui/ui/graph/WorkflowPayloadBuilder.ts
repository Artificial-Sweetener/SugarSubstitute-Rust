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
/** Build save-ready ComfyUI workflow payloads from live graph state. */

import { normalizeSubgraphPayload } from './SubgraphSerialization.js';
import { attachWorkflowWidgetSnapshots } from './WidgetSnapshots.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGraph } from '../types/graph.js';
import type { WorkflowWithNodes } from './WidgetSnapshots.js';

interface WorkflowPayload extends WorkflowWithNodes {
  definitions?: UnknownRecord;
}

export interface NormalizedSubgraph extends UnknownRecord {
  id: string;
}

function cloneWorkflowPayload(value: unknown): WorkflowPayload {
  if (!isRecord(value)) {
    return {};
  }
  const cloned: unknown = JSON.parse(JSON.stringify(value));
  return isRecord(cloned) ? (cloned as WorkflowPayload) : {};
}

/** Clone and enrich one workflow with authoritative subgraphs and widget snapshots. */
export function enrichWorkflowPayload(
  workflowPayload: unknown,
  graph: ComfyGraph | null | undefined,
): WorkflowPayload {
  const cloned = cloneWorkflowPayload(workflowPayload);
  const definitions = isRecord(cloned.definitions) ? { ...cloned.definitions } : {};
  definitions.subgraphs = collectWorkflowSubgraphs(workflowPayload, graph);
  cloned.definitions = definitions;
  return attachWorkflowWidgetSnapshots(cloned, graph);
}

/** Merge declared and live subgraph definitions by stable id. */
export function collectWorkflowSubgraphs(
  workflowPayload: unknown,
  graph: ComfyGraph | null | undefined,
): NormalizedSubgraph[] {
  const merged = new Map<string, NormalizedSubgraph>();
  const payload = isRecord(workflowPayload) ? workflowPayload : {};
  const definitions = isRecord(payload.definitions) ? payload.definitions : {};
  const declared = definitions.subgraphs;
  if (Array.isArray(declared)) {
    for (const entry of declared) {
      const normalized = normalizeSubgraphEntry(entry, isRecord(entry) ? entry.id : undefined);
      if (normalized) {
        merged.set(normalized.id, normalized);
      }
    }
  }
  const liveSubgraphs = graph?._subgraphs instanceof Map ? graph._subgraphs : null;
  if (liveSubgraphs) {
    for (const [subgraphId, subgraph] of liveSubgraphs.entries()) {
      const normalized = normalizeSubgraphEntry(subgraph, subgraphId);
      if (normalized) {
        merged.set(normalized.id, normalized);
      }
    }
  }
  return Array.from(merged.values());
}

/** Normalize any supported LiteGraph subgraph representation. */
export function normalizeSubgraphEntry(
  rawEntry: unknown,
  fallbackId: unknown,
): NormalizedSubgraph | null {
  if (!isRecord(rawEntry)) {
    return null;
  }
  let entry: unknown = rawEntry;
  try {
    if (typeof rawEntry.asSerialisable === 'function') {
      entry = rawEntry.asSerialisable.call(rawEntry);
    } else if (typeof rawEntry.serialize === 'function') {
      entry = rawEntry.serialize.call(rawEntry);
    } else if (isRecord(rawEntry.graph) && typeof rawEntry.graph.asSerialisable === 'function') {
      entry = rawEntry.graph.asSerialisable.call(rawEntry.graph);
    } else if (isRecord(rawEntry.graph) && typeof rawEntry.graph.serialize === 'function') {
      entry = rawEntry.graph.serialize.call(rawEntry.graph);
    }
  } catch (_error) {
    return null;
  }
  if (!isRecord(entry)) {
    return null;
  }
  const normalized: unknown = normalizeSubgraphPayload(entry, fallbackId);
  return isRecord(normalized) && typeof normalized.id === 'string'
    ? (normalized as NormalizedSubgraph)
    : null;
}
