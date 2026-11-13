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
/** Publish attributable live-generation transitions without retaining jobs, images, or viewer state. */
import type { WorkflowGenerationEvent } from '../domain/WorkflowGenerationEvent.js';

/** Isolate consumer failures so canvas delivery cannot interrupt queue or result ownership. */
export class WorkflowGenerationEvents {
  readonly #listeners = new Set<(event: WorkflowGenerationEvent) => void>();
  /** Bind actionable failure reporting at the composition boundary. */
  constructor(private readonly report: (error: unknown) => void) {}
  /** Observe future transitions only; restored results are never replayed as new generations. */
  subscribe(listener: (event: WorkflowGenerationEvent) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }
  /** Notify each consumer independently while preserving publication order. */
  publish(event: WorkflowGenerationEvent): void {
    for (const listener of this.#listeners) {
      try {
        listener(event);
      } catch (error: unknown) {
        this.report(error);
      }
    }
  }
  /** Cancel live consumers before releasing subscriptions at integration teardown. */
  dispose(): void {
    this.publish({ type: 'disposed' });
    this.#listeners.clear();
  }
}
