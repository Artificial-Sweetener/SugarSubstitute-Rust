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
/** Place imported Cube definitions as real native subgraph nodes. */

import type { ImportPayload } from '../import/PlacementPayload.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type {
  BuiltCubeGraph,
  ComfyCubeGraphBuilder,
  NativeCubeSubgraph,
} from './ComfyCubeGraphBuilder.js';
import type { ComfyCubeNodeFactory, CubeNode } from './node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from './node/CubeNodeCatalog.js';

const MINIMUM_CUBE_WIDTH = 240;
const MINIMUM_CUBE_HEIGHT = 160;

export interface CubePlacementHistory {
  beforeChange?(): void;
  afterChange?(): void;
  setDirtyCanvas?(foreground?: boolean, background?: boolean): void;
}

export interface CubePlacementOptions {
  instanceAlias?: string;
  position?: Vec2;
}

export interface CubeIdentity {
  cubeId: string;
  cubeVersion: string;
  instanceId: string;
  defaultAlias: string;
  instanceAlias: string;
  metadata: UnknownRecord;
}

export interface CubeParentGeometry {
  position: Vec2;
  size: Vec2;
}

export interface BuiltCubePlacement {
  built: BuiltCubeGraph;
  title: string;
  identity: CubeIdentity;
  geometry: CubeParentGeometry;
  recordHistory: boolean;
}

export interface PlacedCube {
  node: CubeNode;
  subgraph: NativeCubeSubgraph;
  warnings: string[];
  internalNodeCount: number;
}

export interface CubePlacementServiceOptions {
  graphBuilder: ComfyCubeGraphBuilder;
  nodeFactory: ComfyCubeNodeFactory;
  catalog: CubeNodeCatalog;
  history: CubePlacementHistory;
}

/** Own imported Cube placement as one native root-graph node. */
export class CubePlacementService {
  readonly #graphBuilder: ComfyCubeGraphBuilder;
  readonly #nodeFactory: ComfyCubeNodeFactory;
  readonly #catalog: CubeNodeCatalog;
  readonly #history: CubePlacementHistory;

  /** Bind graph construction, native node creation, and host history boundaries. */
  constructor(options: CubePlacementServiceOptions) {
    this.#graphBuilder = options.graphBuilder;
    this.#nodeFactory = options.nodeFactory;
    this.#catalog = options.catalog;
    this.#history = options.history;
  }

  /** Build and place one imported Cube definition. */
  place(payload: ImportPayload, options: CubePlacementOptions = {}): PlacedCube {
    const title = resolveTitle(payload, options.instanceAlias);
    const built = this.#graphBuilder.build(payload, `Cube: ${title}`);
    return this.placeBuilt({
      built,
      title,
      identity: readPayloadIdentity(payload, options.instanceAlias),
      geometry: readPayloadGeometry(payload, options.position),
      recordHistory: true,
    });
  }

  /** Place one prebuilt native definition as a real Cube node. */
  placeBuilt(request: BuiltCubePlacement): PlacedCube {
    if (request.recordHistory) this.#history.beforeChange?.();
    try {
      const metadata = buildInstanceMetadata(request.identity);
      writeDefinitionMetadata(request.built, metadata);
      const node = this.#nodeFactory.create({
        instanceId: requireInstanceId(request.identity.instanceId),
        subgraph: request.built.subgraph,
        title: request.title,
        position: request.geometry.position,
        size: normalizedSize(request.geometry.size),
        identity: metadata,
        surface: readSurfaceState(request.identity.metadata),
      });
      this.#catalog.add(node);
      this.#history.setDirtyCanvas?.(true, true);
      return {
        node,
        subgraph: request.built.subgraph,
        warnings: request.built.warnings,
        internalNodeCount: request.built.nodesBySymbol.size,
      };
    } finally {
      if (request.recordHistory) this.#history.afterChange?.();
    }
  }
}

/** Persist definition identity alongside Comfy's serialized subgraph data. */
function writeDefinitionMetadata(built: BuiltCubeGraph, metadata: UnknownRecord): void {
  built.subgraph.extra = {
    ...(isRecord(built.subgraph.extra) ? built.subgraph.extra : {}),
    sugarcubes_cube: cloneRecord(metadata),
    sugarcubes_kind: 'cube',
  };
}

