/** Translate face interaction without reproducing graph permissions or geometry. */
export class CubeNodeSwapCoordinator {
    #options;
    /** Share exactly the same application owner used by stack dragging and keyboard movement. */
    constructor(options) {
        this.#options = options;
    }
    /** Reflect native order availability for this stable instance. */
    canSwap(metadata, direction) {
        const instanceId = readInstanceId(metadata);
        return instanceId !== null && this.#options.reorder.canMove(instanceId, delta(direction));
    }
    /** Report rejected or stale actions through the existing SugarCubes feedback owner. */
    async swap(metadata, direction) {
        const instanceId = readInstanceId(metadata);
        if (!instanceId || !this.canSwap(metadata, direction))
            return;
        try {
            await this.#options.reorder.move(instanceId, delta(direction));
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
