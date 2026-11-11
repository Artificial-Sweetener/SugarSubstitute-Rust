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
import type {
  SessionBinding,
  SessionSource,
  SessionSubmission,
  SessionOutput,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';

import type { WorkflowOutputSource } from '../domain/WorkflowOutput.js';

interface EntryIdentities {
  readonly sources: Map<string, SessionSource>;
  readonly runs: Map<string, string>;
  readonly outputs: Map<string, string>;
  readonly scenes: Map<string, string>;
}

/** Allocate identities from immutable source/run facts rather than node labels or output filenames. */
export class WorkflowOutputIdentities {
  readonly #entries = new Map<string, EntryIdentities>();
  readonly #issued = new Set<string>();

  /** Supply UUID generation at the infrastructure boundary. */
  constructor(private readonly nextId: () => string) {}

  /** Freeze the submitted graph's source identity even if a reused node later changes metadata. */
  sources(
    binding: SessionBinding,
    values: readonly WorkflowOutputSource[],
  ): readonly SessionSource[] {
    const entry = this.#entry(binding.entryId);
    return Object.freeze(
      values.map((source) => {
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
      }),
    );
  }

  /** Enumerate captured source revisions, including an older final explicitly retained for inspection. */
  inventory(entryId: string): readonly SessionSource[] {
    return Object.freeze([...this.#entry(entryId).sources.values()]);
  }

  /** Keep the accepted prompt's run identity stable after live routing retires. */
  run(entryId: string, prompt: string): string {
    return this.#mapped(this.#entry(entryId).runs, prompt);
  }

  /** Keep an exact immutable result identity stable without depending on its mutable file descriptor. */
  output(entryId: string, runtimeResult: string): string {
    return this.#mapped(this.#entry(entryId).outputs, runtimeResult);
  }

  /** Distinguish scene-group identity from authored normalized scene keys. */
  scene(entryId: string, group: string, key: string | null): string {
    return this.#mapped(this.#entry(entryId).scenes, JSON.stringify([group, key]));
  }

  /** Seed exact source IDs from a validated checkpoint before binding fresh runtime output owners. */
  restoreSources(entryId: string, values: readonly SessionSource[]): void {
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
  restoreResults(
    entryId: string,
    submissions: readonly SessionSubmission[],
    outputs: readonly SessionOutput[],
  ): void {
    const entry = this.#entry(entryId);
    for (const submission of submissions) {
      entry.runs.set(submission.promptId, submission.runId);
      this.#issued.add(submission.runId);
      if (submission.scene) {
        entry.scenes.set(
          JSON.stringify([submission.scene.groupId, null]),
          submission.scene.groupId,
        );
        entry.scenes.set(
          JSON.stringify([submission.scene.groupId, submission.scene.sceneId]),
          submission.scene.sceneId,
        );
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
  retain(entryIds: readonly string[]): void {
    const open = new Set(entryIds);
    for (const id of this.#entries.keys()) if (!open.has(id)) this.#entries.delete(id);
  }

  /** Bound metadata to current graph sources, visible finals and accepted associations retained by their owners. */
  retainMembership(
    entryId: string,
    sources: ReadonlySet<string>,
    runs: ReadonlySet<string>,
    outputs: ReadonlySet<string>,
    scenes: ReadonlySet<string>,
  ): void {
    const entry = this.#entry(entryId);
    for (const [key, value] of entry.sources) if (!sources.has(value.id)) entry.sources.delete(key);
    for (const [key, value] of entry.runs) if (!runs.has(value)) entry.runs.delete(key);
    for (const [key, value] of entry.outputs) if (!outputs.has(value)) entry.outputs.delete(key);
    for (const [key, value] of entry.scenes) if (!scenes.has(value)) entry.scenes.delete(key);
    this.#issued.clear();
    for (const value of this.#entries.values()) {
      for (const source of value.sources.values()) this.#issued.add(source.id);
      for (const mapping of [value.runs, value.outputs, value.scenes])
        for (const id of mapping.values()) this.#issued.add(id);
    }
  }

  /** Release all metadata at page teardown. */
  dispose(): void {
    this.#entries.clear();
    this.#issued.clear();
  }

  /** Create a namespace only from a caller's established durable occurrence. */
  #entry(id: string): EntryIdentities {
    let entry = this.#entries.get(id);
    if (!entry) {
      if (!sessionUuid(id)) throw new TypeError('Desktop output occurrence identity is invalid.');
      entry = { sources: new Map(), runs: new Map(), outputs: new Map(), scenes: new Map() };
      this.#entries.set(id, entry);
    }
    return entry;
  }

  /** Allocate once for an immutable execution key. */
  #mapped(values: Map<string, string>, key: string): string {
    let id = values.get(key);
    if (!id) {
      id = this.#identity();
      values.set(key, id);
    }
    return id;
  }

  /** Refuse invalid or reused UUID generation instead of silently conflating outputs. */
  #identity(): string {
    const id = this.nextId();
    if (!sessionUuid(id) || this.#issued.has(id))
      throw new TypeError('Desktop execution identity was invalid or reused.');
    this.#issued.add(id);
    return id;
  }
}
