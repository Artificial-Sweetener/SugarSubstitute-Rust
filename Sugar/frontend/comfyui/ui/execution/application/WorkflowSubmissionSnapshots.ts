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
/** Retain accepted immutable prompt association independently of live-job retirement and final media. */
import type {
  SessionBinding,
  SessionSource,
  SessionSubmission,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';

/** Keep checkpoint receipts after transport routing retires without retaining host graphs or pixels. */
export class WorkflowSubmissionSnapshots {
  readonly #entries = new Map<string, Map<string, SessionSubmission>>();
  readonly #listeners = new Set<() => void>();

  /** Commit before final output; prompt association is itself a durability-significant transition. */
  accept(submission: SessionSubmission): void {
    let prompts = this.#entries.get(submission.entryId);
    const previous = prompts?.get(submission.promptId);
    if (previous) {
      if (JSON.stringify(previous) !== JSON.stringify(submission))
        throw new TypeError('An accepted prompt cannot change its captured ownership.');
      return;
    }
    for (const entry of this.#entries.values())
      if (entry.has(submission.promptId))
        throw new TypeError('An accepted prompt cannot belong to multiple desktop occurrences.');
    prompts ??= new Map();
    prompts.set(
      submission.promptId,
      Object.freeze({
        ...submission,
        scene: submission.scene ? Object.freeze({ ...submission.scene }) : null,
        sources: Object.freeze(submission.sources.map((source) => Object.freeze({ ...source }))),
      }),
    );
    this.#entries.set(submission.entryId, prompts);
    this.#publish();
  }

  /** Read accepted pending and completed associations without depending on backend history. */
  read(entryId: string): readonly SessionSubmission[] {
    return Object.freeze(
      [...(this.#entries.get(entryId)?.values() ?? [])].sort((a, b) => a.runOrder - b.runOrder),
    );
  }

  /** Resolve the frozen source captured for one accepted prompt, never the current reused node. */
  source(entryId: string, prompt: string, executionPath: string): SessionSource | null {
    return (
      this.#entries
        .get(entryId)
        ?.get(prompt)
        ?.sources.find((source) => source.executionPath === executionPath) ?? null
    );
  }

  /** Import association metadata without publishing a generation or creating live transport jobs. */
  restore(entryId: string, submissions: readonly SessionSubmission[]): void {
    if (submissions.some((submission) => submission.entryId !== entryId))
      throw new TypeError('Restored prompt association names another desktop occurrence.');
    const next = new Map<string, SessionSubmission>();
    for (const submission of submissions) {
      if (
        next.has(submission.promptId) ||
        [...this.#entries].some(
          ([id, prompts]) => id !== entryId && prompts.has(submission.promptId),
        )
      )
        throw new TypeError('Restored prompt ownership is ambiguous.');
      next.set(
        submission.promptId,
        Object.freeze({
          ...submission,
          scene: submission.scene ? Object.freeze({ ...submission.scene }) : null,
          sources: Object.freeze(submission.sources.map((source) => Object.freeze({ ...source }))),
        }),
      );
    }
    this.#entries.set(entryId, next);
    this.#publish();
  }

  /** Remove closed occurrences while ordinary completed-job retirement leaves receipts intact. */
  synchronize(bindings: readonly SessionBinding[]): void {
    const open = new Set(bindings.map((binding) => binding.entryId));
    let changed = false;
    for (const entry of this.#entries.keys())
      if (!open.has(entry)) {
        this.#entries.delete(entry);
        changed = true;
      }
    if (changed) this.#publish();
  }

  /** Retain exactly the pending/current scene and visible/comparison prompt memberships chosen by results. */
  retain(entryId: string, promptIds: ReadonlySet<string>): void {
    const prompts = this.#entries.get(entryId);
    if (!prompts) return;
    let changed = false;
    for (const prompt of prompts.keys())
      if (!promptIds.has(prompt)) {
        prompts.delete(prompt);
        changed = true;
      }
    if (changed) this.#publish();
  }

  /** Notify the checkpoint owner before any final media is required. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Release all metadata at page teardown without changing any backend job. */
  dispose(): void {
    this.#entries.clear();
    this.#listeners.clear();
  }

  /** Keep observation distinct from execution delivery and editor insertion. */
  #publish(): void {
    for (const listener of this.#listeners) listener();
  }
}
