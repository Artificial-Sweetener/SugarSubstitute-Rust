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

/** Clone and enrich one workflow with authoritative subgraphs and widget snapshots. */
export function enrichWorkflowPayload(workflowPayload, graph) {
  const cloned =
    workflowPayload && typeof workflowPayload === 'object'
      ? JSON.parse(JSON.stringify(workflowPayload))
      : {};
  const definitions =
    cloned.definitions && typeof cloned.definitions === 'object' ? { ...cloned.definitions } : {};
  definitions.subgraphs = collectWorkflowSubgraphs(workflowPayload, graph);
  cloned.definitions = definitions;
  return attachWorkflowWidgetSnapshots(cloned, graph);
}

/** Merge declared and live subgraph definitions by stable id. */
export function collectWorkflowSubgraphs(workflowPayload, graph) {
  const merged = new Map();
  const declared = workflowPayload?.definitions?.subgraphs;
  if (Array.isArray(declared)) {
    for (const entry of declared) {
      const normalized = normalizeSubgraphEntry(entry, entry?.id);
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
export function normalizeSubgraphEntry(rawEntry, fallbackId) {
  if (!rawEntry) {
    return null;
  }
  let entry = rawEntry;
  try {
    if (typeof rawEntry?.asSerialisable === 'function') {
      entry = rawEntry.asSerialisable();
    } else if (typeof rawEntry?.serialize === 'function') {
      entry = rawEntry.serialize();
    } else if (typeof rawEntry?.graph?.asSerialisable === 'function') {
      entry = rawEntry.graph.asSerialisable();
    } else if (typeof rawEntry?.graph?.serialize === 'function') {
      entry = rawEntry.graph.serialize();
    }
  } catch (_error) {
    return null;
  }
  if (!entry || typeof entry !== 'object' || Array.isArray(entry)) {
    return null;
  }
  return normalizeSubgraphPayload(entry, fallbackId);
}
