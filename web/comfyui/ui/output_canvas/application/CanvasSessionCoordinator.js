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
/** Restore one independent browser view per open workflow and retire closed bindings. */
import { canvasSessionId } from '../domain/CanvasIdentity.js';
import { canvasSceneCatalog } from './CanvasOutputCatalog.js';
/** Coordinate lightweight primary views without owning surface resources or native multi-view accounting. */
export class CanvasSessionCoordinator {
    sessions;
    results;
    sources;
    views;
    selection;
    metadata;
    #primary = new Map();
    #sessionsByView = new Map();
    #contexts = new Map();
    #catalogs = new Map();
    #active = null;
    #listeners = new Set();
    #retired = new Set();
    #stops = [];
    /** Inject authoritative execution and view owners with one metadata change boundary. */
    constructor(sessions, results, sources, views, selection, metadata) {
        this.sessions = sessions;
        this.results = results;
        this.sources = sources;
        this.views = views;
        this.selection = selection;
        this.metadata = metadata;
    }
    /** Observe named result/session changes while source discovery remains with its adapter. */
    install() {
        if (this.#stops.length)
            return;
        this.#stops.push(this.sessions.subscribe(() => this.#synchronize()), this.results.subscribe((session) => this.#refresh(session)), this.metadata.subscribe(() => {
            const active = this.sessions.read().active;
            if (active)
                this.#refresh(active.sessionId);
        }));
        this.#synchronize();
    }
    /** Retain the qualified browser view during history restoration and release it for graph replacement. */
    active() {
        const active = this.sessions.presentation();
        return active ? (this.#primary.get(active.sessionId) ?? null) : null;
    }
    /** Project an exact open session into its view without consulting the active graph. */
    catalog(view) {
        const id = this.#sessionsByView.get(view);
        if (!id)
            return null;
        const result = this.results.read(id);
        if (!result)
            return null;
        const sources = this.sources.read(result.context);
        const previous = this.#catalogs.get(view);
        const inspection = this.selection.inspection(view);
        const scene = this.views.read(view).scene;
        if (previous?.result === result &&
            previous.sources === sources &&
            previous.inspection === inspection &&
            previous.scene === scene)
            return previous.catalog;
        const catalog = canvasSceneCatalog(result, sources, inspection, scene);
        this.#catalogs.set(view, { result, sources, inspection, scene, catalog });
        return catalog;
    }
    /** Deliver scoped result changes and explicit active-view transitions. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Allow inspection state owners to release a retired view through one lifecycle signal. */
    onRetired(listener) {
        this.#retired.add(listener);
        return () => this.#retired.delete(listener);
    }
    /** Release only this coordinator's primary views, preserving independently owned native views. */
    dispose() {
        for (const stop of this.#stops.splice(0))
            stop();
        for (const [session, view] of this.#primary)
            this.#remove(session, view);
        this.#active = null;
        this.#listeners.clear();
        this.#retired.clear();
    }
    /** Keep view identity stable through rename and activation, while closure releases all light state. */
    #synchronize() {
        const snapshot = this.sessions.read();
        const open = new Set(snapshot.open.map((session) => session.sessionId));
        for (const [session, view] of this.#primary)
            if (!open.has(session))
                this.#remove(session, view);
        for (const session of snapshot.open) {
            if (!this.#primary.has(session.sessionId)) {
                const view = this.views.create(canvasSessionId(session.sessionId)).viewId;
                this.#primary.set(session.sessionId, view);
                this.#sessionsByView.set(view, session.sessionId);
            }
            if (this.#contexts.get(session.sessionId) !== session) {
                this.#contexts.set(session.sessionId, session);
                this.#refresh(session.sessionId);
            }
        }
        const active = this.active();
        if (this.#active !== active) {
            this.#active = active;
            this.#publish(active);
        }
    }
    /** Reconcile one view using only existing topology metadata and current result descriptors. */
    #refresh(session) {
        const view = this.#primary.get(session);
        if (!view)
            return;
        const catalog = this.catalog(view);
        if (catalog)
            this.selection.reconcile(view, catalog);
        this.#publish(view);
    }
    /** Retire this browser view and its metadata without manufacturing another binding. */
    #remove(session, view) {
        this.selection.remove(view);
        this.views.remove(view);
        this.sources.remove(session);
        this.#primary.delete(session);
        this.#sessionsByView.delete(view);
        this.#contexts.delete(session);
        this.#catalogs.delete(view);
        for (const listener of this.#retired)
            listener(view);
    }
    /** Preserve explicit view qualification across presentation delivery. */
    #publish(view) {
        for (const listener of this.#listeners)
            listener(view);
    }
}
