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
/** Build a Cube's real internal Comfy subgraph without root-graph projections. */

import {
  applyExecutionMode,
  applyExtrasToNode,
  applyInputValueToNode,
  resolveInputSlotIndex,
} from '../import/ImportNodeWriter.js';
import type { ImportPayload } from '../import/PlacementPayload.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type {
  ComfyGraph,
  ComfyInput,
  ComfyNode,
  ComfyOutput,
  NumericVector,
} from '../types/graph.js';
import { buildCubePayloadTopology } from './CubePayloadTopology.js';
import { resolveCubeInputBoundaryType } from './CubeBoundaryTypeResolver.js';
import { deriveCubeOutputSurfaceNames } from './CubeOutputSurfaceNames.js';
import { resolveCubeDefinitionDescription } from './node/CubeDefinitionIdentityWriter.js';
import { initializePromotedWidgetIdentity } from './node/PromotedWidgetIdentityInitializer.js';
import { attachNativeCubeAuthoredLayout } from './geometry/NativeCubeAuthoredLayout.js';
import {
  applyEmptyCubeBoundaryLayout,
  applyNativeCubeBoundaryLayout,
  attachNativeCubeBoundaryLayout,
  EMPTY_CUBE_INPUT_POSITION,
  EMPTY_CUBE_OUTPUT_POSITION,
  labelEmptyCubeBoundaryAffordances,
} from './geometry/NativeCubeBoundaryLayout.js';

interface NativeBoundaryInput {
  connect(slot: ComfyInput, node: NativeGraphNode): unknown;
}

interface NativeBoundaryOutput {
  connect(slot: ComfyOutput, node: NativeGraphNode): unknown;
}

interface NativeBoundaryNode {
  bounding?: NumericVector;
  boundingRect?: NumericVector;
  emptySlot?: NativeBoundarySlot & { name: string };
  pos?: NumericVector;
  size?: NumericVector;
  slots?: NativeBoundarySlot[];
  arrange?(): void;
}

interface NativeBoundarySlot {
  boundingRect?: NumericVector;
  linkIds?: unknown[];
  pos?: NumericVector;
}

export interface NativeGraphNode extends ComfyNode {
  id: string | number;
  pos: NumericVector;
  size: NumericVector;
  properties: UnknownRecord;
  inputs: ComfyInput[];
  outputs: ComfyOutput[];
  connect(outputSlot: number, targetNode: NativeGraphNode, inputSlot: number): unknown;
}

export interface NativeCubeSubgraph extends ComfyGraph {
  id: string;
  name: string;
  extra?: UnknownRecord;
  _nodes: NativeGraphNode[];
  inputs: Array<{ name: string; type: string }>;
  outputs: Array<{ name: string; type: string }>;
  onAfterChange?: ((graph: unknown, info?: unknown) => void) | null;
  inputNode: NativeBoundaryNode;
  outputNode: NativeBoundaryNode;
  add(node: NativeGraphNode): void;
  remove(node: NativeGraphNode): void;
  addInput(name: string, type: string): NativeBoundaryInput;
  addOutput(name: string, type: string): NativeBoundaryOutput;
  configure(data: UnknownRecord): unknown;
  asSerialisable?(): UnknownRecord;
}

export interface CubeGraphBuilderHost {
  rootGraph: {
    createSubgraph(data: UnknownRecord): NativeCubeSubgraph;
  };
  createNode(type: string): NativeGraphNode | null;
  createUuid(): string;
  discardSubgraph?(subgraph: NativeCubeSubgraph): void;
}

export interface BuiltCubeGraph {
  subgraph: NativeCubeSubgraph;
  nodesBySymbol: ReadonlyMap<string, NativeGraphNode>;
  warnings: string[];
}

/** Own translation from a prepared Cube import to one native Comfy subgraph. */
export class ComfyCubeGraphBuilder {
  readonly #host: CubeGraphBuilderHost;

  /** Bind the narrow Comfy graph capabilities required by Cube construction. */
  constructor(host: CubeGraphBuilderHost) {
    this.#host = host;
  }

  /** Register a blank native definition for importing an already-defined Cube. */
  createEmptyDefinition(title: string, description?: string): NativeCubeSubgraph {
    return this.#createSubgraph(title, description);
  }

