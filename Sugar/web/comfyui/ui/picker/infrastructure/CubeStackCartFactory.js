import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { CubeStackCartSession } from '../application/CubeStackCartSession.js';
import { CubeStackCartCatalog } from './CubeStackCartCatalog.js';
import { cubeStackCartRow } from '../CubeStackCartRow.js';
/** Prepare a full opening draft while preserving the existing catalog/import and geometry owners. */
export class CubeStackCartFactory {
    #options;
    /** Bind catalog, placement, canonical gateway and graph-bound runtime lifetimes. */
    constructor(options) {
        this.#options = options;
    }
    /** Capture one complete cart without inserting native nodes or registering embedded definitions. */
    async open() {
        await this.#options.ensureCatalog();
        const runtime = this.#options.getRuntime();
        runtime.graphScope.assertCurrentRoot('edited');
        if (runtime.reorder.pending)
            throw new Error('Wait for Cube reordering to finish before opening the cart.');
        const snapshot = runtime.graphSnapshots.snapshot();
        const analysis = await runtime.reorder.refresh();
        if (!analysis ||
            !runtime.graphSnapshots.available ||
            runtime.graphSnapshots.snapshot().revision !== snapshot.revision)
            throw new Error('Workflow changed while the Cube cart was opening.');
        const catalog = new CubeStackCartCatalog(this.#options.registry);
        const initial = catalog.capture(runtime.construction, 0);
        let library = initial.library;
        const artifactRows = new Map(initial.library.map((row) => [row.catalogKey, row]));
        const payloads = new Map(initial.payloads);
        let catalogGeneration = 0;
        let refreshing = false;
        const existingRows = new Map(runtime.nodes
            .list()
            .map((node) => [String(requireCubeIdentity(node).instance_id), existingRow(node)]));
        const session = new CubeStackCartSession({
            snapshot,
            analysis,
            artifacts: initial.artifacts,
            snapshots: runtime.graphSnapshots,
            gateway: this.#options.gateway,
            createId: this.#options.createId,
            commit: {
                commit: (opening, entries, result) => runtime.stackCommit.commit(opening, result, entries.flatMap((entry) => {
                    if (entry.source === 'existing')
                        return [];
                    const captured = payloads.get(entry.catalogKey);
                    if (!captured)
                        throw new Error('Captured Cube cart artifact is unavailable.');
                    const prepared = this.#options.placement.prepareCaptured(captured.descriptor, captured.payload);
                    return [
                        { instanceId: entry.instanceId, payload: prepared.payload, size: entry.size },
                    ];
                })),
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
                if (refreshing)
                    throw new Error('Cube library refresh is already in progress.');
                refreshing = true;
                try {
                    await this.#options.refreshCatalog();
                    runtime.graphScope.assertCurrentRoot('edited');
                    const next = catalog.capture(runtime.construction, catalogGeneration + 1);
                    session.registerArtifacts(next.artifacts.filter((artifact) => !artifactRows.has(artifact.catalogKey)));
                    for (const [key, payload] of next.payloads)
                        payloads.set(key, payload);
                    for (const row of next.library)
                        artifactRows.set(row.catalogKey, row);
                    library = next.library;
                    catalogGeneration += 1;
                }
                finally {
                    refreshing = false;
                }
            },
        };
    }
}
/** Read shared native identity once before the cart opens without retaining live metadata references. */
function existingRow(node) {
    const metadata = requireCubeIdentity(node);
    return cubeStackCartRow(String(metadata.instance_id), metadata, node.title || node.subgraph.name, node.mode === 4);
}
