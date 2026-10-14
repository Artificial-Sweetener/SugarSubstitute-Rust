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
/** Discover workflow sources only at graph boundaries and refresh cached labels in scope. */

import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { WorkflowSessionStore } from '../../execution/application/WorkflowSessionStore.js';
import type { WorkflowSessionContext } from '../../execution/domain/WorkflowSession.js';
import type { WorkflowOutputSource } from '../../execution/domain/WorkflowOutput.js';
import { decodeComfyWorkflowSources } from '../../execution/infrastructure/ComfyWorkflowSourceDecoder.js';
import { buildCubeOutputExecutionId } from '../../cube/execution/CubeOutputExecutionIdentity.js';
import { resolveCubeModelTitle } from '../../cube/CubeModelTitlePresentation.js';
import {
  NativeSubgraphChangeObserver,
  type NativeGraphChange,
} from '../../surface/NativeSubgraphChangeObserver.js';
import type { CanvasSourceInventory } from '../application/CanvasSourceInventory.js';
import { CanvasGraphSourceTopology } from './CanvasGraphSourceTopology.js';

interface AuthoredSource {
  readonly nodes: readonly UnknownRecord[];
  readonly source: WorkflowOutputSource;
  readonly output: UnknownRecord | null;
}

/** Keep native node handles inside the adapter, never in catalog or application state. */
export class ComfyCanvasSourceInventory {
  readonly #observers: NativeSubgraphChangeObserver[] = [];
  readonly #authored = new Map<string, AuthoredSource>();
  readonly #topology = new Map<UnknownRecord, CanvasGraphSourceTopology>();
  readonly #listeners = new Set<() => void>();
  #context: WorkflowSessionContext | null = null;
  #stop: (() => void) | null = null;
  #disposed = false;
  #discoveries = 0;
  #metadataRefreshes = 0;

  /** Receive public root access explicitly rather than reading a host global. */
  constructor(
    private readonly sessions: WorkflowSessionStore,
    private readonly inventory: CanvasSourceInventory,
    private readonly root: () => unknown,
    private readonly logger: Pick<Console, 'error'>,
  ) {}

  /** Report explicit topology work separately from scoped labels without retaining graph history. */
  diagnostics() {
    return {
      discoveries: this.#discoveries,
      metadataRefreshes: this.#metadataRefreshes,
      graphs: this.#topology.size,
      sources: this.#authored.size,
    };
  }

