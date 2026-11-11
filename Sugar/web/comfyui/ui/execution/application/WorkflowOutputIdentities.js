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
/** Preserve immutable durable execution identities across checkpoint captures and runtime rebinding. */
import { sessionUuid } from '../../core/embedded/SessionRuntime.js';
/** Allocate identities from immutable source/run facts rather than node labels or output filenames. */
export class WorkflowOutputIdentities {
    nextId;
    #entries = new Map();
    #issued = new Set();
    /** Supply UUID generation at the infrastructure boundary. */
    constructor(nextId) {
        this.nextId = nextId;
    }
    /** Freeze the submitted graph's source identity even if a reused node later changes metadata. */
    sources(binding, values) {
        const entry = this.#entry(binding.entryId);
        return Object.freeze(values.map((source) => {
            const key = JSON.stringify([binding.graphRevision, source.id]);
            let captured = entry.sources.get(key);
            if (!captured) {
                captured = Object.freeze({
                    id: this.#identity(),
                    nodeId: source.id.slice('source:'.length),
                    executionPath: source.executionPath,
                    label: source.label,
                    graphRevision: binding.graphRevision,
                });
                entry.sources.set(key, captured);
            }
            return captured;
        }));
    }
    /** Enumerate captured source revisions, including an older final explicitly retained for inspection. */
    inventory(entryId) {
        return Object.freeze([...this.#entry(entryId).sources.values()]);
    }
    /** Keep the accepted prompt's run identity stable after live routing retires. */
    run(entryId, prompt) {
        return this.#mapped(this.#entry(entryId).runs, prompt);
    }
    /** Keep an exact immutable result identity stable without depending on its mutable file descriptor. */
    output(entryId, runtimeResult) {
        return this.#mapped(this.#entry(entryId).outputs, runtimeResult);
    }
    /** Distinguish scene-group identity from authored normalized scene keys. */
    scene(entryId, group, key) {
        return this.#mapped(this.#entry(entryId).scenes, JSON.stringify([group, key]));
    }
    /** Seed exact source IDs from a validated checkpoint before binding fresh runtime output owners. */
    restoreSources(entryId, values) {
        const entry = this.#entry(entryId);
        for (const source of values) {
            if (!sessionUuid(source.id))
                throw new TypeError('Restored output source identity is invalid.');
            const key = JSON.stringify([source.graphRevision, `source:${source.nodeId}`]);
            const previous = entry.sources.get(key);
            if (previous && previous.id !== source.id)
                throw new TypeError('Restored source revision is ambiguous.');
            entry.sources.set(key, Object.freeze({ ...source }));
            this.#issued.add(source.id);
        }
    }
    /** Preserve restored prompt/run, scene and artifact IDs across the next checkpoint capture. */
    restoreResults(entryId, submissions, outputs) {
        const entry = this.#entry(entryId);
        for (const submission of submissions) {
            entry.runs.set(submission.promptId, submission.runId);
            this.#issued.add(submission.runId);
            if (submission.scene) {
                entry.scenes.set(JSON.stringify([submission.scene.groupId, null]), submission.scene.groupId);
                entry.scenes.set(JSON.stringify([submission.scene.groupId, submission.scene.sceneId]), submission.scene.sceneId);
                this.#issued.add(submission.scene.groupId);
                this.#issued.add(submission.scene.sceneId);
            }
        }
        for (const output of outputs) {
            entry.outputs.set(output.id, output.id);
            this.#issued.add(output.id);
        }
    }
    /** Release closed occurrence identity maps without deleting any output file. */
    retain(entryIds) {
        const open = new Set(entryIds);
        for (const id of this.#entries.keys())
            if (!open.has(id))
                this.#entries.delete(id);
    }
    /** Bound metadata to current graph sources, visible finals and accepted associations retained by their owners. */
    retainMembership(entryId, sources, runs, outputs, scenes) {
        const entry = this.#entry(entryId);
        for (const [key, value] of entry.sources)
            if (!sources.has(value.id))
                entry.sources.delete(key);
        for (const [key, value] of entry.runs)
            if (!runs.has(value))
                entry.runs.delete(key);
        for (const [key, value] of entry.outputs)
            if (!outputs.has(value))
                entry.outputs.delete(key);
        for (const [key, value] of entry.scenes)
            if (!scenes.has(value))
                entry.scenes.delete(key);
        this.#issued.clear();
        for (const value of this.#entries.values()) {
            for (const source of value.sources.values())
                this.#issued.add(source.id);
            for (const mapping of [value.runs, value.outputs, value.scenes])
                for (const id of mapping.values())
                    this.#issued.add(id);
        }
    }
    /** Release all metadata at page teardown. */
    dispose() {
        this.#entries.clear();
        this.#issued.clear();
    }
    /** Create a namespace only from a caller's established durable occurrence. */
    #entry(id) {
        let entry = this.#entries.get(id);
        if (!entry) {
            if (!sessionUuid(id))
                throw new TypeError('Desktop output occurrence identity is invalid.');
            entry = { sources: new Map(), runs: new Map(), outputs: new Map(), scenes: new Map() };
            this.#entries.set(id, entry);
        }
        return entry;
    }
    /** Allocate once for an immutable execution key. */
    #mapped(values, key) {
        let id = values.get(key);
        if (!id) {
            id = this.#identity();
            values.set(key, id);
        }
        return id;
    }
    /** Refuse invalid or reused UUID generation instead of silently conflating outputs. */
    #identity() {
        const id = this.nextId();
        if (!sessionUuid(id) || this.#issued.has(id))
            throw new TypeError('Desktop execution identity was invalid or reused.');
        this.#issued.add(id);
        return id;
    }
}
