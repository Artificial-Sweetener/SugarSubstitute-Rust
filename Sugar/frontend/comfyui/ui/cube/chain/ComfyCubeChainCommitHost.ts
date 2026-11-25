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
/** Apply one Cube chain change to the Comfy graph in a single undoable step, or not at all. */
import type { ImportPayload } from '../../import/PlacementPayload.js';
import type { CubeConstructionService, ConstructedCube } from '../CubeConstructionService.js';
import type { ComfyCubeHistory } from '../ComfyCubeHistoryAdapter.js';
import type { ComfyCubeRootMutationAdapter } from '../ComfyCubeRootMutationAdapter.js';
import type {
  CubeEmbeddedDefinitionStager,
  StagedCubeDefinitions,
} from '../graph/CubeEmbeddedDefinitionStager.js';
import {
  readNativeCubePosition,
  writeNativeCubePosition,
  restoreNativeCubePosition,
} from '../graph/NativeCubePositions.js';
import {
  captureNativeCubeConnections,
  readNativeCubeConnection,
  restoreNativeCubeConnections,
  type NativeCubeConnection,
} from '../graph/NativeCubeConnections.js';
import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import { readInstanceId, type CubeNodeCatalog } from '../node/CubeNodeCatalog.js';
import type { RewireLink } from './CubeChainRewire.js';

/** A Cube to construct from a catalog payload. */
export interface CubeChainCreation {
  readonly instanceId: string;
  readonly alias: string;
  readonly payload: ImportPayload;
  readonly size: readonly [number, number];
}

/** A Cube node built already, such as a duplicate, to add to the graph. */
export interface CubeChainAddition {
  readonly node: CubeNode;
}

/** Everything one chain change does to the graph. */
export interface CubeChainChange {
  readonly creations: readonly CubeChainCreation[];
  readonly additions: readonly CubeChainAddition[];
  readonly removals: readonly string[];
  readonly positions: ReadonlyMap<string, readonly [number, number]>;
  readonly links: { readonly remove: readonly RewireLink[]; readonly add: readonly RewireLink[] };
}

interface ChainCommitOptions {
  readonly graph: Pick<ComfyCubeRootMutationAdapter, 'add' | 'remove' | 'getNodeById' | 'getLink'>;
  readonly nodes: CubeNodeCatalog;
  readonly history: ComfyCubeHistory;
  readonly construction: Pick<CubeConstructionService, 'construct' | 'discard'>;
  readonly definitions: Pick<CubeEmbeddedDefinitionStager, 'stage' | 'discard'>;
  /** Bring workflow features that follow the stack up to date, inside the same undo step. */
  reconcile(): void;
  /** Tell Comfy's graph it changed, so the workflow and every graph observer see the change. */
  announce(): void;
}

/** A native node that can drop one input link. */
interface DisconnectableNode {
  disconnectInput(slot: number): unknown;
}

/** Keep construction, removal, geometry, wiring and their compensation at one host boundary. */
export class ComfyCubeChainCommitHost {
  readonly #options: ChainCommitOptions;

  /** Bind the root graph, Cube inventory, history and construction owners. */
  constructor(options: ChainCommitOptions) {
    this.#options = options;
  }

