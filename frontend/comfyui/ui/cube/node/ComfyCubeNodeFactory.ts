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
/** Create and identify real Comfy subgraph nodes presented as SugarCubes. */

import { isRecord } from '../../types/common.js';
import type { UnknownRecord, Vec2 } from '../../types/common.js';
import type { NativeCubeSubgraph, NativeGraphNode } from '../ComfyCubeGraphBuilder.js';

const DEFAULT_SIZE: Vec2 = [720, 480];
const MINIMUM_SIZE: Vec2 = [320, 180];

/** Describe the native node lifecycle SugarCubes decorates with a Cube face. */
export interface CubeNode extends NativeGraphNode {
  subgraph: NativeCubeSubgraph;
  isSubgraphNode(): boolean;
  serialize(): unknown;
}

export interface CubeNodeConfiguration {
  subgraph: NativeCubeSubgraph;
  instanceId: string;
  title: string;
  position: Vec2;
  size: Vec2;
  identity: UnknownRecord;
  surface: UnknownRecord;
  kind?: 'cube' | 'draft';
}

export interface ComfyCubeNodeFactoryOptions {
  createNode(type: string): NativeGraphNode | null;
}

/** Own the narrow integration boundary between Cube semantics and native nodes. */
export class ComfyCubeNodeFactory {
  readonly #createNode: (type: string) => NativeGraphNode | null;

  /** Bind native node construction without taking graph insertion ownership. */
  constructor(options: ComfyCubeNodeFactoryOptions) {
    this.#createNode = options.createNode;
  }

  /** Create one detached registered subgraph instance as a real native node. */
  create(configuration: CubeNodeConfiguration): CubeNode {
    const candidate = this.#createNode(configuration.subgraph.id);
    const node = requireSubgraphNode(candidate, configuration.subgraph);
    node.id = requireIdentifier(configuration.instanceId);
    this.#configure(node, configuration);
    return node;
  }

  /** Decorate Comfy's already-added selection-conversion node in place. */
  adopt(candidate: unknown, configuration: CubeNodeConfiguration): CubeNode {
    const node = requireSubgraphNode(candidate, configuration.subgraph);
    this.#configure(node, configuration);
    return node;
  }

  /** Apply Cube-owned presentation metadata without replacing native lifecycle state. */
  #configure(node: CubeNode, configuration: CubeNodeConfiguration): void {
    const instanceId = requireIdentifier(configuration.instanceId);
    const identity = cloneRecord(configuration.identity);
    identity.instance_id = instanceId;
    node.title = configuration.title.trim() || configuration.subgraph.name;
    writePair(node.pos, finitePair(configuration.position, [0, 0]));
    const size = constrainedSize(configuration.size);
    node.setSize?.([...size]);
    writePair(node.size, size);
    const kind = configuration.kind === 'draft' ? 'cube_draft' : 'cube';
    node.properties = {
      ...node.properties,
      sugarcubes_kind: kind,
      sugarcubes_cube: identity,
      sugarcubes_surface: cloneRecord(configuration.surface),
    };
  }
}

/** Identify a SugarCube by its durable marker and real native subgraph lifecycle. */
export function isCubeNode(value: unknown): value is CubeNode {
  if (!isRecord(value) || !isRecord(value.properties)) return false;
  return (
    !isSugarMarkedBlueprintNode(value) &&
    isCubeKind(value.properties.sugarcubes_kind) &&
    typeof value.isSubgraphNode === 'function' &&
    value.isSubgraphNode.call(value) === true &&
    isNativeSubgraph(value.subgraph) &&
    isNumericVector(value.pos) &&
    isNumericVector(value.size) &&
    Array.isArray(value.inputs) &&
    Array.isArray(value.outputs) &&
    typeof value.connect === 'function' &&
    typeof value.serialize === 'function'
  );
}

/** Identify a Cube before native graph insertion, including unconfigured copied wrappers. */
export function isCubePlacementCandidate(value: unknown): boolean {
  if (!isRecord(value) || isSugarMarkedBlueprintNode(value)) return false;
  const isSubgraphNode = value.isSubgraphNode;
  if (typeof isSubgraphNode !== 'function' || isSubgraphNode.call(value) !== true) return false;
  if (isRecord(value.properties) && isCubeKind(value.properties.sugarcubes_kind)) return true;
  return (
    isRecord(value.subgraph) &&
    isRecord(value.subgraph.extra) &&
    isCubeKind(value.subgraph.extra.sugarcubes_kind)
  );
}

