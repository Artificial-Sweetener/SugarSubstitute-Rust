/** Backend-resolved originals retain existing actions; only a verified retained presentation carries this source. */
export function retainedCanvasOutput(item) {
    if (!item?.presentationUrl)
        return undefined;
    const artifact = item.artifact;
    if (!artifact ||
        artifact.output.id !== item.id ||
        artifact.binding.runtimeSession !== item.provenance.sessionId ||
        artifact.binding.generation !== item.provenance.generation ||
        artifact.output.entryId !== artifact.binding.entryId ||
        artifact.output.assetDigest !== artifact.output.digest)
        throw new TypeError('Retained canvas output does not match its admitted immutable artifact.');
    return { output: artifact.output, binding: artifact.binding, url: item.presentationUrl };
}
