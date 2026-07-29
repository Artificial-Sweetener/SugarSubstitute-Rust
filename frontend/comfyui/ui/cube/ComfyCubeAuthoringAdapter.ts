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
/** Convert a native Comfy selection into a real Cube subgraph node. */

import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { ComfyGraph, GraphId, NumericVector } from '../types/graph.js';
import type { NativeCubeSubgraph } from './ComfyCubeGraphBuilder.js';
import {
  isDraftCubeNode,
  type ComfyCubeNodeFactory,
  type CubeNode,
} from './node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from './node/CubeNodeCatalog.js';

const DEFAULT_CUBE_SURFACE_SIZE: Vec2 = [720, 480];

export interface AuthoredCubeIdentity {
  cubeId: string;
  defaultAlias: string;
  instanceId: string;
  targetModel: string;
  supportedModels: string[];
  description: string;
}

/** Identify one graph-only Cube draft before it receives a persistent Cube id. */
export interface CubeDraftIdentity {
  instanceId: string;
  defaultAlias: string;
}

interface NativeConversionNode {
  id: GraphId;
  pos: NumericVector;
  size: NumericVector;
  subgraph: NativeCubeSubgraph;
  properties?: UnknownRecord;
  isSubgraphNode?: () => boolean;
}

interface NativeConversionResult {
  subgraph: NativeCubeSubgraph;
  node: NativeConversionNode;
}

interface NativeSubgraphNode extends NativeConversionNode {
  properties: UnknownRecord;
  isSubgraphNode(): boolean;
}

export interface CubeAuthoringCanvas {
  graph?: ComfyGraph;
  selectedItems: Set<unknown>;
  updateSelectedItems?(): void;
}

export interface AuthoredCube {
  node: CubeNode;
  subgraph: NativeCubeSubgraph;
  identity: AuthoredCubeIdentity;
}

/** Retain the real native subgraph used by one unsaved Cube draft. */
export interface AuthoredCubeDraft {
  node: CubeNode;
  subgraph: NativeCubeSubgraph;
  identity: CubeDraftIdentity;
}

export interface ComfyCubeAuthoringAdapterOptions {
  graph?: ComfyGraph;
  subgraphs: Map<string, NativeCubeSubgraph>;
  convertToSubgraph(items: Set<unknown>): unknown;
  canvas: CubeAuthoringCanvas;
  nodeFactory: ComfyCubeNodeFactory;
  catalog: CubeNodeCatalog;
  createEmptySubgraph?(title: string): NativeCubeSubgraph;
  getDraftPosition?(): Vec2;
}

/** Reuse native conversion and retain its generated surface node. */
export class ComfyCubeAuthoringAdapter {
  readonly #subgraphs: Map<string, NativeCubeSubgraph>;
  readonly #convertToSubgraph: (items: Set<unknown>) => unknown;
  readonly #canvas: CubeAuthoringCanvas;
  readonly #nodeFactory: ComfyCubeNodeFactory;
  readonly #catalog: CubeNodeCatalog;
  readonly #createEmptySubgraph: (title: string) => NativeCubeSubgraph;
  readonly #getDraftPosition: () => Vec2;

  /** Bind native selection conversion and Cube-node presentation metadata. */
  constructor(options: ComfyCubeAuthoringAdapterOptions) {
    this.#subgraphs = options.subgraphs;
    this.#convertToSubgraph = options.convertToSubgraph;
    this.#canvas = options.canvas;
    this.#nodeFactory = options.nodeFactory;
    this.#catalog = options.catalog;
    this.#createEmptySubgraph =
      options.createEmptySubgraph ??
      (() => {
        throw new Error('Native empty Cube draft creation is unavailable.');
      });
    this.#getDraftPosition = options.getDraftPosition ?? (() => [0, 0]);
  }

  /** Return the current native selection count. */
  selectedCount(): number {
    return this.#canvas.selectedItems.size;
  }

  /** Require a non-empty native node selection before graph mutation. */
  validateSelection(): void {
    if (this.#canvas.selectedItems.size === 0) {
      throw new Error('Select at least one node to create a SugarCube.');
    }
    const selectedIds = new Set<string>();
    for (const item of this.#canvas.selectedItems) {
      if (!isRecord(item) || item.id == null) {
        throw new Error('Select graph nodes only to create a SugarCube.');
      }
      selectedIds.add(String(item.id));
    }
    if (!selectedIds.size) throw new Error('Select graph nodes only to create a SugarCube.');
  }