/** Detect a legacy Blueprint wrapper that retained Sugar markers after native publish. */
export function isSugarMarkedBlueprintNode(value: unknown): boolean {
  if (!isRecord(value) || !isRecord(value.properties)) return false;
  const constructor = Reflect.get(value, 'constructor');
  const comfyClass =
    (typeof constructor === 'object' && constructor !== null) || typeof constructor === 'function'
      ? Reflect.get(constructor, 'comfyClass')
      : undefined;
  return (
    typeof comfyClass === 'string' &&
    comfyClass.startsWith('SubgraphBlueprint.') &&
    isCubeKind(value.properties.sugarcubes_kind)
  );
}

/** Return whether the native Cube face represents a workflow-only draft. */
export function isDraftCubeNode(value: unknown): value is CubeNode {
  return isCubeNode(value) && value.properties.sugarcubes_kind === 'cube_draft';
}

/** Return the mutable face state serialized with one native Cube node. */
export function requireCubeSurface(node: CubeNode): UnknownRecord {
  const existing = node.properties.sugarcubes_surface;
  if (isRecord(existing)) return existing;
  const surface: UnknownRecord = {};
  node.properties.sugarcubes_surface = surface;
  return surface;
}

/** Return the validated per-instance identity serialized with one Cube node. */
export function requireCubeIdentity(node: CubeNode): UnknownRecord {
  const identity = node.properties.sugarcubes_cube;
  if (!isRecord(identity)) {
    throw new TypeError(`Cube node '${String(node.id)}' is missing its instance identity.`);
  }
  return identity;
}

/** Validate a newly created or converted native SubgraphNode. */
function requireSubgraphNode(value: unknown, expectedSubgraph: NativeCubeSubgraph): CubeNode {
  if (
    !isRecord(value) ||
    typeof value.isSubgraphNode !== 'function' ||
    value.isSubgraphNode.call(value) !== true ||
    value.subgraph !== expectedSubgraph ||
    !isNumericVector(value.pos) ||
    !isNumericVector(value.size) ||
    !Array.isArray(value.inputs) ||
    !Array.isArray(value.outputs) ||
    !isRecord(value.properties) ||
    typeof value.connect !== 'function' ||
    typeof value.serialize !== 'function'
  ) {
    throw new TypeError('Comfy did not provide a real Comfy subgraph node for the Cube.');
  }
  return value as unknown as CubeNode;
}

/** Validate the subgraph definition retained by the native node. */
function isNativeSubgraph(value: unknown): value is NativeCubeSubgraph {
  return (
    isRecord(value) &&
    typeof value.id === 'string' &&
    typeof value.name === 'string' &&
    Array.isArray(value._nodes) &&
    Array.isArray(value.inputs) &&
    Array.isArray(value.outputs)
  );
}

/** Normalize one stable Cube instance identity. */
function requireIdentifier(value: string): string {
  const normalized = value.trim();
  if (!normalized) throw new TypeError('Cube instance identity is required.');
  return normalized;
}

/** Normalize one finite pair without retaining caller-owned arrays. */
function finitePair(value: Vec2, fallback: Vec2): Vec2 {
  const x = Number(value[0]);
  const y = Number(value[1]);
  return [Number.isFinite(x) ? x : fallback[0], Number.isFinite(y) ? y : fallback[1]];
}

/** Enforce the minimum finite Cube card size. */
function constrainedSize(value: Vec2): Vec2 {
  const [width, height] = finitePair(value, DEFAULT_SIZE);
  return [Math.max(MINIMUM_SIZE[0], width), Math.max(MINIMUM_SIZE[1], height)];
}

/** Write host-owned vectors without replacing their runtime representation. */
function writePair(target: ArrayLike<number> & Record<number, number>, source: Vec2): void {
  target[0] = source[0];
  target[1] = source[1];
}

/** Clone JSON-safe state before assigning native-node persistence ownership. */
function cloneRecord(value: UnknownRecord): UnknownRecord {
  const parsed: unknown = JSON.parse(JSON.stringify(value));
  return isRecord(parsed) ? parsed : {};
}

/** Accept LiteGraph's mutable array and typed-array geometry vectors. */
function isNumericVector(value: unknown): value is number[] | Float32Array | Float64Array {
  return Array.isArray(value) || value instanceof Float32Array || value instanceof Float64Array;
}

/** Recognize both persisted Cubes and workflow-only draft Cube faces. */
function isCubeKind(value: unknown): boolean {
  return value === 'cube' || value === 'cube_draft';
}
