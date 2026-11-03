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
/** Translate the live workflow into surface facts keyed by stable workflow paths. */
import { isRecord } from '../../types/common.js';
import type { ComfyNode } from '../../types/graph.js';
import { resolveDimensionAuthority } from '../domain/CanvasDimensionAuthority.js';
import {
  comfyFileReference,
  type WorkflowImageEndpoint,
  type WorkflowMaskEndpoint,
  type WorkflowSurfaceFacts,
} from '../domain/WorkflowSurfacePlan.js';
import { DEFAULT_MASK_OPACITY } from '../domain/MaskColorScheme.js';
import type { InputEntry } from '../domain/InputBinding.js';
import type { WorkflowSurfaceGraph } from '../application/WorkflowSurfaceProvisioning.js';
import type { MaskOpacityStore } from '../application/WorkflowMaskAppearance.js';
import type { InputBindingController } from '../application/InputBindingController.js';
import type { ComfyInputAssociations } from './ComfyInputAssociations.js';
import type { ComfyInputAssetField } from './ComfyInputAssociationGraph.js';
import type { ComfyInputBindings } from './ComfyInputBindings.js';
import type { ComfyInputLoaders } from './ComfyInputLoaders.js';
import type { ComfyWorkflowInputState } from './ComfyWorkflowInputState.js';
import { readRegionalPrompts } from './ComfyRegionalPrompts.js';

/** Node property holding a mask loader's visual opacity in 0..1. */
export const MASK_OPACITY_KEY = 'sugarcubes_canvas_mask_opacity';

/**
 * Read endpoints, pairings and size authorities from the existing association owner.
 *
 * An endpoint is a loader field whose IMAGE (or, for mask loaders, MASK) output is connected;
 * a node with more than one upload field is skipped because its outputs cannot be attributed to
 * one field. Changes are observed through topology flushes, loader bindings and the width/height
 * fields of current size authorities, never through rendered cards.
 */
export class ComfyWorkflowSurfaceGraph implements WorkflowSurfaceGraph, MaskOpacityStore {
  readonly #listeners = new Set<() => void>();
  readonly #bindings = new Map<InputBindingController, () => void>();
  readonly #widgets = new Map<ComfyNode, () => void>();
  readonly #stopTopology: () => void;

  /** Share the association, binding and loader owners used by previews and queue preparation. */
  constructor(
    private readonly associations: ComfyInputAssociations,
    private readonly bindings: ComfyInputBindings,
    private readonly loaders: ComfyInputLoaders,
    private readonly state: Pick<ComfyWorkflowInputState, 'workflow'>,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stopTopology = associations.subscribeTopology(() => this.#changed());
  }

  /** Resolve the loaded workflow's identity; none while a workflow is still being configured. */
  workflow(create: boolean): string | null {
    return this.associations.active ? this.state.workflow(create) : null;
  }

  /** Read current endpoints and observe the bindings and size fields they depend on. */
  facts(): WorkflowSurfaceFacts {
    const inventory = this.associations.inventory,
      topology = this.associations.topology();
    const used = new Set(topology.edges.map((edge) => JSON.stringify([edge.source, edge.type])));
    const images: WorkflowImageEndpoint[] = [],
      masks: WorkflowMaskEndpoint[] = [],
      bindings = new Set<InputBindingController>(),
      observed = new Set<ComfyNode>();
    const imageKeys = new Map<string, string>();
    const fields = inventory.fields().filter((field) => this.#endpoint(field));
    for (const field of fields) {
      const path = inventory.path(field.node),
        key = inventory.workflowKey(field.key);
      const kind = field.loader.kind === 'image' ? 'IMAGE' : 'MASK';
      if (path === null || key === null || !used.has(JSON.stringify([path, kind]))) continue;
      const binding = this.bindings.get(field.node, field.loader);
      bindings.add(binding);
      if (field.loader.kind !== 'image') continue;
      const entry = selectedEntry(binding.state.entries, binding.state.selected);
      images.push({
        key,
        label: nodeLabel(field.node),
        file:
          entry?.source.kind === 'file' && entry.source.file
            ? comfyFileReference(entry.source.file)
            : null,
      });
      imageKeys.set(field.key, key);
    }
    for (const field of fields) {
      if (field.loader.kind !== 'mask') continue;
      const path = inventory.path(field.node),
        key = inventory.workflowKey(field.key);
      if (path === null || key === null || !used.has(JSON.stringify([path, 'MASK']))) continue;
      const association = this.associations.relationships.read(field.key);
      const image =
        association.kind === 'associated'
          ? (imageKeys.get(association.image) ?? null)
          : association.kind === 'ambiguous'
            ? 'ambiguous'
            : null;
      const authority =
        image === null
          ? resolveDimensionAuthority(topology, path, (node, name) =>
              widgetValue(inventory.node(node), name),
            )
          : ({ kind: 'missing', reason: 'no_relevant_spatial_root_with_dimensions' } as const);
      if (authority.kind === 'resolved')
        for (const item of authority.nodes) {
          const node = inventory.node(item.node);
          if (node) observed.add(node);
        }
      const prompts = field.loader.ordered
        ? readRegionalPrompts(topology, path, (node) => inventory.node(node))
        : null;
      for (const node of prompts?.nodes ?? []) observed.add(node);
      const binding = this.bindings.get(field.node, field.loader);
      const state = binding.state;
      const entries = field.loader.ordered
        ? state.entries
        : [selectedEntry(state.entries, state.selected)].filter((entry) => entry !== null);
      masks.push({
        key,
        label: nodeLabel(field.node),
        opacity: opacityOf(field.node),
        image,
        authority,
        regions: prompts?.demand ?? null,
        entries: (entries.length || field.loader.ordered ? entries : [null]).map((entry) => ({
          entry: field.loader.ordered && entry ? entry.id : null,
          label: field.loader.ordered && entry ? entry.label : nodeLabel(field.node),
          initial:
            entry?.source.kind === 'file' && entry.source.file
              ? comfyFileReference(entry.source.file)
              : null,
        })),
      });
    }
    this.#observeBindings(bindings);
    this.#observeWidgets(observed);
    return { images, masks };
  }

  /** Resolve a mask endpoint's binding owner by its stable key. */
  binding(endpoint: string): InputBindingController | null {
    const inventory = this.associations.inventory;
    const field = inventory
      .fields()
      .find((item) => item.loader.kind === 'mask' && inventory.workflowKey(item.key) === endpoint);
    return field ? this.bindings.get(field.node, field.loader) : null;
  }

  /** Store a mask loader's visual opacity on its node, outside native undo history. */
  write(endpoint: string, opacity: number): void {
    const inventory = this.associations.inventory;
    const field = inventory.fields().find((item) => inventory.workflowKey(item.key) === endpoint);
    if (!field) throw new Error('The mask node is no longer in the workflow.');
    field.node.properties = {
      ...(isRecord(field.node.properties) ? field.node.properties : {}),
      [MASK_OPACITY_KEY]: opacity,
    };
    this.#changed();
  }

  /** Observe plan inputs. */
  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    return () => {
      this.#listeners.delete(changed);
    };
  }

  /** Release binding and widget observation. */
  dispose(): void {
    this.#stopTopology();
    this.#observeBindings(new Set());
    this.#observeWidgets(new Set());
    this.#listeners.clear();
  }

  /** Admit loader fields of nodes with exactly one upload field. */
  #endpoint(field: ComfyInputAssetField): boolean {
    return this.loaders.all(field.node.type ?? field.node.class_type ?? '').length === 1;
  }

