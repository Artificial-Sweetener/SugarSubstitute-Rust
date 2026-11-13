import { isRecord } from '../../types/common.js';
import { cubeStackCartRow } from '../CubeStackCartRow.js';
/** Own the external registry-to-captured-artifact boundary independently of modal lifetimes. */
export class CubeStackCartCatalog {
    #registry;
    #entries = new Map();
    /** Bind the validated artifact owner without preparing native placement state. */
    constructor(registry) {
        this.#registry = registry;
    }
    /** Isolate one complete catalog without changing the live graph or registering definitions. */
    capture(construction, generation) {
        const artifacts = [];
        const library = [];
        const payloads = new Map();
        const nextEntries = new Map();
        for (const { type, descriptor } of this.#registry.entries()) {
            const cached = this.#registry.capturedPayload(type);
            if (!cached)
                throw new Error('Cube cart catalog changed during capture.');
            const fingerprint = JSON.stringify(descriptor);
            const previous = this.#entries.get(type);
            if (descriptor.contentHash && previous?.fingerprint === fingerprint) {
                artifacts.push(previous.artifact);
                library.push(previous.row);
                payloads.set(previous.row.catalogKey, previous.placement);
                nextEntries.set(type, previous);
                continue;
            }
            const catalogKey = generation === 0 ? descriptor.key : `${descriptor.key}:${generation}`;
            if (!isRecord(cached.document))
                throw new Error(`Cube '${descriptor.displayName}' has no portable document.`);
            const metadata = {
                ...cached.cube,
                cube_version: descriptor.version,
                default_alias: descriptor.displayName,
                target_model: descriptor.targetModel,
            };
            const artifact = {
                catalogKey,
                cubeId: descriptor.cubeId,
                version: descriptor.version,
                alias: descriptor.displayName,
                document: cached.document,
                size: construction.previewSize(cached, descriptor.inputs.length > 0, {
                    instanceId: 'cart-frame-preview',
                }),
            };
            const placement = { descriptor, payload: cached };
            const row = {
                ...cubeStackCartRow(catalogKey, metadata, descriptor.displayName, false),
                catalogKey,
                models: descriptor.supportedModels.length
                    ? descriptor.supportedModels
                    : [descriptor.targetModel || 'Other models'],
                role: descriptor.inputs.length === 0
                    ? descriptor.outputs.length
                        ? 'Start cubes'
                        : 'Other cubes'
                    : descriptor.outputs.length
                        ? 'Middle cubes'
                        : 'End cubes',
                searchText: [
                    descriptor.displayName,
                    descriptor.cubeId,
                    descriptor.description,
                    descriptor.targetModel,
                    ...descriptor.supportedModels,
                    ...descriptor.searchTerms,
                ]
                    .join(' ')
                    .toLocaleLowerCase(),
            };
            artifacts.push(artifact);
            library.push(row);
            payloads.set(catalogKey, placement);
            nextEntries.set(type, { fingerprint, artifact, row, placement });
        }
        this.#entries = nextEntries;
        return { artifacts, library, payloads };
    }
}
