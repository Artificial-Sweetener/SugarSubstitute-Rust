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
/** Inventory editing surfaces without enrolling hidden native subgraph implementation fields. */
import type { ComfyGraph, ComfyNode, ComfyWidget } from '../../types/graph.js';
import { isCubeNode, requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { NativeSubgraphBoundaryResolver } from '../../cube/graph/NativeSubgraphBoundaryResolver.js';
import { cubeFaceRevealableWidgets } from '../../surface/CubeFaceWidgetPolicy.js';
import { projectNodeCards } from '../../surface/NodeCardProjection.js';
import { parseCubeSurfaceState } from '../../surface/CubeSurfaceState.js';
import { cubeFaceResourcePassThrough } from '../../surface/CubeFaceNodeSemantics.js';
import {
  cubeFacePromptRole,
  findCubeFacePromptWidget,
} from '../../surface/CubeFacePromptPolicy.js';
import {
  orderWorkflowNodes,
  type WorkflowNodeConnection,
} from '../../surface/WorkflowNodeOrder.js';
import { resolveCubeFaceRevealDecision } from '../../surface/CubeFaceRevealPolicy.js';
import type { CubeFaceCardStateService } from '../../surface/CubeFaceCardStateService.js';
import { readNativeSeedControl } from '../../surface/appearance/ComfyNativeSeedControl.js';
import { intersectFieldDomains, semanticFieldName } from '../domain/FieldCompatibility.js';
import type { EditableRelationField, RelationInventory } from '../application/RelationPorts.js';
import type { NodeContract } from '../domain/RelationContracts.js';
import { isFieldValue, nativeFieldDomain } from './NativeFieldContract.js';
import { NativeRelationIdentity } from './NativeRelationIdentity.js';

/** Retain native owners alongside a validated renderer-independent candidate inventory. */
export class NativeRelationInventory {
  readonly #boundary: NativeSubgraphBoundaryResolver;
  readonly owners = new Map<
    string,
    {
      node: ComfyNode;
      widget: ComfyWidget;
      face: ComfyNode;
      cubeId: string | null;
      executionId: string;
      nodeId: string;
    }
  >();

  /** Reuse native exposure resolution instead of scanning nested implementation settings. */
  constructor(
    logger: Pick<Console, 'warn'>,
    private readonly getCardState: () => CubeFaceCardStateService | null = () => null,
  ) {
    this.#boundary = new NativeSubgraphBoundaryResolver(logger);
  }

  /** Read root fields and Cube-face capabilities regardless of renderer or viewport state. */
  read(
    graph: ComfyGraph | null | undefined,
    connections: readonly WorkflowNodeConnection[] = [],
  ): RelationInventory {
    this.owners.clear();
    const fields: EditableRelationField[] = [],
      nodes: NodeContract[] = [];
    const configurationIdentities = new NativeRelationIdentity();
    const editableNodes: Array<{ id: string; readMode(): number; writeMode(mode: number): void }> =
      [];
    for (const root of graph ? orderWorkflowNodes(graph, connections) : []) {
      if (isCubeNode(root)) {
        const identity = requireCubeIdentity(root);
        const cubeId =
          typeof identity.instance_id === 'string' ? identity.instance_id : String(root.id);
        const surface = parseCubeSurfaceState(root.properties.sugarcubes_surface);
        const cards = projectNodeCards(root.subgraph._nodes ?? [], surface, root.subgraph, root);
        for (const card of cards) {
          const face = card.node;
          const symbol =
            typeof face.properties?.sugarcubes_symbol === 'string'
              ? face.properties.sugarcubes_symbol
              : String(face.id);
          const nodeId = JSON.stringify(['cube', cubeId, symbol]);
          const label = `${root.title ?? 'Cube'} · ${card.label}`;
          const owned = cubeFaceRevealableWidgets(face).map((widget) => ({
            node: face,
            widget,
            name: widget.name,
          }));
          for (const source of card.widgetSources ?? []) {
            const widget = source.node.widgets?.find((candidate) => candidate.name === source.name);
            if (widget) owned.push({ node: source.node, widget, name: source.targetName });
          }
          const candidates = owned.flatMap(({ node, widget, name }) =>
            this.#field(
              nodeId,
              cubeId,
              label,
              face,
              node,
              widget,
              name,
              `${String(root.id)}:${String(face.id)}`,
            ),
          );
          fields.push(...candidates);
          const cardState = surface.cards[String(face.id)];
          const promptWidget = findCubeFacePromptWidget(face);
          const promptRole = cubeFacePromptRole(face);
          const activation = resolveCubeFaceRevealDecision(face, cardState);
          const configurationIdentity = configurationIdentities.read(
            face,
            symbol,
            candidates.map((field) => field.contract.name),
          );
          const activeMode = typeof face.mode === 'number' && face.mode !== 4 ? face.mode : 0;
          nodes.push({
            id: nodeId,
            cubeId,
            cubeLabel: root.title ?? 'Cube',
            label,
            classType: face.class_type ?? face.type ?? '',
            configurationIdentity: configurationIdentity ?? nodeId,
            fields: candidates.map((candidate) => candidate.contract),
            linkable:
              (isResourcePatch(face) || promptWidget !== null) &&
              !isResourceProvider(face) &&
              configurationIdentity !== null &&
              face.isSubgraphNode?.() !== true,
            ...(promptWidget ? { promptRole } : {}),
            mode: activation.enabledChoice ? (cardState?.activeMode ?? activeMode) : 4,
          });
          editableNodes.push({
            id: nodeId,
            readMode: () => (typeof face.mode === 'number' ? face.mode : 0),
            writeMode: (mode) => {
              const owner = this.getCardState();
              if (!owner) throw new Error('Cube card activation owner is unavailable.');
              owner.setExecutionMode(root, String(face.id), mode);
            },
          });
        }
      } else {
        const nodeId = JSON.stringify(['node', String(root.id)]);
        for (const widget of cubeFaceRevealableWidgets(root))
          fields.push(
            ...this.#field(
              nodeId,
              null,
              root.title ?? root.type ?? 'Node',
              root,
              root,
              widget,
              widget.name,
              String(root.id),
            ),
          );
      }
    }
    return { fields, nodes, editableNodes };
  }

  /** Synchronize explicitly exposed leaf values before native Cube serialization. */
  prepareField(id: string, value: string | number | boolean): void {
    const owner = this.owners.get(id);
    if (!owner || owner.node.isSubgraphNode?.() !== true) return;
    for (const target of this.#boundary.resolveWidgetTargets(owner.node, owner.widget.name)) {
      const name = target.node.inputs?.[target.inputSlot]?.widget?.name ?? target.inputName;
      const widget = target.node.widgets?.find((candidate) => candidate.name === name);
      if (widget) writeNativeField(target.node, widget, value);
    }
  }

  /** Deduplicate forwarded controls and infer meaning only through explicitly exposed boundaries. */
  #field(
    nodeId: string,
    cubeId: string | null,
    label: string,
    face: ComfyNode,
    node: ComfyNode,
    widget: ComfyWidget,
    name: string,
    executionId: string,
  ): EditableRelationField[] {
    const origins = this.#boundary.resolveWidgetOrigins(face, name);
    const meanings = new Set(origins.map((origin) => semanticFieldName(origin.inputName)));
    if (meanings.size !== 1 || !isFieldValue(widget.value)) return [];
    const semantic = [...meanings][0] ?? '';
    const facadeDomain = nativeFieldDomain(face, widget, origins[0]?.inputName ?? name);
    if (!facadeDomain) return [];
    const targets =
      face.isSubgraphNode?.() === true ? this.#boundary.resolveWidgetTargets(face, name) : [];
    const domains = targets.map((target) => {
      const inputName = target.node.inputs?.[target.inputSlot]?.widget?.name ?? target.inputName;
      const leafWidget = target.node.widgets?.find((candidate) => candidate.name === inputName);
      return leafWidget ? nativeFieldDomain(target.node, leafWidget, inputName) : null;
    });
    if (domains.some((domain) => domain === null)) return [];
    const domain = intersectFieldDomains([
      facadeDomain,
      ...domains.flatMap((domain) => (domain ? [domain] : [])),
    ]);
    if (!domain) return [];
    const id = JSON.stringify([nodeId, name]);
    if (this.owners.has(id)) return [];
    this.owners.set(id, { node, widget, face, cubeId, executionId, nodeId });
    const seedMode =
      semantic === 'seed' ? readNativeSeedControl(node, widget.name)?.readMode() : undefined;
    return [
      {
        contract: {
          id,
          nodeId,
          cubeId,
          label: `${label} · ${name}`,
          name,
          semantic,
          domain,
          value: widget.value,
          ...(seedMode ? { seedMode } : {}),
          linkable: !isResourceProvider(face),
        },
        read: () => (isFieldValue(widget.value) ? widget.value : ''),
        write: (value) => writeNativeField(node, widget, value),
      },
    ];
  }
}

