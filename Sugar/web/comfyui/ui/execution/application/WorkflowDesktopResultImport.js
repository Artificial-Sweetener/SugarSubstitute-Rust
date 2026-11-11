import { desktopOutputResultId, outputSourceId, promptId } from '../domain/ExecutionIdentity.js';
import { DesktopSessionRefusal, } from '../../workflow/domain/DesktopSession.js';
/** Resolve exact artifacts into a fresh generation without constructing runs or publishing generation events. */
export async function restoreDesktopResults(artifacts, entry, binding, context, checkpointId, cancellation) {
    const sources = new Map(entry.sources.map((source) => [source.id, source]));
    const groups = new Map();
    for (const output of entry.outputs) {
        assertCurrent(cancellation);
        const source = sources.get(output.sourceId);
        if (!source)
            throw new DesktopSessionRefusal('identity-mismatch', 'Recorded final source is unavailable.');
        const resolved = await artifacts.resolve(output, binding, cancellation);
        assertCurrent(cancellation);
        const provenance = Object.freeze({
            kind: 'desktop-session',
            sessionId: context.sessionId,
            generation: context.generation,
            entryId: entry.id,
            snapshotId: checkpointId,
            runId: output.runId,
            promptId: output.promptId === null ? null : promptId(output.promptId),
            sourceId: outputSourceId(source.nodeId),
            executionPath: source.executionPath,
            runOrder: output.runOrder,
            sourceOrder: entry.sources.indexOf(source),
            revision: 1,
        });
        const item = Object.freeze({
            id: desktopOutputResultId(output.id),
            provenance,
            batchIndex: output.batchIndex,
            file: Object.freeze({
                filename: resolved.available ? resolved.receipt.file.filename : output.file.filename,
                subfolder: resolved.available ? resolved.receipt.file.subfolder : output.file.subfolder,
                type: resolved.available ? resolved.receipt.file.storage : output.file.storage,
            }),
            dimensions: Object.freeze({ width: output.width, height: output.height }),
            mediaKind: resolved.available
                ? output.media === 'video'
                    ? 'video'
                    : resolved.receipt.animated
                        ? 'animated'
                        : 'image'
                : 'unsupported',
            encodedMime: output.mime,
            artifact: Object.freeze({ output, binding }),
            ...(resolved.available && resolved.url ? { presentationUrl: resolved.url } : {}),
            ...(!resolved.available
                ? { unavailable: Object.freeze({ code: resolved.code, message: resolved.message }) }
                : {}),
        });
        const key = JSON.stringify([
            output.scene?.groupId,
            output.scene?.sceneId,
            output.sourceId,
            output.runId,
        ]);
        const group = groups.get(key) ?? { source, scene: output.scene, items: [] };
        group.items.push(item);
        groups.set(key, group);
    }
    const latest = new Map();
    const sceneGroups = new Map();
    for (const group of groups.values()) {
        const items = Object.freeze(group.items.sort((a, b) => a.batchIndex - b.batchIndex));
        const first = items[0];
        if (!first)
            continue;
        const output = Object.freeze({
            source: Object.freeze({
                id: outputSourceId(group.source.nodeId),
                executionPath: group.source.executionPath,
                label: group.source.label,
                breadcrumb: Object.freeze([]),
                order: first.provenance.sourceOrder,
            }),
            provenance: first.provenance,
            items,
            text: Object.freeze([]),
            batchCount: Math.max(...items.map((item) => item.batchIndex)) + 1,
        });
        keepLatest(latest, output);
        if (group.scene) {
            const scenes = sceneGroups.get(group.scene.groupId) ?? { order: 0, scenes: new Map() };
            scenes.order = Math.max(scenes.order, output.provenance.runOrder);
            const scene = scenes.scenes.get(group.scene.sceneId) ?? {
                scene: group.scene,
                outputs: new Map(),
            };
            keepLatest(scene.outputs, output);
            scenes.scenes.set(group.scene.sceneId, scene);
            sceneGroups.set(group.scene.groupId, scenes);
        }
    }
    // Accepted scenes without finals remain authored navigation members, with no live run to replay.
    for (const submission of entry.submissions)
        if (submission.scene) {
            const scenes = sceneGroups.get(submission.scene.groupId) ?? { order: 0, scenes: new Map() };
            scenes.order = Math.max(scenes.order, submission.runOrder);
            if (!scenes.scenes.has(submission.scene.sceneId))
                scenes.scenes.set(submission.scene.sceneId, {
                    scene: submission.scene,
                    outputs: new Map(),
                });
            sceneGroups.set(submission.scene.groupId, scenes);
        }
    const current = [...sceneGroups].sort((a, b) => b[1].order - a[1].order)[0];
    const scenes = current
        ? Object.freeze({
            id: current[0],
            running: null,
            scenes: Object.freeze([...current[1].scenes]
                .sort((a, b) => a[1].scene.order - b[1].scene.order)
                .map(([key, value]) => Object.freeze({
                key,
                title: value.scene.title,
                order: value.scene.order,
                outputs: Object.freeze([...value.outputs.values()]),
            }))),
        })
        : null;
    return Object.freeze({
        outputs: Object.freeze([...latest.values()]),
        scenes,
        runOrder: Math.max(0, ...entry.outputs.map((output) => output.runOrder), ...entry.submissions.map((submission) => submission.runOrder)),
        presentedRunOrder: Math.max(0, ...entry.outputs.map((output) => output.runOrder)),
    });
}
/** Retain only the newest final for a source, matching the result owner's live policy. */
function keepLatest(outputs, output) {
    const previous = outputs.get(output.source.id);
    if (!previous || previous.provenance.runOrder <= output.provenance.runOrder)
        outputs.set(output.source.id, output);
}
/** Retire external work before any new checkpoint or runtime mutation. */
function assertCurrent(cancellation) {
    if (cancellation.cancelled())
        throw new DesktopSessionRefusal('cancelled', 'Desktop artifact operation was cancelled.');
}
