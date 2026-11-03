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
/** Capture reusable canvas targets coherently without knowing how consumers interpret their pixels. */
import { CanvasSourceUnavailable, canvasSourceUnavailable, } from '../domain/CanvasSourceAvailability.js';
import { canvasSelectionImage } from '../domain/CanvasSelectionImage.js';
import { decodeCanvasRaster, canvasRasterByteLength, MAX_CANVAS_RASTER_BYTES, } from '../../core/embedded/CanvasRuntime.js';
import { canvasSourceTargetKey, sameCanvasSourceDocument, } from '../../core/embedded/CanvasRuntime.js';
/** Own exact context validation and one native capture per canvas for previews and execution. */
export class CanvasSourceCapture {
    host;
    documents;
    /**
     * Resolve capability by canvas identity rather than editor-specific globals.
     *
     * `documents` names input documents a workflow-surface canvas provisioned. Sources bound to such
     * a document are captured from it even while another document is the canvas's active one; the
     * canvas still rejects a stale or retired document, so capture fails closed.
     */
    constructor(host, documents = () => null) {
        this.host = host;
        this.documents = documents;
    }
    /** Require explicit source support before exposing document or layer choices. */
    async context(canvasId) {
        const host = this.host(canvasId);
        if (!canvasId || !host || host.canvasId !== canvasId)
            throw new CanvasSourceUnavailable('Canvas unavailable');
        const result = await host.execute({ type: 'context' });
        if (result.kind === 'failure') {
            if (result.status === 'unavailable' ||
                result.status === 'stale' ||
                result.status === 'unsupported')
                throw new CanvasSourceUnavailable('Source unavailable');
            throw new Error(result.message);
        }
        if (result.kind !== 'context' || !result.supported)
            throw new CanvasSourceUnavailable('Source unavailable');
        return result;
    }
    /** Freeze each requested source against one document revision before returning any captured pixels. */
    async capture(entries, maximumSize) {
        if (!Number.isSafeInteger(maximumSize) || maximumSize < 0 || maximumSize > 1024)
            throw new Error('Invalid canvas source preview size.');
        const groups = new Map();
        for (const entry of entries) {
            const group = groups.get(entry.source.canvasId) ?? [];
            group.push(entry);
            groups.set(entry.source.canvasId, group);
        }
        const captured = new Map();
        for (const [canvasId, group] of groups) {
            const context = await this.context(canvasId);
            const documents = new Map();
            for (const entry of group) {
                const { source } = entry;
                const provisioned = source.documentId !== null && source.documentId !== context.document?.id
                    ? this.documents(canvasId, source.documentId)
                    : null;
                if (!provisioned) {
                    const reason = canvasSourceUnavailable(source, context);
                    if (reason)
                        throw new CanvasSourceUnavailable(reason, source);
                }
                const document = provisioned ?? context.document;
                if (!document)
                    throw new CanvasSourceUnavailable('Open a document');
                const scope = documents.get(document.id) ?? { document, group: [] };
                scope.group.push(entry);
                documents.set(document.id, scope);
            }
            for (const { document, group } of documents.values())
                await this.#captureDocument(canvasId, context, document, group, maximumSize, captured);
        }
        return captured;
    }
    /** Capture one document's targets at one revision and add each entry's representation. */
    async #captureDocument(canvasId, context, document, group, maximumSize, captured) {
        const targets = [
            ...new Map(group.flatMap(({ source, representation }) => {
                const requested = [source.target];
                if (representation === 'image' && source.target.kind === 'selection') {
                    if (!context.targets.includes('composition'))
                        throw new Error('This canvas cannot supply selected image content.');
                    requested.push({ kind: 'composition' });
                }
                return requested.map((target) => [canvasSourceTargetKey(target), target]);
            })).values(),
        ];
        if (targets.length > 64)
            throw new Error('A canvas capture supports at most 64 targets.');
        const host = this.host(canvasId);
        if (!host)
            throw new CanvasSourceUnavailable('Canvas unavailable');
        const result = await host.execute({ type: 'capture', document, targets, maximumSize });
        if (result.kind === 'failure') {
            if (result.status === 'unavailable' ||
                result.status === 'stale' ||
                result.status === 'unsupported')
                throw new CanvasSourceUnavailable('Source unavailable');
            throw new Error(result.message);
        }
        if (result.kind !== 'captured' || result.snapshots.length !== targets.length)
            throw new Error('The canvas returned an incomplete source capture.');
        if (new Set(result.snapshots.map((snapshot) => snapshot.revision)).size > 1)
            throw new Error('The canvas revision changed during source capture.');
        const scale = maximumSize
            ? Math.min(1, maximumSize / Math.max(document.width, document.height))
            : 1;
        const width = Math.max(1, Math.round(document.width * scale)), height = Math.max(1, Math.round(document.height * scale));
        let transferred = 0;
        for (const snapshot of result.snapshots) {
            transferred += canvasRasterByteLength(snapshot.pixels.width, snapshot.pixels.height, snapshot.pixels.format);
            if (transferred > MAX_CANVAS_RASTER_BYTES)
                throw new Error('The canvas source collection exceeds its transfer budget.');
        }
        const represented = new Map();
        for (const entry of group) {
            const matching = result.snapshots.filter((snapshot) => canvasSourceTargetKey(snapshot.target) === canvasSourceTargetKey(entry.source.target));
            const snapshot = matching[0];
            if (matching.length !== 1 ||
                !snapshot ||
                !sameCanvasSourceDocument(snapshot.document, document) ||
                snapshot.pixels.width !== width ||
                snapshot.pixels.height !== height)
                throw new Error('The canvas source identity or dimensions changed.');
            if (snapshot.pixels.format !== (snapshot.target.kind === 'selection' ? 'coverage8' : 'rgba8'))
                throw new Error('The canvas source returned an invalid pixel representation.');
            decodeCanvasRaster(snapshot.pixels);
            const representation = entry.representation ?? 'raw';
            const representationKey = JSON.stringify([
                canvasSourceTargetKey(entry.source.target),
                representation,
            ]);
            const previous = represented.get(representationKey);
            if (previous) {
                captured.set(entry.key, previous);
                continue;
            }
            let pixels = snapshot.pixels;
            if (representation === 'image') {
                if (entry.source.target.kind === 'selection') {
                    const image = result.snapshots.find((snapshot) => snapshot.target.kind === 'composition');
                    if (!image || !sameCanvasSourceDocument(image.document, document))
                        throw new Error('Missing coherent selection image content.');
                    pixels = canvasSelectionImage(image.pixels, pixels);
                }
                else if (pixels.format !== 'rgba8')
                    throw new Error('The canvas image source did not return image pixels.');
            }
            const value = Object.freeze({
                ...snapshot,
                document: Object.freeze({ ...snapshot.document }),
                pixels: Object.freeze({ ...pixels }),
                representation,
            });
            represented.set(representationKey, value);
            captured.set(entry.key, value);
        }
    }
}
