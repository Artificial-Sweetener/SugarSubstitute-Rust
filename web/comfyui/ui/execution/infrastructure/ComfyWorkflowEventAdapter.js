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
/** Adapt Comfy's public events once, preserving preview byte ownership and disposing exact listeners. */
import { isRecord } from '../../types/common.js';
import { comfyEventDetail, decodeComfyPromptId, decodeComfyExecutionPaths, decodeComfyFinalEvent, } from './ComfyExecutionEventDecoder.js';
/** Register one execution/preview/recovery boundary; shared host preview URLs are never copied or revoked. */
export class ComfyWorkflowEventAdapter {
    options;
    #api;
    #listeners = new Map();
    #enhanced = new WeakSet();
    #installed = false;
    /** Validate host capabilities without depending on private frontend stores or WebSocket internals. */
    constructor(options) {
        this.options = options;
        if (!isRecord(options.api) ||
            typeof options.api.addEventListener !== 'function' ||
            typeof options.api.removeEventListener !== 'function')
            throw new TypeError('Comfy execution event API is unavailable.');
        this.#api = options.api;
    }
    /** Register all listeners transactionally, containing unexpected adaptation failures at this boundary. */
    install() {
        if (this.#installed)
            return;
        const add = this.#api.addEventListener;
        if (typeof add !== 'function')
            throw new TypeError('Comfy execution event subscription is unavailable.');
        const handlers = {
            executed: (event) => {
                const decoded = decodeComfyFinalEvent(event);
                if (decoded)
                    this.options.ingestor.final(decoded);
            },
            execution_start: (event) => this.#job(event, (id) => this.options.ingestor.started(id)),
            execution_success: (event) => this.#job(event, (id) => this.options.ingestor.terminal(id, 'complete')),
            execution_interrupted: (event) => this.#job(event, (id) => this.options.ingestor.terminal(id, 'cancelled')),
            execution_error: (event) => this.#job(event, (id) => this.options.ingestor.terminal(id, 'error')),
            execution_cached: (event) => this.#job(event, (id) => this.options.ingestor.cached(id)),
            progress: (event) => this.#progress(event),
            progress_state: (event) => this.#progressState(event),
            executing: (event) => {
                const detail = isRecord(event) ? event.detail : null;
                this.options.ingestor.executingLegacy(decodeComfyExecutionPaths([detail]));
            },
            b_preview_with_metadata: (event) => this.#enhancedPreview(event),
            b_preview: (event) => this.#legacyPreview(event),
            disconnected: () => {
                this.options.ingestor.disconnected();
                this.options.connection(false);
            },
            reconnected: () => {
                this.options.connection(true);
                this.options.reconnect();
            },
        };
        try {
            for (const [name, handle] of Object.entries(handlers)) {
                const listener = (event) => {
                    if (!this.#installed)
                        return;
                    try {
                        handle(event);
                    }
                    catch (error) {
                        this.options.logger.error('SugarCubes workflow event adaptation failed.', {
                            event: name,
                            error,
                        });
                    }
                };
                this.#listeners.set(name, listener);
                add.call(this.#api, name, listener);
            }
            this.#installed = true;
        }
        catch (error) {
            this.dispose();
            throw error;
        }
    }
    /** Remove precisely this adapter's listeners and transient duplicate suppression state. */
    dispose() {
        this.#installed = false;
        const remove = this.#api.removeEventListener;
        for (const [name, listener] of this.#listeners) {
            try {
                if (typeof remove !== 'function')
                    throw new TypeError('Comfy execution event unsubscription is unavailable.');
                remove.call(this.#api, name, listener);
            }
            catch (error) {
                this.options.logger.error('SugarCubes workflow event unsubscription failed.', {
                    event: name,
                    error,
                });
            }
        }
        this.#listeners.clear();
        this.#enhanced = new WeakSet();
    }
    /** Dispatch a validated job-qualified status without forwarding dynamic external values into application code. */
    #job(event, receive) {
        const detail = comfyEventDetail(event);
        const id = decodeComfyPromptId(detail?.prompt_id);
        if (id)
            receive(id);
    }
    /** Prefer qualified progress because Comfy's public executing notification loses its original job ID. */
    #progress(event) {
        const detail = comfyEventDetail(event);
        const id = decodeComfyPromptId(detail?.prompt_id);
        if (id && detail)
            this.options.ingestor.executing(id, decodeComfyExecutionPaths([detail.node]));
    }
    /** Translate running-node context from Comfy's enhanced progress-state map. */
    #progressState(event) {
        const detail = comfyEventDetail(event);
        const id = decodeComfyPromptId(detail?.prompt_id);
        if (!id || !detail || !isRecord(detail.nodes) || Array.isArray(detail.nodes))
            return;
        for (const node of Object.values(detail.nodes))
            if (isRecord(node) && node.state === 'running')
                this.options.ingestor.executing(id, decodeComfyExecutionPaths([
                    node.node_id,
                    node.real_node_id,
                    node.display_node_id,
                    node.parent_node_id,
                ]));
    }
    /** Suppress the identical legacy delivery even when the enhanced event cannot safely be attributed. */
    #enhancedPreview(event) {
        const detail = comfyEventDetail(event);
        if (!detail || !(detail.blob instanceof Blob))
            return;
        this.#enhanced.add(detail.blob);
        const id = decodeComfyPromptId(detail.jobId);
        if (!id)
            return;
        const source = this.options.ingestor.resolveSource(id, decodeComfyExecutionPaths([
            detail.nodeId,
            detail.realNodeId,
            detail.displayNodeId,
            detail.parentNodeId,
        ]));
        if (source && !['complete', 'cancelled', 'error'].includes(source.run.status))
            this.options.preview(source, detail.blob);
    }
    /** Accept legacy bytes only when one known executing job and source supply their missing context. */
    #legacyPreview(event) {
        const blob = isRecord(event) ? event.detail : null;
        if (!(blob instanceof Blob) || this.#enhanced.has(blob))
            return;
        const source = this.options.ingestor.legacyPreviewSource();
        if (source)
            this.options.preview(source, blob);
    }
}
