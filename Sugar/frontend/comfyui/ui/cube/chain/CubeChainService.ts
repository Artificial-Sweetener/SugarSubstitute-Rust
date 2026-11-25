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
/**
 * The root workflow's Cube chain, read from and written to the Comfy graph itself.
 *
 * Every surface that shows or changes the stack goes through here: masonry and the stack rail,
 * the Cube arrows, dragging, the add button, the Add cubes dialog and duplicating. The order is
 * read from the graph each time it is asked for, so wiring a Cube differently on the graph
 * changes the stack, and a stack change edits the graph at once.
 */
import type { ComfyGraph } from '../../types/graph.js';
import type { ProximityMatch } from '../../overlays/proximity/ProximityModel.js';
import { resolveGraphLink } from '../../surface/CubeFaceCardPolicy.js';
import { readNativeCubePosition } from '../graph/NativeCubePositions.js';
import { requireCubeIdentity, type CubeNode } from '../node/ComfyCubeNodeFactory.js';
import { readInstanceId, type CubeNodeCatalog } from '../node/CubeNodeCatalog.js';
import { cubeChainOrder, readLiveChain } from './CubeChainOrder.js';
import { planChain, type ChainEntry, type PlacedFrame } from './CubeChainPlacement.js';
import {
  planRewire,
  type RewireCube,
  type RewireLink,
  type RewirePlan,
} from './CubeChainRewire.js';
import type { CubeChainChange, CubeChainCreation } from './ComfyCubeChainCommitHost.js';
import { cubeStackView, type CubeStackView } from './CubeStackView.js';

/** One entry of a requested stack: a Cube in the workflow, or a new one to create. */
export type CubeChainRequest =
  | { readonly source: 'existing'; readonly instanceId: string }
  | ({ readonly source: 'new' } & CubeChainCreation);

interface CubeChainServiceOptions {
  readonly graph: ComfyGraph;
  readonly nodes: CubeNodeCatalog;
  readonly commit: { commit(change: CubeChainChange): CubeNode[] };
  isCurrentRoot(): boolean;
  matches(): readonly ProximityMatch[];
  accepts(outputType: unknown, inputType: unknown): boolean;
}

/** Own the stack order, placement and wiring of the live root workflow. */
export class CubeChainService {
  readonly #options: CubeChainServiceOptions;
  readonly #listeners = new Set<() => void>();

  /** Bind the root graph, its Cube inventory and the one commit transaction. */
  constructor(options: CubeChainServiceOptions) {
    this.#options = options;
  }

  /** Stack changes apply only to the root workflow. */
  get available(): boolean {
    return this.#options.isCurrentRoot();
  }

  /** Observe changes that can move Cubes in the stack. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Tell every stack surface that the graph changed. */
  changed(): void {
    for (const listener of this.#listeners) listener();
  }

  /** The stack: every Cube of the root workflow in the order it runs. */
  order(): readonly string[] {
    return this.#order(this.#options.matches());
  }

  /** The stack as the backend is told it: the order and the proximity links the page shows. */
  view(): CubeStackView {
    const matches = this.#options.matches();
    return cubeStackView(this.#order(matches), matches);
  }

  /** Read the order with one set of proximity matches. */
  #order(matches: readonly ProximityMatch[]): readonly string[] {
    const { cubes, edges } = readLiveChain(
      this.#options.graph,
      this.#options.nodes.list(),
      readInstanceId,
      matches,
    );
    return cubeChainOrder(cubes, edges);
  }

  /** Whether a Cube can move one place, including whether its wiring survives the move. */
  canMove(instanceId: string, delta: -1 | 1): boolean {
    if (!this.available) return false;
    const requested = swapped(this.order(), instanceId, delta);
    return requested !== null && this.#rewire(this.order(), requested).ok;
  }

  /** Move a Cube one place in the stack. */
  move(instanceId: string, delta: -1 | 1): void {
    const requested = swapped(this.order(), instanceId, delta);
    if (!requested) throw new Error('That Cube is already at the end of the stack.');
    this.reorder(requested);
  }

  /** Put the workflow's Cubes in a new order: floating Cubes move, hardwired Cubes are rewired. */
  reorder(requested: readonly string[]): void {
    const current = this.order();
    if (
      requested.length !== current.length ||
      new Set(requested).size !== requested.length ||
      requested.some((id) => !current.includes(id))
    )
      throw new Error('A stack order must hold every Cube of the workflow exactly once.');
    if (requested.every((id, index) => id === current[index])) return;
    this.apply(requested.map((instanceId) => ({ source: 'existing', instanceId })));
  }

  /** Add one new Cube right after another in the stack. */
  insertAfter(sourceInstanceId: string, creation: CubeChainCreation): CubeNode {
    const current = this.order();
    const index = current.indexOf(sourceInstanceId);
    if (index < 0) throw new Error('That Cube is no longer in the workflow.');
    const requested: CubeChainRequest[] = current.map((instanceId) => ({
      source: 'existing',
      instanceId,
    }));
    requested.splice(index + 1, 0, { source: 'new', ...creation });
    const [node] = this.apply(requested);
    if (!node) throw new Error('The new Cube was not added.');
    return node;
  }

  /** Add an already built Cube, such as a duplicate, at the end of the stack. */
  append(node: CubeNode): void {
    const requested: ChainEntry[] = [
      ...this.order().map((id) => ({ id, existing: true as const })),
      { id: readInstanceId(node), existing: false, size: readNativeCubePosition(node.size) },
    ];
    const positions = planChain(this.#frames(), requested);
    this.#commit({
      creations: [],
      additions: [{ node }],
      removals: [],
      positions,
      links: { remove: [], add: [] },
    });
  }