  /** Return cached native source handles without rediscovering graph topology from presentation activity. */
  nativeSources(): readonly { readonly path: string; readonly node: UnknownRecord }[] {
    return [...this.#authored.values()].flatMap(({ source, nodes, output }) => {
      const node = nodes.at(-1);
      return node && !output ? [{ path: source.executionPath, node }] : [];
    });
  }

  /** Observe committed workflow activation and release cached handles during loading. */
  install(): void {
    if (this.#stop || this.#disposed) return;
    this.#stop = this.sessions.subscribe(() => this.#synchronize());
    this.#synchronize();
  }

  /** Subscribe to actual catalog membership or authored label changes. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Rebuild membership for an explicitly named exposure/topology lifecycle. */
  topologyChanged(): void {
    if (this.#context && !this.#disposed) this.#discover(this.#context);
  }

  /** Release every graph wrapper and handle when this integration ends. */
  dispose(): void {
    this.#disposed = true;
    this.#stop?.();
    this.#stop = null;
    this.#release();
    this.#context = null;
    this.#listeners.clear();
  }

  /** Keep only metadata for open sessions while active graph handles remain replaceable. */
  #synchronize(): void {
    const snapshot = this.sessions.read();
    const active = snapshot.active;
    if (
      active?.sessionId === this.#context?.sessionId &&
      active?.generation === this.#context?.generation
    )
      return;
    this.#release();
    this.#context = active;
    if (active) this.#discover(active);
    else this.#publish();
  }

  /** Serialize once and bind ordinary, nested and cube marker paths to native metadata. */
  #discover(context: WorkflowSessionContext): void {
    this.#discoveries++;
    this.#release();
    this.#context = context;
    try {
      const root = this.root();
      if (!isRecord(root) || typeof root.serialize !== 'function')
        throw new TypeError('Canvas workflow graph serialization is unavailable.');
      const sources = decodeComfyWorkflowSources(root.serialize.call(root));
      const declared = new Map(sources.map((source) => [source.executionPath, source]));
      const graphs = new Set<UnknownRecord>();
      const visit = (
        graph: UnknownRecord,
        prefix: readonly string[],
        ancestors: ReadonlySet<UnknownRecord>,
        chain: readonly UnknownRecord[],
      ): void => {
        if (prefix.length > 32 || ancestors.has(graph)) return;
        graphs.add(graph);
        const nodes = Array.isArray(graph._nodes)
          ? graph._nodes
          : Array.isArray(graph.nodes)
            ? graph.nodes
            : [];
        for (const node of nodes) {
          if (!isRecord(node) || (typeof node.id !== 'string' && typeof node.id !== 'number'))
            continue;
          const segments = [...prefix, String(node.id)];
          const path = segments.join(':');
          const nodes = [...chain, node];
          const source = declared.get(path);
          if (source) this.#authored.set(path, { source, nodes, output: null });
          if (!isRecord(node.subgraph)) continue;
          const outputs = Array.isArray(node.subgraph.outputs) ? node.subgraph.outputs : [];
          outputs.forEach((output: unknown, slot: number) => {
            const marker = buildCubeOutputExecutionId(path, slot);
            const source = declared.get(marker);
            if (source)
              this.#authored.set(marker, {
                source,
                nodes,
                output: isRecord(output) ? output : null,
              });
          });
          visit(node.subgraph, segments, new Set([...ancestors, graph]), nodes);
        }
      };
      visit(root, [], new Set(), []);
      const presented = sources.map((source) => {
        const authored = this.#authored.get(source.executionPath);
        if (!authored) return source;
        const next = sourceCaption(authored);
        this.#authored.set(source.executionPath, { ...authored, source: next });
        return next;
      });
      this.inventory.replace(context, presented);
      for (const graph of graphs) {
        this.#topology.set(graph, new CanvasGraphSourceTopology(graph));
        if (typeof graph.add !== 'function' || typeof graph.remove !== 'function') continue;
        this.#observers.push(
          new NativeSubgraphChangeObserver(graph, (changes) =>
            this.#changed(context, graph, changes),
          ),
        );
      }
      this.#publish();
    } catch (error: unknown) {
      this.#release();
      this.inventory.replace(context, []);
      this.#publish();
      this.logger.error('[SugarCubes] Canvas source inventory failed', {
        sessionId: context.sessionId,
        generation: context.generation,
        error,
      });
    }
  }

  /** Discover only changed native membership/exposure facts; refresh labels from scoped cached handles. */
  #changed(
    context: WorkflowSessionContext,
    graph: UnknownRecord,
    changes: readonly NativeGraphChange[],
  ): void {
    if (this.#context !== context || this.#disposed) return;
    if (
      changes.some((change) => change.kind !== 'change') ||
      this.#topology.get(graph)?.changed()
    ) {
      this.#discover(context);
      return;
    }
    const targets = new Set(changes.map((change) => change.info));
    const changed: WorkflowOutputSource[] = [];
    for (const authored of this.#authored.values()) {
      if (
        !targets.has(undefined) &&
        !targets.has(null) &&
        !authored.nodes.some((node) => targets.has(node))
      )
        continue;
      const next = sourceCaption(authored);
      if (
        next.label !== authored.source.label ||
        next.breadcrumb.join('\0') !== authored.source.breadcrumb.join('\0')
      ) {
        changed.push(next);
        this.#authored.set(authored.source.executionPath, { ...authored, source: next });
      }
    }
    if (!changed.length) return;
    this.#metadataRefreshes++;
    this.inventory.refreshMetadata(context, changed);
    this.#publish();
  }

  /** Release old graph handles before any replacement is observed. */
  #release(): void {
    for (const observer of this.#observers.splice(0)) observer.dispose();
    this.#authored.clear();
    this.#topology.clear();
  }

  /** Publish metadata without allocating images or consulting execution state. */
  #publish(): void {
    for (const listener of this.#listeners) listener();
  }
}

/** Name Cube outputs after their source, retaining exact output names in multi-output captions and provenance. */
function sourceCaption(authored: AuthoredSource): WorkflowOutputSource {
  const captions = authored.nodes.map(
    (node, index) =>
      label(node.title) ||
      (isRecord(node.subgraph) ? label(node.subgraph.name) : '') ||
      label(node.type) ||
      authored.source.breadcrumb[index] ||
      authored.source.label,
  );
  const node = authored.nodes.at(-1);
  if (!authored.output || !node)
    return {
      ...authored.source,
      label: captions.at(-1) ?? authored.source.label,
      breadcrumb: captions.slice(0, -1),
    };
  const properties = isRecord(node.properties) ? node.properties : null;
  const metadata = isRecord(properties?.sugarcubes_cube) ? properties.sugarcubes_cube : null;
  const name = resolveCubeModelTitle({
    targetModel: metadata?.target_model,
    title: captions.at(-1),
  }).nameText;
  const outputName = label(authored.output.name) || 'Output';
  const outputs = isRecord(node.subgraph) ? node.subgraph.outputs : null;
  return {
    ...authored.source,
    label: Array.isArray(outputs) && outputs.length > 1 ? `${name} · ${outputName}` : name,
    breadcrumb: [...captions, outputName],
  };
}

/** Bound literal host titles at the adapter boundary. */
function label(value: unknown): string {
  return typeof value === 'string' ? value.trim().slice(0, 4096) : '';
}
