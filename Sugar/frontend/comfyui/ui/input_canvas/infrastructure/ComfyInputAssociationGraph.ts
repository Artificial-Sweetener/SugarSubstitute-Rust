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
/** Translate live Comfy connections and native subgraph boundaries into portable loader relationships. */
import { collectGraphLinks, getGraphNodes } from '../../graph/GraphQuery.js';
import { NativeSubgraphBoundaryResolver } from '../../cube/graph/NativeSubgraphBoundaryResolver.js';
import { isRecord } from '../../types/common.js';
import type { ComfyGraph, ComfyNode } from '../../types/graph.js';
import { isContextImageInput } from '../domain/InputContextSemantics.js';
import type { InputLoader, InputKind } from '../domain/InputLoader.js';
import type {
  InputAssetAssociationGraph,
  InputAssetConsumerInput,
} from '../domain/InputAssetAssociation.js';
import type { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { isSerializedWidget } from '../../graph/WidgetValueSerialization.js';
import {
  inferDimensionFieldPair,
  type SpatialEdge,
  type SpatialNode,
  type SpatialTopology,
} from '../domain/CanvasDimensionAuthority.js';

/** Keep native source-state lookup behind the graph adapter. */
export interface ComfyInputAssetField {
  readonly key: string;
  readonly node: ComfyNode;
  readonly loader: InputLoader;
}
/** Retain actual graph ownership while flattened consumer identities cross exposed boundaries. */
export interface ComfyInputAssociationScope {
  readonly graph: ComfyGraph;
  readonly parent: ComfyGraph | null;
  readonly prefix: string;
}

/** Own exact native handles and runtime identities without serializing workflows or reading DOM surfaces. */
export class ComfyInputAssociationGraph {
  readonly #nodeKeys = new WeakMap<ComfyNode, string>();
  readonly #fields = new Map<string, ComfyInputAssetField>();
  readonly #paths = new Map<string, ComfyNode | null>();
  readonly #scopes = new Map<ComfyGraph, ComfyInputAssociationScope>();
  readonly #invalid = new Set<ComfyGraph>();
  readonly #boundaries: NativeSubgraphBoundaryResolver;
  #nextKey = 0;

  /** Reuse the shared authoritative native boundary resolver. */
  constructor(
    private readonly loaders: ComfyInputLoaders,
    logger: Pick<Console, 'warn'>,
  ) {
    this.#boundaries = new NativeSubgraphBoundaryResolver(logger);
  }

  /** Preserve field identity while native node instances remain alive. */
  key(node: ComfyNode, field: string): string {
    return JSON.stringify([this.#nodeKey(node), field]);
  }

  /** Return only a field owned by the current workflow inventory. */
  field(key: string): ComfyInputAssetField | null {
    return this.#fields.get(key) ?? null;
  }

  /** Translate a cached native field identity to the same authored path used by queue projection. */
  workflowKey(key: string): string | null {
    const field = this.#fields.get(key);
    const path = field ? this.path(field.node) : null;
    return field && path !== null ? JSON.stringify([path, field.loader.field]) : null;
  }

  /** Classify an owned field by the outputs actually used, or null for an unknown field. */
  kind(key: string): InputKind | null {
    const field = this.#fields.get(key);
    return field ? endpointKind(field.node, field.loader) : null;
  }

  /** List every loader field owned by the current workflow inventory. */
  fields(): readonly ComfyInputAssetField[] {
    return [...this.#fields.values()];
  }

  /** Resolve a live node's unique flattened workflow path. */
  path(node: ComfyNode): string | null {
    const paths = [...this.#paths].filter(([, value]) => value === node).map(([path]) => path);
    return paths.length === 1 ? (paths[0] ?? null) : null;
  }

  /** Resolve a flattened workflow path to its unique live node. */
  node(path: string): ComfyNode | null {
    return this.#paths.get(path) ?? null;
  }

  /**
   * Read one scope's spatial connections and authored size fields under flattened paths.
   *
   * Connections that cross subgraph boundaries are resolved to the inner nodes, as in `read`.
   * Size fields count only when their widget is serialized and not driven by a link.
   */
  spatial(graph: ComfyGraph): SpatialTopology {
    const scope = this.#scopes.get(graph);
    if (!scope || this.#invalid.has(graph)) return { nodes: [], edges: [] };
    const nodes = getGraphNodes(graph);
    const byId = new Map(
      nodes.filter((node) => validNodeId(node.id)).map((node) => [String(node.id), node]),
    );
    const spatialNodes: SpatialNode[] = nodes.flatMap((node) => {
      if (!validNodeId(node.id) || isRecord(node.subgraph)) return [];
      const key = scope.prefix + String(node.id);
      if (this.#paths.get(key) !== node) return [];
      const linked = new Set(
        (node.inputs ?? []).flatMap((input) =>
          input.link !== null && input.link !== undefined ? [input.widget?.name ?? input.name] : [],
        ),
      );
      const pair = inferDimensionFieldPair(
        (node.widgets ?? []).flatMap((widget) =>
          widget.name && isSerializedWidget(widget) && !linked.has(widget.name)
            ? [widget.name]
            : [],
        ),
      );
      return [
        {
          key,
          outputs: (node.outputs ?? []).map((output) =>
            typeof output.type === 'string' ? output.type.toUpperCase() : null,
          ),
          dimensions: pair ? [pair] : [],
        },
      ];
    });
    const edges: SpatialEdge[] = [];
    for (const link of collectGraphLinks(graph)) {
      const provider = byId.get(String(link.origin_id ?? link.origin)),
        consumer = byId.get(String(link.target_id ?? link.target));
      if (!provider || !consumer || !validSlot(link.origin_slot) || !validSlot(link.target_slot))
        continue;
      const origin = this.#boundaries.resolveOutput(provider, link.origin_slot);
      const sourceKey = origin ? scope.prefix + String(origin.nodeId) : null;
      const source = sourceKey ? this.#paths.get(sourceKey) : null;
      if (!origin || !sourceKey || !source) continue;
      const type = source.outputs?.[origin.slot]?.type;
      for (const target of this.#boundaries.resolveInputTargets(consumer, link.target_slot)) {
        const targetKey = scope.prefix + String(target.nodeId);
        const node = this.#paths.get(targetKey);
        if (!node) continue;
        const inputType = node.inputs?.[target.inputSlot]?.type;
        edges.push({
          source: sourceKey,
          output: origin.slot,
          type: typeof type === 'string' ? type.toUpperCase() : null,
          target: targetKey,
          inputType: typeof inputType === 'string' ? inputType.toUpperCase() : null,
        });
      }
    }
    return { nodes: spatialNodes, edges };
  }

  /** Inventory membership at explicit workflow, node-membership or definition lifecycles. */
  reconcile(root: ComfyGraph | null | undefined): readonly ComfyInputAssociationScope[] {
    this.#fields.clear();
    this.#paths.clear();
    this.#scopes.clear();
    this.#invalid.clear();
    const visit = (
      graph: ComfyGraph,
      parent: ComfyGraph | null,
      prefix: string,
      ancestors: ReadonlySet<ComfyGraph>,
    ): void => {
      if (ancestors.has(graph)) {
        this.#invalid.add(graph);
        return;
      }
      const previous = this.#scopes.get(graph);
      if (previous) {
        if (previous.prefix !== prefix) this.#invalid.add(graph);
        return;
      }
      this.#scopes.set(graph, { graph, parent, prefix });
      for (const node of getGraphNodes(graph)) {
        if (!validNodeId(node.id)) continue;
        const path = prefix + String(node.id);
        this.#paths.set(path, this.#paths.has(path) ? null : node);
        if (isRecord(node.subgraph))
          visit(node.subgraph, graph, path + ':', new Set([...ancestors, graph]));
      }
    };
    if (root) visit(root, null, '', new Set());
    for (const scope of this.#scopes.values()) {
      if (this.#invalid.has(scope.graph)) continue;
      for (const node of getGraphNodes(scope.graph)) {
        if (!validNodeId(node.id) || this.#paths.get(scope.prefix + String(node.id)) !== node)
          continue;
        for (const loader of this.loaders.all(node.type ?? node.class_type ?? '')) {
          const key = this.key(node, loader.field);
          this.#fields.set(key, { key, node, loader });
        }
      }
    }
    return [...this.#scopes.values()];
  }

  /** Re-read only a changed graph's connections while preserving its cached membership handles. */
  read(graph: ComfyGraph): InputAssetAssociationGraph {
    const scope = this.#scopes.get(graph);
    if (!scope || this.#invalid.has(graph)) return { endpoints: [], inputs: [] };
    const nodes = getGraphNodes(graph),
      byId = new Map(
        nodes.filter((node) => validNodeId(node.id)).map((node) => [String(node.id), node]),
      );
    const inputs: InputAssetConsumerInput[] = [];
    for (const link of collectGraphLinks(graph)) {
      const provider = byId.get(String(link.origin_id ?? link.origin)),
        consumer = byId.get(String(link.target_id ?? link.target));
      const output = link.origin_slot,
        input = link.target_slot;
      if (!provider || !consumer || !validSlot(output) || !validSlot(input)) continue;
      const nativeInput = consumer.inputs?.[input];
      if (
        !nativeInput ||
        (nativeInput.link != null &&
          link.id != null &&
          String(nativeInput.link) !== String(link.id))
      )
        continue;
      const origin = this.#boundaries.resolveOutput(provider, output);
      const source = origin ? this.#paths.get(scope.prefix + String(origin.nodeId)) : null,
        outputKind = source && origin ? assetKind(source.outputs?.[origin.slot]?.type) : null;
      for (const target of this.#boundaries.resolveInputTargets(consumer, input)) {
        const node = this.#paths.get(scope.prefix + String(target.nodeId));
        if (!node) continue;
        const type = node.inputs?.[target.inputSlot]?.type;
        const kind = assetKind(type) ?? (type == null || type === '*' ? outputKind : null);
        if (!kind) continue;
        if (
          kind === 'image' &&
          !isContextImageInput(
            node.type ?? node.class_type ?? '',
            node.inputs?.[target.inputSlot]?.name ?? '',
          )
        )
          continue;
        const endpoints =
          source && outputKind === kind
            ? this.loaders
                .all(source.type ?? source.class_type ?? '')
                .filter((loader) => endpointKind(source, loader) === kind)
                .map((loader) => this.key(source, loader.field))
                .filter((key) => this.#fields.has(key))
            : [];
        inputs.push({
          consumer: this.#nodeKey(node),
          input: String(target.inputSlot),
          kind,
          endpoints,
        });
      }
    }
    return {
      endpoints: nodes.flatMap((node) =>
        this.loaders.all(node.type ?? node.class_type ?? '').flatMap((loader) => {
          const key = this.key(node, loader.field);
          return this.#fields.has(key)
            ? [
                {
                  key,
                  kind: endpointKind(node, loader),
                  maskOutput:
                    endpointKind(node, loader) === 'image' &&
                    Boolean(
                      node.outputs?.some(
                        (output) => assetKind(output.type) === 'mask' && output.links?.length,
                      ),
                    ),
                },
              ]
            : [];
        }),
      ),
      inputs,
    };
  }

  /** Read observed graph scope without allowing a consumer to reconstruct native hierarchy. */
  scope(graph: ComfyGraph): ComfyInputAssociationScope | null {
    return this.#scopes.get(graph) ?? null;
  }

  /** Assign opaque process-lifetime identities so equal numeric IDs in distinct Cubes never collide. */
  #nodeKey(node: ComfyNode): string {
    let key = this.#nodeKeys.get(node);
    if (!key) {
      key = 'input-node-' + String(++this.#nextKey);
      this.#nodeKeys.set(node, key);
    }
    return key;
  }
}

/**
 * Classify a loader field by the spatial outputs actually used: an image loader whose IMAGE
 * output is used is an image endpoint even when its MASK output is also used; one whose only
 * used spatial output is MASK is a mask endpoint.
 */
function endpointKind(node: ComfyNode, loader: InputLoader): InputKind {
  if (loader.kind !== 'image') return loader.kind;
  const used = (kind: InputKind): boolean =>
    Boolean(
      node.outputs?.some((output) => assetKind(output.type) === kind && output.links?.length),
    );
  return !used('image') && used('mask') ? 'mask' : 'image';
}
/** Admit native node identity before it participates in flattened paths. */
function validNodeId(value: unknown): value is string | number {
  return typeof value === 'string'
    ? value.length > 0
    : typeof value === 'number' && Number.isFinite(value);
}
/** Reject malformed native socket indexes before querying subgraph boundaries. */
function validSlot(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}
/** Normalize only declared image/mask socket semantics. */
function assetKind(value: unknown): InputKind | null {
  const type = typeof value === 'string' ? value.toUpperCase() : '';
  return type === 'IMAGE' ? 'image' : type === 'MASK' ? 'mask' : null;
}