  /**
   * Make the stack exactly `requested`: Cubes left out are removed, new ones are created, and the
   * chain is laid out and wired in that order. Returns the new Cubes in requested order.
   */
  apply(requested: readonly CubeChainRequest[]): CubeNode[] {
    if (!this.available) throw new Error('Open the main workflow to change its Cube stack.');
    const current = this.order();
    const ids = requested.map((entry) => entry.instanceId);
    if (new Set(ids).size !== ids.length) throw new Error('The stack holds a Cube twice.');
    const missing = requested.find(
      (entry) => entry.source === 'existing' && !current.includes(entry.instanceId),
    );
    if (missing) throw new Error('A Cube in the stack is no longer in the workflow.');
    const kept = ids.filter((id) => current.includes(id));
    const rewire = this.#rewire(
      current.filter((id) => kept.includes(id)),
      kept,
    );
    if (!rewire.ok) throw new Error(rewire.reason);
    const aliases = new Set(this.#options.nodes.list().map(aliasOf));
    const creations = requested.flatMap((entry) => {
      if (entry.source === 'existing') return [];
      const alias = uniqueAlias(entry.alias, aliases);
      aliases.add(alias);
      return [{ ...entry, alias }];
    });
    const positions = planChain(
      this.#frames(),
      requested.map((entry) =>
        entry.source === 'existing'
          ? { id: entry.instanceId, existing: true }
          : { id: entry.instanceId, existing: false, size: entry.size },
      ),
    );
    const created = this.#commit({
      creations,
      additions: [],
      removals: current.filter((id) => !ids.includes(id)),
      positions,
      links: rewire,
    });
    const byId = new Map(created.map((node) => [readInstanceId(node), node]));
    return creations.flatMap((creation) => {
      const node = byId.get(creation.instanceId);
      return node ? [node] : [];
    });
  }

  /** Commit one change and tell the stack surfaces. */
  #commit(change: CubeChainChange): CubeNode[] {
    const created = this.#options.commit.commit(change);
    this.changed();
    return created;
  }

  /** Read every Cube's frame as the graph holds it now. */
  #frames(): PlacedFrame[] {
    return this.#options.nodes.list().map((node) => ({
      id: readInstanceId(node),
      position: readNativeCubePosition(node.pos),
      size: readNativeCubePosition(node.size),
    }));
  }

  /** Plan the wiring a new order needs among the Cubes already in the workflow. */
  #rewire(current: readonly string[], requested: readonly string[]): RewirePlan {
    const nodes = this.#options.nodes.list();
    const byNodeId = new Map(nodes.map((node) => [String(node.id), node]));
    const cubes: RewireCube[] = nodes.map((node) => ({
      instanceId: readInstanceId(node),
      title: node.title?.trim() || node.subgraph?.name || 'A Cube',
      inputs: (node.inputs ?? []).map((input, slot) => ({
        slot,
        name: String(input.name ?? ''),
        type: input.type,
      })),
      outputs: (node.outputs ?? []).map((output, slot) => ({
        slot,
        name: String(output.name ?? ''),
        type: output.type,
      })),
    }));
    const links: RewireLink[] = [];
    for (const node of nodes) {
      for (const [targetSlot, input] of (node.inputs ?? []).entries()) {
        const link =
          input.link === null || input.link === undefined
            ? null
            : resolveGraphLink(this.#options.graph, input.link);
        const origin = link ? byNodeId.get(String(link.origin_id ?? link.origin)) : undefined;
        if (!link || !origin || typeof link.origin_slot !== 'number') continue;
        links.push({
          sourceInstance: readInstanceId(origin),
          sourceSlot: link.origin_slot,
          targetInstance: readInstanceId(node),
          targetSlot,
        });
      }
    }
    return planRewire(cubes, links, current, requested, this.#options.accepts);
  }
}

/** Swap a Cube with its neighbour, or null at either end of the stack. */
function swapped(order: readonly string[], instanceId: string, delta: -1 | 1): string[] | null {
  const index = order.indexOf(instanceId);
  const other = order[index + delta];
  if (index < 0 || other === undefined) return null;
  const next = [...order];
  next[index] = other;
  next[index + delta] = instanceId;
  return next;
}

/** Read the workflow-local name a Cube is shown by. */
function aliasOf(node: CubeNode): string {
  const alias = requireCubeIdentity(node).instance_alias;
  return typeof alias === 'string' ? alias.trim() : '';
}

/** Give a repeated Cube a readable name of its own: "Upscale", "Upscale 2", "Upscale 3". */
export function uniqueAlias(requested: string, occupied: ReadonlySet<string>): string {
  const alias = requested.trim();
  if (!occupied.has(alias)) return alias;
  const match = /^(.+?) (\d+)$/.exec(alias);
  const stem = match?.[1] ?? alias;
  let suffix = match?.[2] ? Number(match[2]) + 1 : 2;
  while (occupied.has(`${stem} ${String(suffix)}`)) suffix += 1;
  return `${stem} ${String(suffix)}`;
}
