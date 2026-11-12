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
/** Apply a complete canonical cart in one native history transaction while retaining existing Cube objects. */
import type { ImportPayload } from '../../import/PlacementPayload.js';
import type {
  CubeGraphAnalysis,
  CubeGraphSnapshot,
} from '../../workflow/domain/CubeGraphModels.js';
import type { CubeGraphSnapshotSource } from '../../workflow/domain/CubeGraphStackModels.js';
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
  restoreNativeCubeConnections,
} from '../graph/NativeCubeConnections.js';
import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import { readInstanceId, type CubeNodeCatalog } from '../node/CubeNodeCatalog.js';

/** Carry one prepared artifact captured by the cart, independently of later catalog refreshes. */
export interface CubeStackCreation {
  readonly instanceId: string;
  readonly payload: ImportPayload;
  readonly size: readonly [number, number];
}

interface StackCommitOptions {
  readonly snapshots: CubeGraphSnapshotSource;
  readonly graph: Pick<ComfyCubeRootMutationAdapter, 'add' | 'remove' | 'getNodeById' | 'getLink'>;
  readonly nodes: CubeNodeCatalog;
  readonly history: ComfyCubeHistory;
  readonly construction: Pick<CubeConstructionService, 'construct' | 'discard'>;
  readonly definitions: Pick<CubeEmbeddedDefinitionStager, 'stage' | 'discard'>;
}

interface StagedCreation {
  readonly definitions: StagedCubeDefinitions;
  readonly constructed: ConstructedCube;
}

/** Keep native staging, root mutations, coherent catalog publication and compensation at one host boundary. */
export class ComfyCubeStackCommitHost {
  readonly #options: StackCommitOptions;

  /** Bind the established snapshot, history, definition and root mutation owners. */
  constructor(options: StackCommitOptions) {
    this.#options = options;
  }

  /** Apply one complete validated plan or compensate every mutation before completing native history. */
  commit(
    snapshot: CubeGraphSnapshot,
    result: CubeGraphAnalysis,
    creations: readonly CubeStackCreation[],
  ): void {
    const { snapshots, graph, nodes, construction, definitions, history } = this.#options;
    if (!snapshots.available || snapshots.snapshot().revision !== snapshot.revision)
      throw new Error('Workflow changed before the Cube cart could be applied.');
    const original = nodes.list();
    const byInstance = new Map(original.map((node) => [readInstanceId(node), node]));
    const instances = new Map(result.instances.map((instance) => [instance.instanceId, instance]));
    const placements = new Map(
      result.placements.map((placement) => [placement.instanceId, placement]),
    );
    const additions = new Map(creations.map((creation) => [creation.instanceId, creation]));
    if (
      instances.size !== result.instances.length ||
      placements.size !== instances.size ||
      placements.size !== result.placements.length ||
      additions.size !== creations.length
    )
      throw new Error('Cube cart plan contains duplicate or incomplete native targets.');
    if (
      byInstance.size !== original.length ||
      original.some((node) => graph.getNodeById(node.id) !== node)
    )
      throw new Error('Cube cart inventory no longer belongs to the current native graph.');
    for (const instance of result.instances) {
      const node = byInstance.get(instance.instanceId);
      const placement = placements.get(instance.instanceId);
      if (!placement || placement.nodeId !== instance.nodeId)
        throw new Error('Cube cart plan has an incomplete placement.');
      readNativeCubePosition(placement.position);
      if (node) {
        if (
          String(node.id) !== instance.nodeId ||
          graph.getNodeById(node.id) !== node ||
          additions.has(instance.instanceId)
        )
          throw new Error('Cube cart plan changed an existing native identity.');
      } else if (!additions.has(instance.instanceId))
        throw new Error('Cube cart plan contains an unprepared artifact.');
    }
    if (creations.some((creation) => !instances.has(creation.instanceId)))
      throw new Error('Cube cart contains an unrequested native construction.');
    const removed = original.filter((node) => !instances.has(readInstanceId(node)));
    const positions = new Map(original.map((node) => [node, readNativeCubePosition(node.pos)]));
    if (
      removed.length === 0 &&
      creations.length === 0 &&
      original.every((node) => {
        const target = placements.get(readInstanceId(node));
        const position = positions.get(node);
        return (
          target &&
          position &&
          target.position[0] === position[0] &&
          target.position[1] === position[1]
        );
      })
    )
      return;
    const connections = captureNativeCubeConnections(graph, removed);
    const staged: StagedCreation[] = [];
    const inserted: CubeNode[] = [];
    history.beforeChange();
    try {
      nodes.batch(() => {
        try {
          for (const creation of creations) {
            const instance = instances.get(creation.instanceId);
            const placement = placements.get(creation.instanceId);
            if (!instance || !placement)
              throw new Error('Cube cart construction lost its canonical target.');
            const owned = definitions.stage(creation.payload);
            let constructed: ConstructedCube;
            try {
              constructed = construction.construct(owned.payload, {
                instanceId: creation.instanceId,
                instanceAlias: instance.alias,
                position: [...placement.position],
                size: [...creation.size],
              });
            } catch (error: unknown) {
              try {
                definitions.discard(owned);
              } catch (failure: unknown) {
                throw new AggregateError(
                  [error, failure],
                  'Cube construction failed and embedded definition cleanup was incomplete.',
                );
              }
              throw error;
            }
            staged.push({ definitions: owned, constructed });
            if (graph.getNodeById(constructed.node.id))
              throw new Error('Cube cart native node identifier is already in use.');
            byInstance.set(creation.instanceId, constructed.node);
          }
          for (const { constructed } of staged) {
            inserted.push(constructed.node);
            graph.add(constructed.node);
            nodes.add(constructed.node);
          }
          for (const node of removed) {
            graph.remove(node);
            nodes.remove(readInstanceId(node));
          }
          for (const instance of result.instances) {
            const node = byInstance.get(instance.instanceId);
            const placement = placements.get(instance.instanceId);
            if (!node || !placement)
              throw new Error('Cube cart target is unavailable during native commit.');
            writeNativeCubePosition(node, placement.position);
          }
          const first = result.instances[0];
          const changed = first ? byInstance.get(first.instanceId) : undefined;
          if (changed) nodes.changed(changed);
        } catch (error: unknown) {
          const failures: unknown[] = [];
          const compensate = (work: () => void): void => {
            try {
              work();
            } catch (failure: unknown) {
              failures.push(failure);
            }
          };
          for (const node of [...inserted].reverse())
            compensate(() => {
              if (graph.getNodeById(node.id) === node) graph.remove(node);
              nodes.remove(readInstanceId(node));
            });
          for (const node of removed)
            compensate(() => {
              if (graph.getNodeById(node.id) !== node) graph.add(node);
              nodes.add(node);
            });
          for (const [node, position] of positions)
            compensate(() => restoreNativeCubePosition(node, position));
          compensate(() => restoreNativeCubeConnections(graph, connections));
          for (const item of [...staged].reverse()) {
            compensate(() => construction.discard(item.constructed));
            compensate(() => definitions.discard(item.definitions));
          }
          if (failures.length)
            throw new AggregateError(
              [error, ...failures],
              'Cube cart failed and native compensation was incomplete.',
            );
          throw error;
        }
      });
    } finally {
      try {
        history.setDirtyCanvas(true, true);
      } finally {
        history.afterChange();
      }
    }
  }
}
