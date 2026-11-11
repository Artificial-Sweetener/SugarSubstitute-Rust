import { freezeWorkflowMedia } from '../domain/WorkflowMedia.js';
/** Copy imported references and preserve sparse batch positions for the next genuine live result update. */
export function restoredWorkflowOutput(context, value) {
    if (!Number.isSafeInteger(value.batchCount) || value.batchCount < 0 || value.batchCount > 8192)
        throw new TypeError('Restored batch exceeds the admitted desktop manifest.');
    if (value.items.some((item) => item.provenance.sessionId !== context.sessionId ||
        item.provenance.generation !== context.generation ||
        item.provenance.kind !== 'desktop-session'))
        throw new TypeError('Restored output does not belong to this fresh workflow generation.');
    const items = Object.freeze(value.items.map((item) => Object.freeze({
        ...item,
        file: Object.freeze({ ...item.file }),
        dimensions: item.dimensions ? Object.freeze({ ...item.dimensions }) : null,
        provenance: Object.freeze({ ...item.provenance }),
        ...(item.artifact
            ? {
                artifact: Object.freeze({
                    output: Object.freeze({
                        ...item.artifact.output,
                        file: Object.freeze({ ...item.artifact.output.file }),
                        scene: item.artifact.output.scene
                            ? Object.freeze({ ...item.artifact.output.scene })
                            : null,
                    }),
                    binding: Object.freeze({ ...item.artifact.binding }),
                }),
            }
            : {}),
    })));
    const output = Object.freeze({
        ...value,
        source: Object.freeze({
            ...value.source,
            breadcrumb: Object.freeze([...value.source.breadcrumb]),
        }),
        provenance: Object.freeze({ ...value.provenance }),
        items,
        text: Object.freeze([...value.text]),
    });
    const media = Array.from({ length: output.batchCount }, () => null);
    for (const item of items) {
        if (!Number.isSafeInteger(item.batchIndex) ||
            item.batchIndex < 0 ||
            item.batchIndex >= output.batchCount ||
            media[item.batchIndex] !== null)
            throw new TypeError('Restored output batch position is invalid.');
        media[item.batchIndex] = item;
    }
    return {
        output,
        payload: freezeWorkflowMedia({ images: media, videos: [], models: [], text: output.text }),
        delivery: 'desktop-session',
        completedPreviewRunOrder: items.reduce((latest, item) => item.batchIndex === 0 && item.file.type !== 'input'
            ? Math.max(latest, item.provenance.runOrder)
            : latest, -1),
    };
}
