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
/** Hold the Add cubes draft and apply it to the workflow through the Cube chain, once. */
import { CubeStackDraft } from '../domain/CubeStackDraft.js';
/** Own the draft's lifetime and its one application to the live workflow. */
export class CubeStackCartSession {
    #options;
    #artifacts;
    #draft;
    #listeners = new Set();
    #retired = false;
    /** Start the draft from the stack as it is now. */
    constructor(options) {
        this.#options = options;
        this.#artifacts = new Map(options.artifacts.map((artifact) => [artifact.catalogKey, artifact]));
        if (this.#artifacts.size !== options.artifacts.length)
            throw new Error('Cube cart artifacts contain duplicate identities.');
        this.#draft = new CubeStackDraft(options.order, options.createId);
    }
    /** Expose immutable references for keyed presentation without revealing mutable draft state. */
    get entries() {
        return this.#draft.entries;
    }
    /** Enable Apply and Reset only for a live changed draft. */
    get changed() {
        return !this.#retired && this.#draft.changed;
    }
    /** Observe only session transitions without starting graph or catalog discovery. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Accept a complete new catalog generation without replacing staged artifact definitions. */
    registerArtifacts(artifacts) {
        this.#requireEditable();
        const keys = artifacts.map((artifact) => artifact.catalogKey);
        if (new Set(keys).size !== keys.length || keys.some((key) => this.#artifacts.has(key)))
            throw new Error('Cube cart refresh contains duplicate captured identities.');
        for (const artifact of artifacts)
            this.#artifacts.set(artifact.catalogKey, artifact);
    }
    /** Stage a captured artifact, permitting independent repeated instances. */
    add(catalogKey, beforeDraftId = null) {
        this.#requireEditable();
        if (!this.#artifacts.has(catalogKey))
            throw new Error('Cube cart artifact is unavailable.');
        const id = this.#draft.add(catalogKey, beforeDraftId);
        this.#notify();
        return id;
    }
    /** Remove a draft reference while keeping the live workflow untouched. */
    remove(draftId) {
        this.#edit(() => this.#draft.remove(draftId));
    }
    /** Accept a complete drag or keyboard draft permutation. */
    reorder(draftIds) {
        this.#edit(() => this.#draft.reorder(draftIds));
    }
    /** Restore the original full stack and remove every staged addition. */
    reset() {
        this.#edit(() => this.#draft.reset());
    }
    /** Make the workflow's stack the draft. Returns false when nothing changed. */
    apply() {
        this.#requireEditable();
        this.#requireCurrent();
        if (!this.#draft.changed)
            return false;
        this.#options.chain.apply(this.#requests());
        this.#retired = true;
        this.#notify();
        return true;
    }
    /** Retire a closed modal so a late click cannot change its former workflow. */
    dispose() {
        this.#retired = true;
        this.#listeners.clear();
    }
    /** Turn draft rows into the stack the chain builds. */
    #requests() {
        return this.#draft.entries.map((entry) => {
            if (entry.source === 'existing')
                return { source: 'existing', instanceId: entry.instanceId };
            const artifact = this.#artifacts.get(entry.catalogKey);
            if (!artifact)
                throw new Error('Cube cart captured artifact is unavailable.');
            return {
                source: 'new',
                instanceId: entry.draftId.slice(4),
                alias: artifact.alias,
                payload: this.#options.prepare(entry.catalogKey),
                size: artifact.size,
            };
        });
    }
    /** Refuse a draft whose workflow gained or lost Cubes while the dialog was open. */
    #requireCurrent() {
        const opening = [...this.#options.order].sort();
        const now = [...this.#options.chain.order()].sort();
        if (opening.length !== now.length || opening.some((id, index) => id !== now[index]))
            throw new Error('Workflow changed while the Cube cart was open. Reopen the cart to continue.');
    }
    /** Enforce one live draft editor. */
    #requireEditable() {
        if (this.#retired)
            throw new Error('Cube cart is closed.');
    }
    /** Publish accepted draft edits once. */
    #edit(edit) {
        this.#requireEditable();
        edit();
        this.#notify();
    }
    /** Publish accepted state through narrow session subscriptions. */
    #notify() {
        for (const listener of this.#listeners)
            listener();
    }
}