/** Parse stable instance identity at the prepared import boundary. */
function readPayloadIdentity(payload: ImportPayload, instanceAlias?: string): CubeIdentity {
  const cube = isRecord(payload.cube) ? payload.cube : {};
  const cubeMetadata = isRecord(cube.metadata) ? cube.metadata : {};
  return {
    cubeId: readString(cube.cube_id),
    cubeVersion: readString(cube.version),
    instanceId: createUuid(),
    defaultAlias: readString(cube.default_alias),
    instanceAlias: readString(instanceAlias) || readString(cube.default_alias),
    metadata: cloneRecord({ ...cube, ...cubeMetadata }),
  };
}

/** Build persisted metadata from one validated Cube identity. */
function buildInstanceMetadata(identity: CubeIdentity): UnknownRecord {
  return {
    ...identity.metadata,
    schema: 1,
    kind: 'cube',
    cube_id: identity.cubeId,
    cube_version: identity.cubeVersion,
    instance_id: requireInstanceId(identity.instanceId),
    default_alias: identity.defaultAlias,
    instance_alias: identity.instanceAlias,
  };
}

/** Read the optional persisted Cube face state. */
function readSurfaceState(metadata: UnknownRecord): UnknownRecord {
  return isRecord(metadata.surface_state) ? cloneRecord(metadata.surface_state) : {};
}

/** Clone JSON-safe metadata before assigning instance ownership. */
function cloneRecord(value: UnknownRecord): UnknownRecord {
  const parsed: unknown = JSON.parse(JSON.stringify(value));
  return isRecord(parsed) ? parsed : {};
}

/** Resolve one visible instance title. */
function resolveTitle(payload: ImportPayload, instanceAlias?: string): string {
  const cube = isRecord(payload.cube) ? payload.cube : {};
  return (
    readString(instanceAlias) ||
    readString(cube.default_alias) ||
    readString(payload.default_alias) ||
    'SugarCube'
  );
}

/** Set finite parent geometry from placement and authored Cube bounds. */
function readPayloadGeometry(payload: ImportPayload, explicitPosition?: Vec2): CubeParentGeometry {
  const origin = readPair(explicitPosition ?? payload.layout?.origin, [0, 0]);
  const cube = isRecord(payload.cube) ? payload.cube : {};
  const metadata = isRecord(cube.metadata) ? cube.metadata : {};
  const surfaceSize = readPairSize(metadata.surface_size, [0, 0]);
  const authoredGroup = payload.layout?.groups?.find((group) => isRecord(group.sugarcubes));
  const size =
    surfaceSize[0] > 0 && surfaceSize[1] > 0
      ? surfaceSize
      : readPairSize(authoredGroup?.bounding, [900, 600]);
  return { position: origin, size: normalizedSize(size) };
}

/** Enforce the minimum finite Cube frame size. */
function normalizedSize(size: Vec2): Vec2 {
  return [
    Math.max(MINIMUM_CUBE_WIDTH, Number(size[0]) || 0),
    Math.max(MINIMUM_CUBE_HEIGHT, Number(size[1]) || 0),
  ];
}

/** Read width and height from a legacy bounds record. */
function readPairSize(value: unknown, fallback: Vec2): Vec2 {
  if (!Array.isArray(value)) return fallback;
  const offset = value.length >= 4 ? 2 : 0;
  const width = Number(value[offset]);
  const height = Number(value[offset + 1]);
  return [
    Number.isFinite(width) && width > 0 ? width : fallback[0],
    Number.isFinite(height) && height > 0 ? height : fallback[1],
  ];
}

/** Read one finite position pair. */
function readPair(value: unknown, fallback: Vec2): Vec2 {
  if (!Array.isArray(value)) return fallback;
  const x = Number(value[0]);
  const y = Number(value[1]);
  return [Number.isFinite(x) ? x : fallback[0], Number.isFinite(y) ? y : fallback[1]];
}

/** Require a stable extension-owned instance identity. */
function requireInstanceId(value: string): string {
  const normalized = value.trim();
  if (!normalized) throw new Error('Cube instance identity is required.');
  return normalized;
}

/** Read a trimmed metadata string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Create a host-independent stable instance identity. */
function createUuid(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') return globalThis.crypto.randomUUID();
  return `sugarcube-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}
