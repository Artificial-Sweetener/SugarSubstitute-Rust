import { createComfyInputBinding, isComfyInputSourceLinked, } from './ComfyInputBinding.js';
import { getGraphNodes } from '../../graph/GraphQuery.js';
import { isRecord } from '../../types/common.js';
/** Restore bound-node observers at native workflow loading, with no DOM-driven graph discovery. */
export class ComfyInputBindings {
    loaders;
    ports;
    #nodes = new Map();
    /** Share canvas and file adapters while keeping node callbacks scoped to the loaded workflow. */
    constructor(loaders, ports) {
        this.loaders = loaders;
        this.ports = ports;
    }
    /** Reuse one authoritative controller across every presentation of the same live node. */
    get(node, loader) {
        let owner = this.#nodes.get(node);
        if (!owner) {
            const previous = node.onRemoved;
            const removed = (...args) => {
                try {
                    return typeof previous === 'function' ? previous.apply(node, args) : undefined;
                }
                finally {
                    this.#release(node);
                }
            };
            node.onRemoved = removed;
            owner = {
                bindings: new Map(),
                stop: () => {
                    if (node.onRemoved === removed)
                        node.onRemoved = previous;
                },
            };
            this.#nodes.set(node, owner);
        }
        const current = owner.bindings.get(loader.field);
        if (current)
            return current;
        const binding = createComfyInputBinding(node, loader, this.ports);
        owner.bindings.set(loader.field, binding);
        binding.adoptFiles();
        void binding.initialize();
        return binding;
    }
    /** Restore selectable sources through Comfy's loaded-node lifecycle, including unconfigured loaders. */
    loaded(node) {
        this.#loaded(node, new Set());
    }
    /** Follow only the exact loaded instance's native descendants, rejecting recursive graph references. */
    #loaded(node, seen) {
        if (seen.has(node))
            return;
        seen.add(node);
        if (isRecord(node.subgraph))
            for (const child of getGraphNodes(node.subgraph))
                this.#loaded(child, seen);
        for (const loader of this.loaders.all(node.type ?? node.class_type ?? '')) {
            if (isComfyInputSourceLinked(node, loader.field))
                continue;
            try {
                this.get(node, loader);
            }
            catch (error) {
                this.ports.report(error);
            }
        }
    }
    /** Release node observers before a new workflow replaces their native instances. */
    clear() {
        for (const node of this.#nodes.keys())
            this.#release(node);
    }
    /** Negotiate newly authenticated source capabilities for exact known loader instances. */
    async ready() {
        await Promise.all([...this.#nodes.values()].flatMap((owner) => [...owner.bindings.values()].map((binding) => binding.initialize())));
    }
    /** Restore only callbacks still owned by the retiring node instance. */
    #release(node) {
        const record = this.#nodes.get(node);
        if (!record)
            return;
        this.#nodes.delete(node);
        for (const binding of record.bindings.values())
            binding.dispose();
        record.stop();
    }
}
