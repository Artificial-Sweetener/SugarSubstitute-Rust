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
 * Give a loaded Cube definition's boundaries back the links inside it.
 *
 * Images written before 2026-11-25 carry the workflow execution prepared rather than the one that
 * was queued: each Cube's boundary records there name the port and its type and nothing else.
 * The links that reach those ports are still in the definition, but a boundary that lists none
 * reads as unconnected, so the Cube showed no ports and no proximity link once loaded. Before
 * Comfy configures a workflow, each Cube boundary that lists no links gets the ones the
 * definition draws to it; a boundary that lists its links is left exactly as it is.
 */
import { isCubeKind } from './CubeKind.js';
import { normalizeSubgraphLinks } from '../graph/SubgraphConnectionSerialization.js';
import { isRecord } from '../types/common.js';

const INPUT_NODE = -10;
const OUTPUT_NODE = -20;

/** Fill missing boundary links in every serialized Cube definition; return how many changed. */
export function repairCubeBoundaryLinks(workflow: unknown): number {
  if (!isRecord(workflow) || !isRecord(workflow.definitions)) return 0;
  const definitions = workflow.definitions.subgraphs;
  if (!Array.isArray(definitions)) return 0;
  let repaired = 0;
  for (const definition of definitions) {
    if (!isRecord(definition) || !isRecord(definition.extra)) continue;
    if (!isCubeKind(definition.extra.sugarcubes_kind)) continue;
    let links;
    try {
      links = normalizeSubgraphLinks(definition.links);
    } catch {
      continue;
    }
    const inputNode = readNodeId(definition.inputNode, INPUT_NODE);
    const outputNode = readNodeId(definition.outputNode, OUTPUT_NODE);
    repaired += fill(definition.inputs, (slot) =>
      links.filter((link) => link.origin_id === inputNode && link.origin_slot === slot),
    );
    repaired += fill(definition.outputs, (slot) =>
      links.filter((link) => link.target_id === outputNode && link.target_slot === slot),
    );
  }
  return repaired;
}

/** Give each boundary record without a link list the links the definition draws to it. */
function fill(records: unknown, linksTo: (slot: number) => readonly { id: number }[]): number {
  if (!Array.isArray(records)) return 0;
  let filled = 0;
  records.forEach((record, slot) => {
    if (!isRecord(record) || Array.isArray(record.linkIds)) return;
    const found = linksTo(slot);
    if (!found.length) return;
    record.linkIds = found.map((link) => link.id);
    filled += 1;
  });
  return filled;
}

function readNodeId(node: unknown, fallback: number): number {
  return isRecord(node) && typeof node.id === 'number' ? node.id : fallback;
}
