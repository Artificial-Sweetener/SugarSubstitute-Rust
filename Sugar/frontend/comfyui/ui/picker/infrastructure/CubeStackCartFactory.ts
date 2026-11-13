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
/** Capture exact existing workflow and catalog artifacts through their established authoritative adapters. */
import type { ComfyCubeRuntime } from '../../cube/ComfyCubeRuntime.js';
import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { CubeGraphStackGateway } from '../../workflow/domain/CubeGraphStackModels.js';
import type { CubeStackRowModel } from '../../surface/stack/CubeStackView.js';
import { CubeStackCartSession } from '../application/CubeStackCartSession.js';
import type { CubePickerCatalogRegistry } from '../CubePickerCatalogRegistry.js';
import type { CubePickerPlacementAdapter } from '../CubePickerPlacementAdapter.js';
import type { CubeStackLibraryRow } from '../CubeStackCartView.js';
import { CubeStackCartCatalog } from './CubeStackCartCatalog.js';
import { cubeStackCartRow } from '../CubeStackCartRow.js';

/** Supply one immutable opening state and a session bound to its captured graph lifetime. */
export interface CapturedCubeStackCart {
  readonly session: CubeStackCartSession;
  readonly library: readonly CubeStackLibraryRow[];
  readonly existingRows: ReadonlyMap<string, CubeStackRowModel>;
  readonly artifactRows: ReadonlyMap<string, CubeStackLibraryRow>;
  refreshLibrary(): Promise<void>;
}

/** Require only the graph, construction geometry and commit ports used during cart capture. */
interface CubeStackCartRuntime {
  readonly graphScope: Pick<ComfyCubeRuntime['graphScope'], 'assertCurrentRoot'>;
  readonly reorder: Pick<ComfyCubeRuntime['reorder'], 'pending' | 'refresh'>;
  readonly graphSnapshots: ComfyCubeRuntime['graphSnapshots'];
  readonly construction: Pick<ComfyCubeRuntime['construction'], 'previewSize'>;
  readonly nodes: Pick<ComfyCubeRuntime['nodes'], 'list'>;
  readonly stackCommit: ComfyCubeRuntime['stackCommit'];
}

interface CartFactoryOptions {
  readonly registry: Pick<CubePickerCatalogRegistry, 'entries' | 'capturedPayload'>;
  readonly placement: Pick<CubePickerPlacementAdapter, 'prepareCaptured'>;
  readonly gateway: CubeGraphStackGateway;
  getRuntime(): CubeStackCartRuntime;
  ensureCatalog(): Promise<void>;
  refreshCatalog(): Promise<void>;
  createId(): string;
}

/** Prepare a full opening draft while preserving the existing catalog/import and geometry owners. */
export class CubeStackCartFactory {
  readonly #options: CartFactoryOptions;
  /** Bind catalog, placement, canonical gateway and graph-bound runtime lifetimes. */
  constructor(options: CartFactoryOptions) {
    this.#options = options;
  }

  /** Capture one complete cart without inserting native nodes or registering embedded definitions. */
  async open(): Promise<CapturedCubeStackCart> {
    await this.#options.ensureCatalog();
    const runtime = this.#options.getRuntime();
    runtime.graphScope.assertCurrentRoot('edited');
    if (runtime.reorder.pending)
      throw new Error('Wait for Cube reordering to finish before opening the cart.');
    const snapshot = runtime.graphSnapshots.snapshot();
    const analysis = await runtime.reorder.refresh();
    if (
      !analysis ||
      !runtime.graphSnapshots.available ||
      runtime.graphSnapshots.snapshot().revision !== snapshot.revision
    )
      throw new Error('Workflow changed while the Cube cart was opening.');
    const catalog = new CubeStackCartCatalog(this.#options.registry);
    const initial = catalog.capture(runtime.construction, 0);
    let library = initial.library;
    const artifactRows = new Map(initial.library.map((row) => [row.catalogKey, row]));
    const payloads = new Map(initial.payloads);
    let catalogGeneration = 0;
    let refreshing = false;
    const existingRows = new Map(
      runtime.nodes
        .list()
        .map((node) => [String(requireCubeIdentity(node).instance_id), existingRow(node)]),
    );
    const session = new CubeStackCartSession({
      snapshot,
      analysis,
      artifacts: initial.artifacts,
      snapshots: runtime.graphSnapshots,
      gateway: this.#options.gateway,
      createId: this.#options.createId,
      commit: {
        commit: (opening, entries, result) =>
          runtime.stackCommit.commit(
            opening,
            result,
            entries.flatMap((entry) => {
              if (entry.source === 'existing') return [];
              const captured = payloads.get(entry.catalogKey);
              if (!captured) throw new Error('Captured Cube cart artifact is unavailable.');
              const prepared = this.#options.placement.prepareCaptured(
                captured.descriptor,
                captured.payload,
              );
              return [
                { instanceId: entry.instanceId, payload: prepared.payload, size: entry.size },
              ];
            }),
          ),
      },
    });
    return {
      session,
      get library() {
        return library;
      },
      existingRows,
      artifactRows,
      refreshLibrary: async () => {
        if (refreshing) throw new Error('Cube library refresh is already in progress.');
        refreshing = true;
        try {
          await this.#options.refreshCatalog();
          runtime.graphScope.assertCurrentRoot('edited');
          const next = catalog.capture(runtime.construction, catalogGeneration + 1);
          session.registerArtifacts(
            next.artifacts.filter((artifact) => !artifactRows.has(artifact.catalogKey)),
          );
          for (const [key, payload] of next.payloads) payloads.set(key, payload);
          for (const row of next.library) artifactRows.set(row.catalogKey, row);
          library = next.library;
          catalogGeneration += 1;
        } finally {
          refreshing = false;
        }
      },
    };
  }
}

/** Read shared native identity once before the cart opens without retaining live metadata references. */
function existingRow(node: CubeNode): CubeStackRowModel {
  const metadata = requireCubeIdentity(node);
  return cubeStackCartRow(
    String(metadata.instance_id),
    metadata,
    node.title || node.subgraph.name,
    node.mode === 4,
  );
}
