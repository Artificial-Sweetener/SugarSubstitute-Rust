/** Translate face interaction without reproducing graph permissions or geometry. */
export class CubeNodeSwapCoordinator {
    #options;
    /** Share exactly the same chain used by stack dragging and keyboard movement. */
    constructor(options) {
        this.#options = options;
    }
    /** Whether this Cube can move that way, wiring included. */
    canSwap(metadata, direction) {
        const instanceId = readInstanceId(metadata);
        return instanceId !== null && this.#options.chain.canMove(instanceId, delta(direction));
    }
    /** Report a refused move through the existing SugarCubes feedback owner. */
    swap(metadata, direction) {
        const instanceId = readInstanceId(metadata);
        if (!instanceId || !this.canSwap(metadata, direction))
            return;
        try {
            this.#options.chain.move(instanceId, delta(direction));
        }
        catch (error) {
            this.#options.reportError(error);
        }
    }
}
/** Narrow native chrome identity without inventing an instance from an obsolete group marker. */
function readInstanceId(metadata) {
    return typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() || null : null;
}
/** Translate native horizontal arrows into canonical adjacent movement. */
function delta(direction) {
    return direction === 'left' ? -1 : 1;
}
