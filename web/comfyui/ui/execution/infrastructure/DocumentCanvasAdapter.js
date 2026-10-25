/** Adapt document operations without owning execution subscriptions, identity, queues or recovery. */
export function documentCanvasAdapter(id, host, images, report) {
    return {
        id,
        capabilities: {
            get sources() {
                return host.sources ?? undefined;
            },
            get masks() {
                return host.masks ?? undefined;
            },
            output: {
                ordering: 'serial',
                bind(_submission, cancellation) {
                    if (!host.readCanvas()?.capabilities.insertImageLayer)
                        return null;
                    return host
                        .readContext()
                        .then((context) => {
                        if (!context.canvas.capabilities.insertImageLayer)
                            throw new Error('This canvas does not support image layers.');
                        const document = context.document;
                        if (!document)
                            throw new Error(`Open a document in ${context.canvas.label} before generating an image.`);
                        if (cancellation.cancelled())
                            return null;
                        return {
                            async receive(output, stop) {
                                const returned = output.canvasReturns?.find((value) => value.executionPath === output.source.executionPath);
                                if (returned?.kind === 'unavailable')
                                    throw new Error(returned.message);
                                if (returned?.kind === 'region' &&
                                    (returned.canvasId !== context.canvas.id ||
                                        !context.canvas.capabilities.positionImageLayer))
                                    throw new Error('This canvas cannot receive the captured positioned inpaint.');
                                for (const { media, batchIndex } of output.images) {
                                    if (stop.cancelled())
                                        return;
                                    try {
                                        const region = returned?.kind === 'region'
                                            ? returned.regions.length === 1
                                                ? returned.regions[0]
                                                : returned.regions[batchIndex]
                                            : null;
                                        if (returned?.kind === 'region' && !region)
                                            throw new Error('The inpaint batch has no matching captured mask.');
                                        const prepared = await images.load(media.file, stop, region ?? null);
                                        if (!prepared)
                                            continue;
                                        if (stop.cancelled())
                                            return;
                                        const result = await host.insertImage({
                                            document: returned?.kind === 'region' ? returned.document : document,
                                            image: prepared.image,
                                            position: prepared.position,
                                            layerName: media.file.filename.slice(0, 128) || 'Generated image',
                                        }, stop);
                                        if (result.status !== 'inserted' && result.status !== 'cancelled')
                                            throw new Error(result.message);
                                    }
                                    catch (error) {
                                        if (!stop.cancelled())
                                            report(error);
                                    }
                                }
                            },
                        };
                    })
                        .catch((error) => {
                        if (!cancellation.cancelled())
                            report(error);
                        return null;
                    });
                },
            },
        },
    };
}