  /** Create an empty graph-only Cube draft with its real native subgraph boundary. */
  createEmptyDraft(identity: CubeDraftIdentity): AuthoredCubeDraft {
    const subgraph = this.#createEmptySubgraph(`Cube: ${identity.defaultAlias}`);
    const metadata = buildDraftMetadata(identity);
    markSubgraphAsDraft(subgraph, metadata);
    const node = this.#nodeFactory.create({
      instanceId: identity.instanceId,
      subgraph,
      title: identity.defaultAlias,
      position: this.#getDraftPosition(),
      size: DEFAULT_CUBE_SURFACE_SIZE,
      identity: metadata,
      surface: {},
      kind: 'draft',
    });
    this.#subgraphs.set(subgraph.id, subgraph);
    this.#catalog.add(node);
    return { node, subgraph, identity };
  }

  /** Convert through Comfy while retaining its authoritative native boundary links. */
  createDraftFromSelection(identity: CubeDraftIdentity): AuthoredCubeDraft {
    this.validateSelection();
    const converted = readConversionResult(
      this.#convertToSubgraph(new Set(this.#canvas.selectedItems)),
    );
    return this.#finalizeDraft(converted.node, identity);
  }

  /** Require exactly one unconverted native subgraph. */
  validateSelectedSubgraph(): void {
    readSelectedSubgraphNode(this.#canvas.selectedItems);
  }

  /** Mark one existing native subgraph as a graph-only draft without changing its interface. */
  createDraftFromSelectedSubgraph(identity: CubeDraftIdentity): AuthoredCubeDraft {
    const node = readSelectedSubgraphNode(this.#canvas.selectedItems);
    return this.#finalizeDraft(node, identity);
  }

  /** Promote exactly one saved draft without rebuilding its internal native subgraph. */
  promoteDraft(instanceId: string, identity: AuthoredCubeIdentity): AuthoredCube {
    const node = this.#catalog.get(instanceId);
    if (!node || !isDraftCubeNode(node)) {
      throw new Error('The selected Cube draft is no longer available.');
    }
    const metadata = buildCubeMetadata(identity);
    node.subgraph.name = `Cube: ${identity.defaultAlias}`;
    node.subgraph.extra = {
      ...(isRecord(node.subgraph.extra) ? node.subgraph.extra : {}),
      sugarcubes_kind: 'cube',
      sugarcubes_cube: cloneRecord(metadata),
    };
    this.#nodeFactory.adopt(node, {
      instanceId: identity.instanceId,
      subgraph: node.subgraph,
      title: identity.defaultAlias,
      position: readPair(node.pos, [0, 0]),
      size: initialCubeSurfaceSize(node.size),
      identity: metadata,
      surface: {},
    });
    this.#catalog.changed(node);
    return { node, subgraph: node.subgraph, identity };
  }

  /** Restore a failed first-save promotion to its workflow-only draft state. */
  restoreDraft(instanceId: string): void {
    const node = this.#catalog.get(instanceId);
    if (!node) return;
    const identity: CubeDraftIdentity = {
      instanceId,
      defaultAlias: node.title?.trim() || 'Untitled Cube',
    };
    const metadata = buildDraftMetadata(identity);
    markSubgraphAsDraft(node.subgraph, metadata);
    this.#nodeFactory.adopt(node, {
      instanceId,
      subgraph: node.subgraph,
      title: identity.defaultAlias,
      position: readPair(node.pos, [0, 0]),
      size: initialCubeSurfaceSize(node.size),
      identity: metadata,
      surface: {},
      kind: 'draft',
    });
    this.#catalog.changed(node);
  }

  /** Apply Cube identity and presentation metadata to a native subgraph node. */
  #finalizeDraft(nativeNode: NativeConversionNode, identity: CubeDraftIdentity): AuthoredCubeDraft {
    const metadata = buildDraftMetadata(identity);
    nativeNode.subgraph.name = `Cube: ${identity.defaultAlias}`;
    markSubgraphAsDraft(nativeNode.subgraph, metadata);
    const node = this.#nodeFactory.adopt(nativeNode, {
      instanceId: identity.instanceId,
      subgraph: nativeNode.subgraph,
      title: identity.defaultAlias,
      position: readPair(nativeNode.pos, [0, 0]),
      size: initialCubeSurfaceSize(nativeNode.size),
      identity: metadata,
      surface: {},
      kind: 'draft',
    });

    this.#subgraphs.set(nativeNode.subgraph.id, nativeNode.subgraph);
    this.#catalog.add(node);
    this.#canvas.selectedItems.clear();
    this.#canvas.updateSelectedItems?.();
    return { node, subgraph: nativeNode.subgraph, identity };
  }
}

/** Persist the draft marker where both native Comfy renderers can read it. */
function markSubgraphAsDraft(subgraph: NativeCubeSubgraph, metadata: UnknownRecord): void {
  subgraph.extra = {
    ...(isRecord(subgraph.extra) ? subgraph.extra : {}),
    sugarcubes_kind: 'cube_draft',
    sugarcubes_cube: cloneRecord(metadata),
  };
}

