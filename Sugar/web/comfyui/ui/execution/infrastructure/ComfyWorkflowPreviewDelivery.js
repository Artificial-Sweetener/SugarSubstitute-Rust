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
/** Deliver ephemeral preview bytes to interested renderers without retaining a background pixel cache. */
/** Own preview delivery identities while each interested renderer owns its admitted pixels and URLs. */
export class ComfyWorkflowPreviewDelivery {
    results;
    logger;
    canvasPreview;
    #consumers = new Set();
    #sequence = 0;
    #disposed = false;
    /** Bind metadata ownership and diagnostics without borrowing Comfy's shared object URLs. */
    constructor(results, logger, canvasPreview = () => { }) {
        this.results = results;
        this.logger = logger;
        this.canvasPreview = canvasPreview;
    }
    /** Subscribe one explicit renderer interest; inactive surfaces refuse bytes before admission. */
    subscribe(consumer) {
        if (this.#disposed)
            throw new Error('Workflow preview delivery is disposed.');
        this.#consumers.add(consumer);
        return () => {
            this.#consumers.delete(consumer);
        };
    }
    /** Forward a current frame synchronously without holding its Blob after delivery. */
    deliver(source, blob) {
        if (this.#disposed)
            return;
        const sequence = ++this.#sequence;
        const preview = Object.freeze({
            sessionId: source.run.session.sessionId,
            generation: source.run.session.generation,
            promptId: source.run.promptId,
            sourceId: source.source.id,
            runOrder: source.run.order,
            sequence,
            token: `preview:${sequence}`,
            dimensions: null,
        });
        if (!this.results.publishPreview(preview))
            return;
        this.canvasPreview({ source, preview, bytes: blob });
        for (const consumer of this.#consumers) {
            try {
                if (consumer.wants(source))
                    consumer.receive(preview, blob);
            }
            catch (error) {
                this.logger.error('SugarCubes workflow preview delivery failed.', {
                    promptId: source.run.promptId,
                    sourceId: source.source.id,
                    error,
                });
            }
        }
    }
    /** Retire consumers without taking ownership of renderer-held allocations. */
    dispose() {
        this.#disposed = true;
        this.#consumers.clear();
    }
}