  /** Register an authoring draft whose editor creates bindings only when wired. */
  createEmptyDraft(title: string, description?: string): NativeCubeSubgraph {
    const subgraph = this.#createSubgraph(title, description);
    labelEmptyCubeBoundaryAffordances(subgraph);
    applyEmptyCubeBoundaryLayout(subgraph);
    return subgraph;
  }

  /** Build and connect real nodes wholly inside a registered Comfy subgraph. */
  build(payload: ImportPayload, title: string): BuiltCubeGraph {
    const topology = buildCubePayloadTopology(payload);
    const subgraph = this.createEmptyDefinition(title, readCubeDescription(payload));
    try {
      const nodesBySymbol = new Map<string, NativeGraphNode>();
      const warnings: string[] = [];
      const origin = readOrigin(payload);
      attachNativeCubeAuthoredLayout(subgraph, payload);
      const inputBoundaryNodes = new Set<NativeGraphNode>();
      const outputBoundaryNodes = new Set<NativeGraphNode>();
      const linkedInputs = new Set(
        topology.nodeConnections.map(
          ({ targetSymbol, targetInput }) => `${targetSymbol}\u0000${targetInput}`,
        ),
      );
      for (const binding of topology.inputs) {
        for (const target of binding.targets) {
          linkedInputs.add(`${target.symbol}\u0000${target.input}`);
        }
      }

      for (const entry of topology.nodes) {
        const symbol = readString(entry.symbol);
        const classType = readString(entry.class_type);
        if (!symbol || !classType) {
          warnings.push('Cube node entry is missing symbol or class_type.');
          continue;
        }
        const node = this.#host.createNode(classType);
        if (!node) {
          warnings.push(`Cube node type '${classType}' is unavailable.`);
          continue;
        }
        node.id = this.#host.createUuid();
        initializePromotedWidgetIdentity(node);
        applyAuthoredGeometry(node, entry.layout, origin);
        node.properties.sugarcubes_symbol = symbol;
        const titleValue = readString(entry.layout?.title);
        if (titleValue) node.title = titleValue;
        subgraph.add(node);
        nodesBySymbol.set(symbol, node);
        applyExecutionMode(node, entry.mode ?? entry.extras?.mode);

        for (const [inputName, value] of Object.entries(entry.inputs ?? {})) {
          if (!linkedInputs.has(`${symbol}\u0000${inputName}`)) {
            applyInputValueToNode(node, inputName, value);
          }
        }
        if (entry.extras) applyExtrasToNode(node, entry.extras);
        applyAuthoredGeometry(node, entry.layout, origin);
      }

      for (const connection of topology.nodeConnections) {
        const source = nodesBySymbol.get(connection.sourceSymbol);
        const target = nodesBySymbol.get(connection.targetSymbol);
        const targetSlot = target ? resolveInputSlotIndex(target, connection.targetInput) : -1;
        if (
          !source ||
          !target ||
          targetSlot < 0 ||
          connection.sourceSlot >= source.outputs.length
        ) {
          warnings.push(
            `Cube connection '${connection.sourceSymbol}' -> ` +
              `'${connection.targetSymbol}.${connection.targetInput}' is unavailable.`,
          );
          continue;
        }
        source.connect(connection.sourceSlot, target, targetSlot);
      }

      for (const input of topology.inputs) {
        const resolvedTargets = input.targets
          .map((target) => {
            const node = nodesBySymbol.get(target.symbol);
            const slotIndex = node ? resolveInputSlotIndex(node, target.input) : -1;
            const slot = slotIndex >= 0 ? node?.inputs[slotIndex] : undefined;
            return node && slot ? { node, slot } : null;
          })
          .filter(
            (target): target is { node: NativeGraphNode; slot: ComfyInput } => target !== null,
          );
        const firstTarget = resolvedTargets[0];
        if (!firstTarget) {
          warnings.push(`Cube input '${input.name}' has no compatible internal target.`);
          continue;
        }
        const boundary = subgraph.addInput(
          input.name,
          input.type ?? resolveCubeInputBoundaryType(resolvedTargets.map((target) => target.slot)),
        );
        for (const target of resolvedTargets) boundary.connect(target.slot, target.node);
        for (const target of resolvedTargets) inputBoundaryNodes.add(target.node);
      }

      const surfaceOutputNames = deriveCubeOutputSurfaceNames(topology.outputs);
      for (const [index, output] of topology.outputs.entries()) {
        const source = nodesBySymbol.get(output.sourceSymbol);
        const slot = source?.outputs[output.sourceSlot];
        const type = output.type ?? readString(slot?.type);
        if (!source || !slot || !type) {
          warnings.push(`Cube output '${output.name}' has no compatible internal source.`);
          continue;
        }
        subgraph.addOutput(surfaceOutputNames[index] ?? output.name, type).connect(slot, source);
        outputBoundaryNodes.add(source);
      }

      subgraph.inputNode.arrange?.();
      subgraph.outputNode.arrange?.();
      attachNativeCubeBoundaryLayout(subgraph, inputBoundaryNodes, outputBoundaryNodes);
      applyNativeCubeBoundaryLayout(subgraph);
      return { subgraph, nodesBySymbol, warnings };
    } catch (error: unknown) {
      this.#host.discardSubgraph?.(subgraph);
      throw error;
    }
  }

