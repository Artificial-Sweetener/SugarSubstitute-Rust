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
/** Detect semantic prompt editors and assign their shared Cube-face layout policy. */

import type { ComfyGraph, ComfyLink, ComfyNode, ComfyWidget, GraphId } from '../types/graph.js';
import { cubeFaceVisibleWidgets } from './CubeFaceWidgetPolicy.js';

/** Reserve two masonry columns for the one semantic prompt card in a responsive row. */
export const CUBE_FACE_PROMPT_CARD_COLUMN_SPAN = 2;

type PromptRole = 'positive' | 'negative';

/** Return the width policy for one card without making renderers infer semantics independently. */
export function cubeFaceCardColumnSpan(node: ComfyNode): number {
  return findCubeFacePromptWidget(node) ? CUBE_FACE_PROMPT_CARD_COLUMN_SPAN : 1;
}

/** Rank semantic prompt cards ahead of ordinary cards without changing their order within a role. */
export function cubeFaceCardMasonryPriority(node: ComfyNode): number {
  const widget = findCubeFacePromptWidget(node);
  if (!widget) return 2;
  const role = promptRoleFor(node, widget);
  return role === 'positive' ? 0 : role === 'negative' ? 1 : 2;
}

/** Find the one unambiguous editable multiline widget that represents a prompt. */
export function findCubeFacePromptWidget(node: ComfyNode): ComfyWidget | null {
  const candidates = cubeFaceVisibleWidgets(node).filter(isEditableMultilineWidget);
  if (candidates.length === 0) return null;
  const matching = candidates.filter((widget) => hasPromptEvidence(node, widget));
  return matching.length === 1 ? (matching[0] ?? null) : null;
}

/** Keep prompt detection conservative while honoring authored labels and CONDITIONING flow. */
function hasPromptEvidence(node: ComfyNode, widget: ComfyWidget): boolean {
  return promptRoleFor(node, widget) !== null || hasPromptToken(node, widget);
}

/** Resolve a semantic prompt role from authored labels before following graph conditioning flow. */
function promptRoleFor(node: ComfyNode, widget: ComfyWidget): PromptRole | null {
  const evidence = tokens(
    `${node.title ?? ''} ${node.type ?? ''} ${node.class_type ?? ''} ${widget.name}`,
  );
  return roleFromTokens(evidence) ?? resolveDownstreamPromptRole(node);
}

/** Return whether a card's authored labels identify it as a generic prompt editor. */
function hasPromptToken(node: ComfyNode, widget: ComfyWidget): boolean {
  return tokens(
    `${node.title ?? ''} ${node.type ?? ''} ${node.class_type ?? ''} ${widget.name}`,
  ).has('prompt');
}

/** Recognize the two multiline widget forms Comfy exposes at this renderer boundary. */
function isEditableMultilineWidget(widget: ComfyWidget): boolean {
  if (widget.options?.multiline === true || widget.type === 'customtext') return true;
  const element = widget.element;
  return typeof HTMLTextAreaElement !== 'undefined' && element instanceof HTMLTextAreaElement;
}

/** Normalize prompt-relevant authored labels without coupling to host localization. */
function tokens(value: string): ReadonlySet<string> {
  return new Set(value.toLowerCase().match(/[a-z0-9]+/g) ?? []);
}

/** Return a role only when exactly one authored role is present. */
function roleFromTokens(values: ReadonlySet<string>): PromptRole | null {
  if (values.has('positive') === values.has('negative')) return null;
  return values.has('positive') ? 'positive' : 'negative';
}

/** Trace CONDITIONING flow to a sampler's named positive or negative input. */
function resolveDownstreamPromptRole(node: ComfyNode): PromptRole | null {
  const graph = node.graph;
  if (!graph || !hasConditioningOutput(node)) return null;
  const nodes = graphNodes(graph);
  const visited = new Set<ComfyNode>();
  const queue: ComfyNode[] = [node];
  let discovered: PromptRole | null = null;

  while (queue.length > 0) {
    const current = queue.shift();
    if (!current || visited.has(current)) continue;
    visited.add(current);
    for (const link of outgoingLinks(graph, current)) {
      const target = nodes.get(String(link.target_id ?? link.target));
      if (!target) continue;
      const input = target.inputs?.[numericSlot(link.target_slot)];
      const role = input ? roleForInput(input.name ?? input.label ?? '') : null;
      if (role) {
        if (discovered && discovered !== role) return null;
        discovered = role;
        continue;
      }
      if (hasConditioningOutput(target)) queue.push(target);
    }
  }
  return discovered;
}

/** Build a stable id lookup for the active Cube subgraph. */
function graphNodes(graph: ComfyGraph): Map<string, ComfyNode> {
  return new Map((graph._nodes ?? graph.nodes ?? []).map((node) => [String(node.id ?? ''), node]));
}

/** Yield graph links originating at one node regardless of Comfy's link collection shape. */
function outgoingLinks(graph: ComfyGraph, node: ComfyNode): ComfyLink[] {
  const ids = (node.outputs ?? []).flatMap((output) => [
    ...(output.links ?? []),
    ...(output.link === null || output.link === undefined ? [] : [output.link]),
  ]);
  return ids.map((id) => resolveLink(graph, id)).filter((link): link is ComfyLink => link !== null);
}

/** Resolve a host link by id without requiring one particular Comfy graph implementation. */
function resolveLink(graph: ComfyGraph, id: GraphId): ComfyLink | null {
  const fromMethod = graph.getLink?.(id);
  if (fromMethod) return fromMethod;
  const links = graph.links ?? graph._links;
  if (links instanceof Map) return links.get(id) ?? null;
  if (Array.isArray(links)) return links.find((link) => link.id === id) ?? null;
  return links?.[String(id)] ?? null;
}

/** Restrict topology inference to nodes that can continue CONDITIONING flow. */
function hasConditioningOutput(node: ComfyNode): boolean {
  return (node.outputs ?? []).some(
    (output) => String(output.type ?? '').toUpperCase() === 'CONDITIONING',
  );
}

/** Read one valid host slot index. */
function numericSlot(value: unknown): number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 ? value : 0;
}

/** Match the exact semantic input names SugarSubstitute uses for prompt roles. */
function roleForInput(value: string): PromptRole | null {
  const normalized = value.trim().toLowerCase();
  return normalized === 'positive' ? 'positive' : normalized === 'negative' ? 'negative' : null;
}
