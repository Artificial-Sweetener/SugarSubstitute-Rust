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
/** Restore workflow views at the committed session boundary without observing presentation DOM. */
import { WorkflowViewPreferences } from '../application/WorkflowViewPreferences.js';
/** Identify browser-local view metadata, separate from user workflow documents. */
export const WORKFLOW_VIEW_STORAGE_KEY = 'SugarCubes.WorkflowViews.v1';
/** Adapt storage and authoritative session commitment to one shared presentation owner. */
export class ComfyWorkflowViewPreferences {
    sessions;
    defaultCanvas;
    #views;
    #listeners = new Set();
    #stop = null;
    #ready = false;
    /** Inject browser persistence so invalid/unavailable storage never blocks workflow switching. */
    constructor(sessions, storage, defaultCanvas, logger) {
        this.sessions = sessions;
        this.defaultCanvas = defaultCanvas;
        let stored = null;
        try {
            const json = storage()?.getItem(WORKFLOW_VIEW_STORAGE_KEY);
            if (json && json.length <= 2_000_000)
                stored = JSON.parse(json);
        }
        catch (error) {
            logger.warn('SugarCubes could not restore workflow view preferences.', { error });
        }
        this.#views = new WorkflowViewPreferences(stored, (value) => {
            try {
                storage()?.setItem(WORKFLOW_VIEW_STORAGE_KEY, JSON.stringify(value));
            }
            catch (error) {
                logger.warn('SugarCubes could not persist workflow view preferences.', { error });
            }
        });
    }
    /** Read accepted per-workflow choices with no graph or storage access. */
    get snapshot() {
        return this.#views.snapshot;
    }
    /** Expose graph commitment separately from the retained view choice during loading. */
    get ready() {
        return this.#ready;
    }
    /** Bind once after host setup and ignore intermediate asynchronous graph-load notifications. */
    setup() {
        if (this.#stop)
            return;
        this.#stop = this.sessions.subscribe(this.#synchronize);
        this.#synchronize();
    }
    /** Observe choices without requesting inventory or surface reconstruction. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Persist explicit actions only while the graph has a committed active workflow. */
    change(value) {
        if (!this.sessions.read().active || !this.#views.change(value))
            return;
        this.#publish();
    }
    /** Retire the host subscription without changing persisted choices. */
    dispose() {
        this.#stop?.();
        this.#stop = null;
        this.#listeners.clear();
    }
    /** Restore both view choices together only after Comfy commits the target workflow. */
    #synchronize = () => {
        const snapshot = this.sessions.read();
        this.#views.retain(snapshot.open.map((context) => context.sessionId));
        const ready = !snapshot.loading && snapshot.active !== null;
        const changed = ready !== this.#ready;
        this.#ready = ready;
        if (!ready || !snapshot.active) {
            if (changed)
                this.#publish();
            return;
        }
        const context = snapshot.active;
        if (this.#views.activate({ id: context.sessionId, path: context.path }, this.defaultCanvas) ||
            changed)
            this.#publish();
    };
    /** Deliver one accepted selection/change signal to the canvas and masonry owners. */
    #publish() {
        for (const listener of this.#listeners)
            listener();
    }
}