/** Infer editor-local boundaries when Comfy stores them only on native node slots. */

/** Validate Comfy's native selection-to-subgraph result. */
function readConversionResult(value: unknown): NativeConversionResult {
  if (!isRecord(value) || !isNativeSubgraph(value.subgraph) || !isConversionNode(value.node)) {
    throw new TypeError('Comfy did not return a valid native subgraph conversion.');
  }
  if (value.node.subgraph !== value.subgraph) {
    throw new TypeError('Comfy returned a wrapper for a different subgraph definition.');
  }
  return { subgraph: value.subgraph, node: value.node };
}

/** Validate the one selected native subgraph node that will be promoted in place. */
function readSelectedSubgraphNode(items: Set<unknown>): NativeConversionNode {
  if (items.size !== 1) {
    throw new Error('Select exactly one subgraph node to convert it to a SugarCube.');
  }
  const selected = items.values().next().value;
  if (!isNativeSubgraphNode(selected)) {
    throw new Error('Select a native Comfy subgraph node to convert it to a SugarCube.');
  }
  if (
    selected.properties.sugarcubes_kind === 'cube' ||
    selected.properties.sugarcubes_kind === 'cube_draft'
  ) {
    throw new Error('The selected subgraph is already a SugarCube.');
  }
  if (selected.isSubgraphNode.call(selected) !== true) {
    throw new Error('Select a native Comfy subgraph node to convert it to a SugarCube.');
  }
  if (!Array.isArray(selected.subgraph.inputs) || !Array.isArray(selected.subgraph.outputs)) {
    throw new Error('The selected subgraph does not expose native input and output boundaries.');
  }
  return selected;
}

/** Narrow a selected host value to the native subgraph members used for promotion. */
function isNativeSubgraphNode(value: unknown): value is NativeSubgraphNode {
  return (
    isConversionNode(value) &&
    isRecord(value.properties) &&
    typeof value.isSubgraphNode === 'function'
  );
}

/** Validate the native subgraph members used after conversion. */
function isNativeSubgraph(value: unknown): value is NativeCubeSubgraph {
  return (
    isRecord(value) &&
    typeof value.id === 'string' &&
    typeof value.name === 'string' &&
    Array.isArray(value._nodes) &&
    isRecord(value.inputNode) &&
    isRecord(value.outputNode)
  );
}

/** Validate only the disposable native wrapper data used during conversion. */
function isConversionNode(value: unknown): value is NativeConversionNode {
  return (
    isRecord(value) &&
    value.id != null &&
    isRecord(value.subgraph) &&
    isNumericVector(value.pos) &&
    isNumericVector(value.size)
  );
}

/** Choose a useful first parent size independently of the native wrapper. */
function initialCubeSurfaceSize(value: NumericVector): Vec2 {
  const [width, height] = readPair(value, DEFAULT_CUBE_SURFACE_SIZE);
  return [
    Math.max(DEFAULT_CUBE_SURFACE_SIZE[0], width),
    Math.max(DEFAULT_CUBE_SURFACE_SIZE[1], height),
  ];
}

/** Read one finite pair from a host-owned vector. */
function readPair(value: ArrayLike<number>, fallback: Vec2): Vec2 {
  const x = Number(value[0]);
  const y = Number(value[1]);
  return [Number.isFinite(x) ? x : fallback[0], Number.isFinite(y) ? y : fallback[1]];
}

/** Narrow an unknown host value to a numeric two-dimensional vector. */
function isNumericVector(value: unknown): value is NumericVector {
  return (
    (Array.isArray(value) || value instanceof Float32Array || value instanceof Float64Array) &&
    value.length >= 2
  );
}

/** Build authoritative persisted Cube identity. */
function buildCubeMetadata(identity: AuthoredCubeIdentity): UnknownRecord {
  return {
    schema: 1,
    kind: 'cube',
    cube_id: identity.cubeId,
    cube_version: '',
    instance_id: identity.instanceId,
    default_alias: identity.defaultAlias,
    instance_alias: identity.defaultAlias,
    description: identity.description,
    ...(identity.targetModel ? { target_model: identity.targetModel } : {}),
    ...(identity.supportedModels.length ? { supported_models: [...identity.supportedModels] } : {}),
  };
}

/** Build workflow-only draft metadata with no persistent Cube id. */
function buildDraftMetadata(identity: CubeDraftIdentity): UnknownRecord {
  return {
    schema: 1,
    kind: 'draft',
    instance_id: identity.instanceId,
    default_alias: identity.defaultAlias,
    instance_alias: identity.defaultAlias,
  };
}

/** Clone JSON-safe metadata before assigning domain ownership. */
function cloneRecord(value: UnknownRecord): UnknownRecord {
  const parsed: unknown = JSON.parse(JSON.stringify(value));
  return isRecord(parsed) ? parsed : {};
}
