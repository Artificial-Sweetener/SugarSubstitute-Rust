import { isRecord } from '../types/common.js';
/** Fetch qualification facts without guessing whether arbitrary output nodes perform saving. */
export class OutputSavingCapabilitiesClient {
    api;
    /** Bind the existing host-addressed transport. */
    constructor(api) {
        this.api = api;
    }
    /** Bound catalog retrieval and narrow every dynamic class declaration. */
    async load() {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), 20000);
        try {
            const { response, data } = await this.api.fetchJson('/sugarcubes/output-saving/capabilities', { signal: controller.signal });
            if (response.ok === false ||
                data.success !== true ||
                !isRecord(data.data) ||
                !Array.isArray(data.data.nodes))
                throw new Error('Output saving capabilities are unavailable.');
            const result = new Map();
            for (const node of data.data.nodes) {
                if (!isRecord(node) ||
                    typeof node.class_type !== 'string' ||
                    !node.class_type ||
                    typeof node.managed !== 'boolean' ||
                    (node.kind !== 'save' && node.kind !== 'preview' && node.kind !== null) ||
                    (node.managed && node.kind === null) ||
                    result.has(node.class_type))
                    throw new Error('Invalid output saving capabilities.');
                result.set(node.class_type, Object.freeze({ classType: node.class_type, managed: node.managed, kind: node.kind }));
            }
            return result;
        }
        finally {
            clearTimeout(timer);
        }
    }
}