/** Exclude resource origins such as loaders by requiring a MODEL/CLIP/VAE input and matching output. */
function isResourcePatch(node: ComfyNode): boolean {
  const resources = new Set(['MODEL', 'CLIP', 'VAE']);
  return [...cubeFaceResourcePassThrough(node)].some((resource) => resources.has(resource));
}

/** Respect inheritance's resource-origin distinction without maintaining a loader class list. */
function isResourceProvider(node: ComfyNode): boolean {
  const resources = new Set(['MODEL', 'CLIP', 'VAE']);
  return (
    (node.outputs ?? []).some(
      (output) => typeof output.type === 'string' && resources.has(output.type.toUpperCase()),
    ) &&
    !(node.inputs ?? []).some(
      (input) => typeof input.type === 'string' && resources.has(input.type.toUpperCase()),
    )
  );
}

/** Commit through the real widget owner while retaining native callback and graph change semantics. */
function writeNativeField(
  node: ComfyNode,
  widget: ComfyWidget,
  value: string | number | boolean,
): void {
  if (widget.value === value) return;
  const previous = widget.value;
  widget.value = value;
  widget.callback?.call(widget, value, node, node.graph?.canvas);
  if (typeof node.onWidgetChanged === 'function')
    node.onWidgetChanged.call(node, widget.name, value, previous, widget);
  node.graph?.setDirtyCanvas?.(true, true);
}
