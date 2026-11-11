//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Capture an admitted portable checkpoint from the current Comfy document and result owners. */
import { desktopCheckpointError, utf8Bytes } from '../../core/embedded/SessionRuntime.js';
import { readComfyWorkflowInputState } from '../../input_canvas/infrastructure/ComfyWorkflowInputState.js';
import { DesktopSessionRefusal, } from '../domain/DesktopSession.js';
/** Keep checkpoint inventory and bounded DTO admission together; this owner never changes workflow state. */
export class ComfyDesktopCapture {
    options;
    constructor(options) {
        this.options = options;
    }
    /** Freeze all semantic owners before awaiting receipts; the client refuses any later observation fence. */
    async capture(id, revision, cancellation) {
        const snapshot = this.options.execution.sessions.read();
        const versions = await this.options.versions();
        const entries = [];
        let total = 0;
        for (const context of snapshot.open) {
            this.options.assertCurrent(cancellation);
            const binding = this.options.bindings.binding(context.sessionId);
            const document = this.options.documents.capture(context, binding.graphRevision);
            total += utf8Bytes(document.graphJson) + utf8Bytes(document.baselineGraphJson ?? '');
            if (total > 128 * 1024 * 1024)
                throw new DesktopSessionRefusal('too-large', 'Desktop workspace exceeds checkpoint admission.');
            const result = this.options.execution.results.read(context.sessionId);
            const preferences = this.options.presentation.capture(context.sessionId);
            if (!result || !preferences)
                throw new DesktopSessionRefusal('stale', 'Desktop workflow owner was retired.');
            const outputs = await this.options.outputs.capture(binding, this.options.scope.backend, result, cancellation);
            this.options.assertCurrent(cancellation);
            // Results arriving while receipts were awaited can advance this workflow past the captured
            // document; the capture then describes no single moment and is retried, not refused.
            if (!sameBinding(this.options.bindings.binding(context.sessionId), binding))
                throw new DesktopSessionRefusal('stale', 'Desktop workflow changed while its checkpoint was captured.');
            const route = readComfyWorkflowInputState(JSON.parse(document.graphJson))?.route ?? 'output';
            const view = this.options.canvas.capture(context, binding, outputs, this.options.identities, preferences, route);
            entries.push(Object.freeze({
                id: binding.entryId,
                document,
                input: this.options.input.get(binding.entryId) ?? null,
                view,
                sourceRevision: binding.sourceRevision,
                navigationRevision: binding.navigationRevision,
                sources: this.options.identities.inventory(binding.entryId),
                outputs,
                submissions: this.options.submissions.read(binding.entryId),
            }));
        }
        const assets = new Map();
        for (const output of entries.flatMap((entry) => entry.outputs)) {
            if (output.assetDigest === null)
                continue;
            const known = this.options.assets.get(output.assetDigest);
            const asset = output.file.storage === 'temp' ? requestedTemporaryAsset(output) : known;
            if (!asset)
                throw new DesktopSessionRefusal('identity-mismatch', 'Retained final asset descriptor is unavailable.');
            const previous = assets.get(asset.digest) ?? known;
            if (asset.digest !== output.digest ||
                asset.bytes !== output.bytes ||
                asset.media !== output.media ||
                (previous &&
                    (previous.bytes !== asset.bytes ||
                        previous.extension !== asset.extension ||
                        previous.media !== asset.media)))
                throw new DesktopSessionRefusal('identity-mismatch', 'Final asset receipts disagree about their encoded identity.');
            assets.set(asset.digest, asset);
        }
        const checkpoint = Object.freeze({
            version: 1,
            id,
            revision,
            previousId: this.options.previousId(),
            capturedAt: this.options.now(),
            scope: this.options.scope,
            producer: 'SugarCubes desktop frontend',
            frontend: versions.frontend,
            sugarcubes: versions.sugarcubes,
            entries: Object.freeze(entries),
            activeEntry: snapshot.active
                ? this.options.bindings.binding(snapshot.active.sessionId).entryId
                : null,
            route: this.options.settingsRoute?.read() ?? 'workflow',
            placement: null,
            assets: Object.freeze([...assets.values()]),
        });
        const rule = desktopCheckpointError(checkpoint);
        if (rule !== null)
            throw new DesktopSessionRefusal('invalid', `Desktop checkpoint failed semantic admission: ${rule}.`);
        return checkpoint;
    }
}
/** Describe exact temporary bytes the native publisher must verify and retain before durable acknowledgement. */
function requestedTemporaryAsset(output) {
    const extensions = {
        'image/png': 'png',
        'image/jpeg': 'jpg',
        'image/webp': 'webp',
        'image/gif': 'gif',
        'video/mp4': 'mp4',
        'video/webm': 'webm',
    };
    const extension = Object.hasOwn(extensions, output.mime) ? extensions[output.mime] : undefined;
    if (!extension)
        throw new DesktopSessionRefusal('unavailable', 'Temporary final format cannot be retained.');
    return Object.freeze({
        version: 1,
        digest: output.digest,
        bytes: output.bytes,
        extension,
        media: output.media,
    });
}
/** Whether a workflow still sits at the exact revisions a capture started from. */
function sameBinding(left, right) {
    return (left.entryId === right.entryId &&
        left.runtimeSession === right.runtimeSession &&
        left.generation === right.generation &&
        left.graphRevision === right.graphRevision &&
        left.sourceRevision === right.sourceRevision &&
        left.navigationRevision === right.navigationRevision);
}
