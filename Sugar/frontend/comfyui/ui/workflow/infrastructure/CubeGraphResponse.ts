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
/** Validate canonical graph identity, segment ownership and geometry at the network boundary. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type {
  CubeGraphAnalysis,
  CubeGraphInstance,
  CubeGraphPlacement,
  CubeGraphSegment,
} from '../domain/CubeGraphModels.js';

/** Reject malformed projections before their permissions can affect live graph objects. */
export function readCubeGraphAnalysis(value: unknown): CubeGraphAnalysis {
  const response = record(value);
  if (response.schema_version !== 1) invalid('unsupported schema');
  const semanticHash = text(response.workflow_semantic_hash);
  if (!/^[a-f0-9]{64}$/u.test(semanticHash)) invalid('invalid semantic hash');
  const instances = array(response.instances).map(readInstance);
  unique(instances.map((instance) => instance.instanceId));
  unique(instances.map((instance) => instance.nodeId));
  const segments = array(response.segments).map(readSegment);
  const members = segments.flatMap((segment) => [...segment.instanceIds]);
  unique(members);
  const instanceIds = new Set(instances.map((instance) => instance.instanceId));
  if (members.length !== instances.length || members.some((id) => !instanceIds.has(id)))
    invalid('segments must partition the exact instance set');
  const nodes = array(record(response.workflow).nodes).map(record);
  const nodeIds = nodes.map((node) => nodeId(node.id));
  unique(nodeIds);
  const byNode = new Map(nodes.map((node, index) => [nodeIds[index], node]));
  const placements: CubeGraphPlacement[] = instances.map((instance) => {
    const node = byNode.get(instance.nodeId);
    if (!node) invalid('missing instance node');
    const metadata = record(record(node.properties).sugarcubes_cube);
    if (metadata.instance_id !== instance.instanceId) invalid('instance node identity mismatch');
    return {
      instanceId: instance.instanceId,
      nodeId: instance.nodeId,
      position: position(node.pos),
    };
  });
  return { semanticHash, instances, segments, placements };
}

/** Parse the schema-one instance fields without coercing identity or execution mode. */
function readInstance(value: unknown): CubeGraphInstance {
  const instance = record(value);
  const mode = instance.execution_mode;
  if (typeof mode !== 'number' || !Number.isInteger(mode) || mode < 0)
    invalid('invalid execution mode');
  return {
    instanceId: text(instance.instance_id),
    nodeId: text(instance.node_id),
    definitionId: text(instance.definition_id),
    cubeId: text(instance.cube_id, true),
    version: text(instance.cube_version, true),
    alias: text(instance.instance_alias, true),
    executionMode: mode,
  };
}

/** Require coherent segment permissions instead of accepting contradictory boundaries. */
function readSegment(value: unknown): CubeGraphSegment {
  const segment = record(value);
  const instanceIds = array(segment.instance_ids).map((id) => text(id));
  const boundaryNodeIds = array(segment.boundary_node_ids).map((id) => text(id));
  if (typeof segment.reorderable !== 'boolean' || instanceIds.length === 0)
    invalid('invalid segment');
  unique(instanceIds);
  unique(boundaryNodeIds);
  if (segment.reorderable && (instanceIds.length < 2 || boundaryNodeIds.length > 0))
    invalid('contradictory reorder permission');
  return { instanceIds, reorderable: segment.reorderable, boundaryNodeIds };
}

/** Require finite graph coordinates without accepting numeric strings. */
function position(value: unknown): readonly [number, number] {
  const coordinates = array(value);
  const [x, y] = coordinates;
  if (
    coordinates.length !== 2 ||
    typeof x !== 'number' ||
    typeof y !== 'number' ||
    !Number.isFinite(x) ||
    !Number.isFinite(y)
  )
    invalid('invalid node position');
  return [x, y];
}

/** Narrow one dynamic object while excluding arrays at the adapter boundary. */
function record(value: unknown): UnknownRecord {
  if (!isRecord(value) || Array.isArray(value)) invalid('expected object');
  return value;
}

/** Narrow one dynamic collection without leaking unvalidated values into the application. */
function array(value: unknown): unknown[] {
  if (!Array.isArray(value)) invalid('expected array');
  return value;
}

/** Preserve user-authored text while requiring nonempty stable identifiers. */
function text(value: unknown, allowEmpty = false): string {
  if (typeof value !== 'string' || (!allowEmpty && !value.trim())) invalid('expected text');
  return value;
}

/** Accept the two native node identifier representations without boolean coercion. */
function nodeId(value: unknown): string {
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return String(value);
  return text(value);
}

/** Reject ambiguous identifiers before constructing any map or permission lookup. */
function unique(values: readonly string[]): void {
  if (new Set(values).size !== values.length) invalid('duplicate identity');
}

/** Explain invalid boundary data consistently without including workflow contents. */
function invalid(reason: string): never {
  throw new TypeError(`Cube graph response is incompatible: ${reason}.`);
}
