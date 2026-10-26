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
    if (!field) return null;
    const paths = [...this.#paths].filter(([, node]) => node === field.node).map(([path]) => path);
    return paths.length === 1 ? JSON.stringify([paths[0], field.loader.field]) : null;
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
                .filter((loader) => loader.kind === kind)
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
                  kind: loader.kind,
                  maskOutput:
                    loader.kind === 'image' &&
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
