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
/** Find the multiline prompts that condition a regional mask loader's consumers. */
import { parsePrompt } from '../../../../sugarprompt/core/src/index.js';
import type { ComfyNode, ComfyWidget } from '../../types/graph.js';
import type { SpatialTopology } from '../domain/CanvasDimensionAuthority.js';
import { regionalPromptDemand, type RegionalPromptDemand } from '../domain/RegionalPromptDemand.js';

/** Upstream hops searched from each consumer of the mask. */
const PROMPT_HOPS = 3;
/** Node type whose text carries regional `[SEP]` separators. */
const PROMPT_NODE = 'PrimitiveStringMultiline';

/**
 * Walk up to three hops upstream from every consumer of the mask and read each multiline
 * prompt found, in discovery order. Prompt nodes end their branch of the walk. Separators are
 * parsed by SugarPrompt, which owns the `[SEP]` and `[SEP|name]` syntax.
 */
export function readRegionalPrompts(
  topology: SpatialTopology,
  mask: string,
  node: (path: string) => ComfyNode | null,
): { readonly demand: RegionalPromptDemand; readonly nodes: readonly ComfyNode[] } {
  const incoming = new Map<string, string[]>();
  for (const edge of topology.edges) {
    const sources = incoming.get(edge.target) ?? [];
    sources.push(edge.source);
    incoming.set(edge.target, sources);
  }
  const consumers = topology.edges
    .filter((edge) => edge.source === mask && edge.type === 'MASK')
    .map((edge) => edge.target);
  const prompts: { readonly path: string; readonly node: ComfyNode }[] = [],
    seen = new Set<string>([mask]);
  let frontier = [...new Set(consumers)];
  for (let hop = 0; hop < PROMPT_HOPS && frontier.length; hop++) {
    const next: string[] = [];
    for (const target of frontier)
      for (const source of incoming.get(target) ?? []) {
        if (seen.has(source)) continue;
        seen.add(source);
        const live = node(source);
        if ((live?.type ?? live?.class_type) === PROMPT_NODE) {
          if (live) prompts.push({ path: source, node: live });
        } else next.push(source);
      }
    frontier = next;
  }
  return {
    demand: regionalPromptDemand(
      prompts.map(({ path, node }) => ({
        path,
        separators: parsePrompt(promptField(node)?.value ?? '').regions.separators.map(
          (separator) => separator.name,
        ),
      })),
    ),
    nodes: prompts.map(({ node }) => node),
  };
}

/** Find a prompt node's authored text field: `value`, or `text` when absent. */
export function promptField(
  node: ComfyNode,
): { readonly widget: ComfyWidget; readonly value: string } | null {
  for (const name of ['value', 'text']) {
    const widget = node.widgets?.find((candidate) => candidate.name === name);
    if (widget && typeof widget.value === 'string') return { widget, value: widget.value };
  }
  return null;
}
