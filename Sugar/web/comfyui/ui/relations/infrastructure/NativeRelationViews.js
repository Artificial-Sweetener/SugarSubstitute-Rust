import { nativeRelationCards } from './NativeRelationCardAdapter.js';
import { readNativeNodeCard } from '../../surface/NativeNodeCardBoundary.js';
/** Cache native component discovery until that exact surface's contents or lifetime changes. */
export class NativeRelationViews {
    document;
    #views = new Map();
    /** Bind discovery to the document that owns mounted native components. */
    constructor(document) {
        this.document = document;
    }
    /** Resolve one face's retained views without inspecting unrelated Vue components. */
    read(node, cubeId) {
        let instances = this.#views.get(node);
        const cached = instances?.get(cubeId);
        if (cached?.length &&
            cached.every((surface) => {
                const boundary = readNativeNodeCard(surface.root);
                return (surface.root.isConnected &&
                    !surface.canvasTitle &&
                    (cubeId === null || (boundary?.node === node && boundary.cubeId === cubeId)));
            }))
            return cached;
        const surfaces = nativeRelationCards(this.document, node, cubeId);
        if (!instances)
            this.#views.set(node, (instances = new Map()));
        instances.set(cubeId, surfaces);
        return surfaces;
    }
    /** Invalidate only cached views touched by identified native surface changes. */
    invalidate(elements) {
        for (const [node, instances] of this.#views) {
            for (const [cubeId, surfaces] of instances) {
                if (!surfaces.length ||
                    elements.some((element) => {
                        const boundary = readNativeNodeCard(element);
                        return ((boundary?.node === node && boundary.cubeId === cubeId) ||
                            (cubeId === null && element.dataset.nodeId === String(node.id)));
                    }) ||
                    surfaces.some((surface) => !surface.root.isConnected ||
                        elements.some((element) => element.contains(surface.root) || surface.root.contains(element))))
                    instances.delete(cubeId);
            }
            if (!instances.size)
                this.#views.delete(node);
        }
    }
    /** Drop detached component references at workflow replacement. */
    clear() {
        this.#views.clear();
    }
}
