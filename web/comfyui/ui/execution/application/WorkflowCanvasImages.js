/** Admit each finished image once per job and output source, including overlapping jobs. */
export function workflowCanvasImages(run, source, images, seen) {
    const fresh = [];
    for (const media of images) {
        if (!media || media.mediaKind !== 'image' || media.file.type === 'input')
            continue;
        const id = JSON.stringify([
            run.session.sessionId,
            run.session.generation,
            run.promptId,
            source,
            media.file.type,
            media.file.subfolder,
            media.file.filename,
        ]);
        if (seen.has(id))
            continue;
        seen.add(id);
        fresh.push(Object.freeze({ id, media }));
    }
    return Object.freeze(fresh);
}