  /** Apply the whole change and return the Cubes it added, or restore everything and throw. */
  commit(change: CubeChainChange): CubeNode[] {
    const { graph, nodes, construction, definitions, history } = this.#options;
    const original = nodes.list();
    const byInstance = new Map(original.map((node) => [readInstanceId(node), node]));
    const removed = change.removals.map((id) => {
      const node = byInstance.get(id);
      if (!node) throw new Error('A Cube to remove is no longer in the workflow.');
      return node;
    });
    const positions = new Map(original.map((node) => [node, readNativeCubePosition(node.pos)]));
    if (isUnchanged(change, byInstance)) return [];
    const severed = captureNativeCubeConnections(graph, removed);
    const staged: { definitions: StagedCubeDefinitions; constructed: ConstructedCube }[] = [];
    const inserted: CubeNode[] = [];
    const unlinked: NativeCubeConnection[] = [];
    const linked: NativeCubeConnection[] = [];
    history.beforeChange();
    try {
      nodes.batch(() => {
        try {
          for (const creation of change.creations) {
            const owned = definitions.stage(creation.payload);
            let constructed: ConstructedCube;
            try {
              constructed = construction.construct(owned.payload, {
                instanceId: creation.instanceId,
                instanceAlias: creation.alias,
                position: [...(change.positions.get(creation.instanceId) ?? [0, 0])],
                size: [...creation.size],
              });
            } catch (error: unknown) {
              try {
                definitions.discard(owned);
              } catch (failure: unknown) {
                throw new AggregateError(
                  [error, failure],
                  'Cube construction failed and its definitions could not be cleaned up.',
                );
              }
              throw error;
            }
            staged.push({ definitions: owned, constructed });
            if (graph.getNodeById(constructed.node.id))
              throw new Error('A new Cube’s native node id is already in use.');
            byInstance.set(creation.instanceId, constructed.node);
          }
          for (const node of [
            ...staged.map(({ constructed }) => constructed.node),
            ...change.additions.map((addition) => addition.node),
          ]) {
            inserted.push(node);
            graph.add(node);
            nodes.add(node);
            byInstance.set(readInstanceId(node), node);
          }
          for (const node of removed) {
            graph.remove(node);
            nodes.remove(readInstanceId(node));
          }
          for (const [instanceId, position] of change.positions) {
            const node = byInstance.get(instanceId);
            if (!node) throw new Error('A Cube to place is no longer in the workflow.');
            writeNativeCubePosition(node, position);
          }
          for (const link of change.links.remove) {
            const target = requireNode(byInstance, link.targetInstance);
            const id = target.inputs[link.targetSlot]?.link;
            if (id === null || id === undefined) continue;
            unlinked.push(readNativeCubeConnection(graph, id));
            (target as unknown as DisconnectableNode).disconnectInput(link.targetSlot);
          }
          for (const link of change.links.add) {
            const source = requireNode(byInstance, link.sourceInstance);
            const target = requireNode(byInstance, link.targetInstance);
            source.connect(link.sourceSlot, target, link.targetSlot);
            const id = target.inputs[link.targetSlot]?.link;
            if (id === null || id === undefined)
              throw new Error(`${target.title ?? 'A Cube'} could not be wired.`);
            linked.push(readNativeCubeConnection(graph, id));
          }
          this.#options.reconcile();
          this.#options.announce();
          const first = inserted[0] ?? original[0];
          if (first) nodes.changed(first);
        } catch (error: unknown) {
          compensate(error, [
            ...linked.map(
              (connection) => () =>
                (connection.target as unknown as DisconnectableNode).disconnectInput(
                  connection.targetSlot,
                ),
            ),
            () => restoreNativeCubeConnections(graph, unlinked),
            ...[...inserted].reverse().map((node) => () => {
              if (graph.getNodeById(node.id) === node) graph.remove(node);
              nodes.remove(readInstanceId(node));
            }),
            ...removed.map((node) => () => {
              if (graph.getNodeById(node.id) !== node) graph.add(node);
              nodes.add(node);
            }),
            ...[...positions].map(
              ([node, position]) =>
                () =>
                  restoreNativeCubePosition(node, position),
            ),
            () => restoreNativeCubeConnections(graph, severed),
            ...[...staged]
              .reverse()
              .flatMap((item) => [
                () => construction.discard(item.constructed),
                () => definitions.discard(item.definitions),
              ]),
          ]);
        }
      });
    } finally {
      try {
        history.setDirtyCanvas(true, true);
      } finally {
        history.afterChange();
      }
    }
    return inserted;
  }
}

/** A change that adds, removes and rewires nothing and leaves every Cube where it is. */
function isUnchanged(change: CubeChainChange, nodes: ReadonlyMap<string, CubeNode>): boolean {
  return (
    !change.creations.length &&
    !change.additions.length &&
    !change.removals.length &&
    !change.links.remove.length &&
    !change.links.add.length &&
    [...change.positions].every(([id, position]) => {
      const node = nodes.get(id);
      const now = node ? readNativeCubePosition(node.pos) : null;
      return now !== null && now[0] === position[0] && now[1] === position[1];
    })
  );
}

/** Resolve one Cube of the change by instance id. */
function requireNode(nodes: ReadonlyMap<string, CubeNode>, instanceId: string): CubeNode {
  const node = nodes.get(instanceId);
  if (!node) throw new Error('A Cube to wire is no longer in the workflow.');
  return node;
}

/** Undo every step that ran, then rethrow the failure with any undo that failed beside it. */
function compensate(error: unknown, steps: readonly (() => void)[]): never {
  const failures: unknown[] = [];
  for (const step of steps) {
    try {
      step();
    } catch (failure: unknown) {
      failures.push(failure);
    }
  }
  if (failures.length)
    throw new AggregateError(
      [error, ...failures],
      'A Cube change failed and could not be fully undone.',
    );
  throw error;
}
