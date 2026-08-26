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
/** Construct detached native Cube nodes from prepared imports. */

import type { ImportPayload } from '../import/PlacementPayload.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type {
  BuiltCubeGraph,
  ComfyCubeGraphBuilder,
  NativeCubeSubgraph,
} from './ComfyCubeGraphBuilder.js';
import type { ComfyCubeNodeFactory, CubeNode } from './node/ComfyCubeNodeFactory.js';
import { writeCubeDefinitionIdentity } from './node/CubeDefinitionIdentityWriter.js';
import { writeCubeDefinitionDocument } from '../workflow/CubeDefinitionDocumentWriter.js';
import type { CubeInitialSizeResolver } from './geometry/CubeInitialSizePolicy.js';

const MINIMUM_CUBE_WIDTH = 240;
const MINIMUM_CUBE_HEIGHT = 160;

export interface CubeConstructionOptions {
  instanceAlias?: string;
  instanceId?: string;
  position?: Vec2;
  revisionRef?: string;
  size?: Vec2;
  surface?: UnknownRecord;
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

export interface BuiltCubeConstruction {
  built: BuiltCubeGraph;
  title: string;
  identity: CubeIdentity;
  geometry: CubeParentGeometry;
  document?: UnknownRecord;
}

export interface ConstructedCube {
  node: CubeNode;
  subgraph: NativeCubeSubgraph;
  warnings: string[];
  internalNodeCount: number;
}

export interface CubeDefinitionRollback {
  discard(subgraph: NativeCubeSubgraph): void;
}

export interface CubeConstructionServiceOptions {
  graphBuilder: ComfyCubeGraphBuilder;
  nodeFactory: ComfyCubeNodeFactory;
  definitions: CubeDefinitionRollback;
  resolveInitialSize: CubeInitialSizeResolver;
  createInstanceId(): string;
}

/** Own graph assembly, instance identity, metadata, and detached node construction. */
export class CubeConstructionService {
  readonly #graphBuilder: ComfyCubeGraphBuilder;
  readonly #nodeFactory: ComfyCubeNodeFactory;
  readonly #definitions: CubeDefinitionRollback;
  readonly #resolveInitialSize: CubeInitialSizeResolver;
  readonly #createInstanceId: () => string;

  /** Bind focused native construction collaborators and rollback ownership. */
  constructor(options: CubeConstructionServiceOptions) {
    this.#graphBuilder = options.graphBuilder;
    this.#nodeFactory = options.nodeFactory;
    this.#definitions = options.definitions;
    this.#resolveInitialSize = options.resolveInitialSize;
    this.#createInstanceId = options.createInstanceId;
  }

  /** Build one fresh definition and return its detached configured native node. */
  construct(payload: ImportPayload, options: CubeConstructionOptions = {}): ConstructedCube {
    const title = resolveTitle(payload, options.instanceAlias);
    const built = this.#graphBuilder.build(payload, `Cube: ${title}`);
    const identity = readPayloadIdentity(
      payload,
      options.instanceId ?? this.#createInstanceId(),
      options.instanceAlias,
      options.surface,
      options.revisionRef,
    );
    return this.constructBuilt({
      built,
      title,
      identity,
      geometry: {
        position: readPayloadPosition(payload, options.position),
        size:
          options.size ??
          this.#resolveInitialSize({
            surface: readSurfaceState(identity.metadata),
            hasInputs: built.subgraph.inputs.length > 0,
          }),
      },
      ...(isRecord(payload.document) ? { document: payload.document } : {}),
    });
  }

  /** Configure one prebuilt definition and return a detached real Cube node. */
  constructBuilt(request: BuiltCubeConstruction): ConstructedCube {
    const metadata = buildInstanceMetadata(request.identity);
    writeCubeDefinitionIdentity(request.built.subgraph, 'cube', metadata);
    writeCubeDefinitionDocument(request.built.subgraph, request.document, {
      cubeId: request.identity.cubeId,
      cubeVersion: request.identity.cubeVersion,
    });
    try {
      const node = this.#nodeFactory.create({
        instanceId: requireInstanceId(request.identity.instanceId),
        subgraph: request.built.subgraph,
        title: request.title,
        position: request.geometry.position,
        size: normalizedSize(request.geometry.size),
        identity: metadata,
        surface: readSurfaceState(request.identity.metadata),
      });
      return {
        node,
        subgraph: request.built.subgraph,
        warnings: request.built.warnings,
        internalNodeCount: request.built.nodesBySymbol.size,
      };
    } catch (error: unknown) {
      this.#definitions.discard(request.built.subgraph);
      throw error;
    }
  }

  /** Discard a detached construction that will not enter the root graph. */
  discard(constructed: ConstructedCube): void {
    this.#definitions.discard(constructed.subgraph);
  }
}

/** Parse stable instance identity at the prepared-import boundary. */
function readPayloadIdentity(
  payload: ImportPayload,
  instanceId: string,
  instanceAlias?: string,
  surface?: UnknownRecord,
  revisionRef?: string,
): CubeIdentity {
  const cube = isRecord(payload.cube) ? payload.cube : {};
  const cubeMetadata = isRecord(cube.metadata) ? cube.metadata : {};
  return {
    cubeId: readString(cube.cube_id),
    cubeVersion: readString(cube.version),
    instanceId: requireInstanceId(instanceId),
    defaultAlias: readString(cube.default_alias),
    instanceAlias: readString(instanceAlias) || readString(cube.default_alias),
    metadata: cloneRecord({
      ...cube,
      ...cubeMetadata,
      ...(surface ? { surface_state: surface } : {}),
      ...(revisionRef ? { cube_revision_ref: revisionRef } : {}),
    }),
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

/** Read a finite fresh-placement origin without inheriting authored model-space size. */
function readPayloadPosition(payload: ImportPayload, explicitPosition?: Vec2): Vec2 {
  return readPair(explicitPosition ?? payload.layout?.origin, [0, 0]);
}

/** Enforce the minimum finite Cube frame size. */
function normalizedSize(size: Vec2): Vec2 {
  return [
    Math.max(MINIMUM_CUBE_WIDTH, Number(size[0]) || 0),
    Math.max(MINIMUM_CUBE_HEIGHT, Number(size[1]) || 0),
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
