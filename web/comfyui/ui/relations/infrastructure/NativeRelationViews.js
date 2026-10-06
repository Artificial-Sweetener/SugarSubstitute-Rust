import { nativeRelationCards } from './NativeRelationCardAdapter.js';
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
        const cached = this.#views.get(node);
        if (cached?.surfaces.length &&
            cached.surfaces.every((surface) => surface.root.isConnected && !surface.canvasTitle))
            return cached.surfaces;
        const surfaces = nativeRelationCards(this.document, node, cubeId);
        this.#views.set(node, { cubeId, surfaces });
        return surfaces;
    }
    /** Invalidate only cached views touched by identified native surface changes. */
    invalidate(elements) {
        for (const [node, { cubeId, surfaces }] of this.#views) {
            const locator = `${String(node.graph?.id)}:${String(node.id)}`;
            if (!surfaces.length ||
                elements.some((element) => element.dataset.cubeNodeLocator === locator ||
                    element.closest('.sugarcubes-cube-face__node-card')?.dataset
                        .cubeNodeLocator === locator ||
                    (cubeId === null && element.dataset.nodeId === String(node.id))) ||
                surfaces.some((surface) => !surface.root.isConnected ||
                    elements.some((element) => element.contains(surface.root) || surface.root.contains(element))))
                this.#views.delete(node);
        }
    }
    /** Drop detached component references at workflow replacement. */
    clear() {
        this.#views.clear();
    }
}