  /** Create one native subgraph record with the boundary surface required by its use case. */
  #createSubgraph(title: string, description?: string): NativeCubeSubgraph {
    return this.#host.rootGraph.createSubgraph(
      createEmptySubgraph(this.#host.createUuid(), title, description),
    );
  }
}

/** Create the minimum current Comfy subgraph record before native mutation. */
function createEmptySubgraph(id: string, title: string, description?: string): UnknownRecord {
  return {
    id,
    name: title,
    ...(description ? { description } : {}),
    inputNode: { id: -10, bounding: [...EMPTY_CUBE_INPUT_POSITION, 75, 100] },
    outputNode: { id: -20, bounding: [...EMPTY_CUBE_OUTPUT_POSITION, 75, 100] },
    inputs: [],
    outputs: [],
    widgets: [],
    version: 1,
    state: { lastNodeId: 0, lastLinkId: 0, lastRerouteId: 0, lastGroupId: 0 },
    revision: 0,
    config: {},
    links: [],
    nodes: [],
    reroutes: [],
    groups: [],
    extra: {},
  };
}

/** Resolve imported Cube metadata before Comfy registers its generated node definition. */
function readCubeDescription(payload: ImportPayload): string {
  const cube = isRecord(payload.cube) ? payload.cube : {};
  const metadata = isRecord(cube.metadata) ? { ...cube, ...cube.metadata } : cube;
  return resolveCubeDefinitionDescription(metadata);
}

/** Apply authored editor geometry relative to the Cube graph origin. */
function applyAuthoredGeometry(node: NativeGraphNode, layout: unknown, origin: Vec2): void {
  const record = isRecord(layout) ? layout : {};
  const pos = readPair(record.pos, [0, 0]);
  const size = readPair(record.size, node.size);
  node.pos[0] = pos[0] - origin[0];
  node.pos[1] = pos[1] - origin[1];
  node.setSize?.(size);
  node.size[0] = size[0];
  node.size[1] = size[1];
  const flags = isRecord(record.flags)
    ? record.flags
    : isRecord(record.extra) && isRecord(record.extra.flags)
      ? record.extra.flags
      : null;
  if (flags) node.flags = { ...flags };
  const style = isRecord(record.style)
    ? record.style
    : isRecord(record.extra) && isRecord(record.extra.style)
      ? record.extra.style
      : null;
  if (style) {
    if (typeof style.color === 'string') node.color = style.color;
    if (typeof style.bgcolor === 'string') node.bgcolor = style.bgcolor;
    if (style.shape !== undefined) node.shape = style.shape;
  }
}

/** Read the prepared payload's coordinate origin. */
function readOrigin(payload: ImportPayload): Vec2 {
  return readPair(payload.layout?.origin, [0, 0]);
}

/** Read one finite numeric pair from an untrusted host value. */
function readPair(value: unknown, fallback: ArrayLike<number>): Vec2 {
  if (!Array.isArray(value)) {
    return [Number(fallback[0]) || 0, Number(fallback[1]) || 0];
  }
  const x = Number(value[0]);
  const y = Number(value[1]);
  return [
    Number.isFinite(x) ? x : Number(fallback[0]) || 0,
    Number.isFinite(y) ? y : Number(fallback[1]) || 0,
  ];
}

/** Read one trimmed host string. */
function readString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}
