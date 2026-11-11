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
/** Sequence temporary cart edits and one validated canonical application without host dependencies. */
import { cubeGraphOrder } from '../../workflow/domain/CubeGraphOrder.js';
import { CubeStackDraft } from '../domain/CubeStackDraft.js';
/** Own the exact opening workflow, draft lifetime, pending transition and one-shot Apply semantics. */
export class CubeStackCartSession {
    #options;
    #artifacts;
    #draft;
    #listeners = new Set();
    #pending = false;
    #retired = false;
    /** Capture canonical opening order and already isolated placement-ready artifacts. */
    constructor(options) {
        this.#options = options;
        this.#artifacts = new Map(options.artifacts.map((artifact) => [artifact.catalogKey, artifact]));
        if (this.#artifacts.size !== options.artifacts.length)
            throw new Error('Cube cart artifacts contain duplicate identities.');
        this.#draft = new CubeStackDraft(cubeGraphOrder(options.analysis), options.createId);
        this.#requireCurrent();
    }
    /** Expose immutable references for keyed presentation without revealing mutable draft state. */
    get entries() {
        return this.#draft.entries;
    }
    /** Enable Apply and Reset only for a live changed draft. */
    get changed() {
        return !this.#retired && this.#draft.changed;
    }
    /** Prevent edits and duplicate application while a canonical plan is pending. */
    get pending() {
        return this.#pending;
    }
    /** Resolve existing row metadata from the same immutable opening analysis. */
    get openingAnalysis() {
        return this.#options.analysis;
    }
    /** Observe only session transitions without starting graph or catalog discovery. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Accept a complete new catalog generation without replacing staged artifact definitions. */
    registerArtifacts(artifacts) {
        this.#requireEditable();
        this.#requireCurrent();
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
    /** Accept a complete drag or keyboard draft permutation for canonical validation on Apply. */
    reorder(draftIds) {
        this.#edit(() => this.#draft.reorder(draftIds));
    }
    /** Restore the original full stack and remove every staged addition. */
    reset() {
        this.#edit(() => this.#draft.reset());
    }
    /** Apply exactly once, rejecting stale workflows and malformed canonical responses before native mutation. */
    async apply() {
        this.#requireEditable();
        this.#requireCurrent();
        if (!this.#draft.changed)
            return false;
        const entries = this.#plan();
        this.#pending = true;
        this.#notify();
        try {
            const result = await this.#options.gateway.applyStack(this.#options.snapshot, entries);
            this.#requireCurrent();
            this.#validateResult(entries, result);
            this.#options.commit.commit(this.#options.snapshot, entries, result);
            this.#retired = true;
            return true;
        }
        finally {
            this.#pending = false;
            this.#notify();
        }
    }
    /** Retire a closed modal so delayed responses cannot modify its former graph. */
    dispose() {
        this.#retired = true;
        this.#listeners.clear();
    }
    /** Translate draft references into complete captured-artifact intent without reloading catalog state. */
    #plan() {
        return this.#draft.entries.map((entry) => {
            if (entry.source === 'existing')
                return { source: 'existing', instanceId: entry.instanceId };
            const artifact = this.#artifacts.get(entry.catalogKey);
            if (!artifact)
                throw new Error('Cube cart captured artifact is unavailable.');
            return { source: 'new', instanceId: entry.draftId.slice(4), ...artifact };
        });
    }
    /** Require exact requested order, retained native identities and captured new artifact versions. */
    #validateResult(entries, result) {
        const requested = new Map(entries.map((entry) => [entry.instanceId, entry]));
        const originals = new Map(this.#options.analysis.instances.map((instance) => [instance.instanceId, instance]));
        const order = cubeGraphOrder(result);
        if (result.instances.length !== entries.length ||
            new Set(result.instances.map((instance) => instance.instanceId)).size !== entries.length ||
            order.length !== entries.length ||
            order.some((id, index) => id !== entries[index]?.instanceId))
            throw new Error('Cube cart response did not fulfill its complete requested order.');
        for (const instance of result.instances) {
            const entry = requested.get(instance.instanceId);
            if (!entry)
                throw new Error('Cube cart response contains an unrequested instance.');
            if (entry.source === 'existing') {
                const original = originals.get(instance.instanceId);
                if (!original ||
                    original.nodeId !== instance.nodeId ||
                    original.definitionId !== instance.definitionId ||
                    original.cubeId !== instance.cubeId ||
                    original.version !== instance.version ||
                    original.alias !== instance.alias ||
                    original.executionMode !== instance.executionMode)
                    throw new Error('Cube cart response changed a retained workflow identity.');
            }
            else {
                const artifact = this.#artifacts.get(entry.catalogKey);
                if (!artifact ||
                    instance.cubeId !== artifact.cubeId ||
                    instance.version !== artifact.version ||
                    instance.executionMode !== 0 ||
                    !instance.alias.trim())
                    throw new Error('Cube cart response changed a captured artifact identity.');
            }
        }
        const nativeIds = new Map(result.instances.map((instance) => [instance.instanceId, instance.nodeId]));
        if (result.placements.length !== entries.length ||
            new Set(result.placements.map((placement) => placement.instanceId)).size !== entries.length ||
            result.placements.some((placement) => nativeIds.get(placement.instanceId) !== placement.nodeId ||
                !placement.position.every(Number.isFinite)))
            throw new Error('Cube cart response contains incomplete native placements.');
    }
    /** Protect every asynchronous application boundary with the same exact native snapshot owner. */
    #requireCurrent() {
        if (this.#retired ||
            !this.#options.snapshots.available ||
            this.#options.snapshots.snapshot().revision !== this.#options.snapshot.revision)
            throw new Error('Workflow changed while the Cube cart was open. Reopen the cart to continue.');
    }
    /** Enforce one live synchronous draft editor and one asynchronous application transition. */
    #requireEditable() {
        if (this.#retired)
            throw new Error('Cube cart is closed.');
        if (this.#pending)
            throw new Error('Cube cart application is already in progress.');
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