  /** Follow exactly the bindings the last plan read. */
  #observeBindings(bindings: ReadonlySet<InputBindingController>): void {
    for (const [binding, stop] of this.#bindings)
      if (!bindings.has(binding)) {
        stop();
        this.#bindings.delete(binding);
      }
    for (const binding of bindings)
      if (!this.#bindings.has(binding))
        this.#bindings.set(
          binding,
          binding.subscribe(() => this.#changed()),
        );
  }

  /** Follow widget commits on the size authorities and regional prompts the last plan read. */
  #observeWidgets(nodes: ReadonlySet<ComfyNode>): void {
    for (const [node, stop] of this.#widgets)
      if (!nodes.has(node)) {
        stop();
        this.#widgets.delete(node);
      }
    for (const node of nodes) {
      if (this.#widgets.has(node)) continue;
      const previous = node.onWidgetChanged;
      const listener = (...args: unknown[]): unknown => {
        const result: unknown =
          typeof previous === 'function' ? previous.apply(node, args) : undefined;
        this.#changed();
        return result;
      };
      node.onWidgetChanged = listener;
      this.#widgets.set(node, () => {
        if (node.onWidgetChanged === listener) node.onWidgetChanged = previous;
      });
    }
  }

  /** Notify plan observers, isolating their failures. */
  #changed(): void {
    for (const listener of this.#listeners)
      try {
        listener();
      } catch (error: unknown) {
        this.report(error);
      }
  }
}

/** Resolve the selected entry, or the first one. */
function selectedEntry(entries: readonly InputEntry[], selected: string | null): InputEntry | null {
  return entries.find((entry) => entry.id === selected) ?? entries[0] ?? null;
}

/** Prefer the user's node title. */
function nodeLabel(node: ComfyNode): string {
  const title = typeof node.title === 'string' ? node.title.trim() : '';
  return (title || node.type || node.class_type || 'Input').slice(0, 256);
}

/** Read a stored opacity, falling back to the default for missing or invalid values. */
function opacityOf(node: ComfyNode): number {
  const value = isRecord(node.properties) ? node.properties[MASK_OPACITY_KEY] : undefined;
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 1
    ? value
    : DEFAULT_MASK_OPACITY;
}

/** Read a widget's current value from a live node. */
function widgetValue(node: ComfyNode | null, name: string): unknown {
  return node?.widgets?.find((widget) => widget.name === name)?.value;
}
