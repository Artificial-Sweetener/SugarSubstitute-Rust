/** Turn one picked catalog type into a new Cube after the source in the stack. */
export class CubePickerInsertAfterService {
    #options;
    /** Bind the picker's prepared payloads to the live chain. */
    constructor(options) {
        this.#options = options;
    }
    /** Insert one Cube after the source instance and return its node. */
    insert(sourceInstanceId, type) {
        const runtime = this.#options.getRuntime();
        const prepared = this.#options.placement.prepare(type);
        const instanceId = this.#options.createId();
        const size = runtime.construction.previewSize(prepared.payload, prepared.descriptor.inputs.length > 0, { instanceId });
        return runtime.chain.insertAfter(sourceInstanceId, {
            instanceId,
            alias: prepared.descriptor.displayName,
            payload: prepared.payload,
            size: [size[0], size[1]],
        });
    }
}
