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
/** Read the saved tabs Comfy opens lazily so every open document becomes capturable without activating it. */
import type { WorkflowSessionContext } from '../../execution/domain/WorkflowSession.js';
import type { ComfyDesktopDocuments } from './ComfyDesktopDocuments.js';

export interface ComfyDesktopDocumentReadingOptions {
  readonly documents: Pick<ComfyDesktopDocuments, 'unread' | 'read' | 'isBeingRead'>;
  /** Re-observe the workspace after a pass; the owner decides what a newly readable document means. */
  readonly settled: () => void;
  readonly report: (error: unknown) => void;
}

/**
 * Comfy restores saved tabs in the background and opens a chosen tab before its file arrives, so an open
 * workflow can briefly (or, until activated, indefinitely) hold no document. Comfy loads every tab it opens
 * itself, so only background tabs need this owner: it asks Comfy to read each one through its supported load,
 * one at a time, never overlapping a read Comfy has started, and remembers a failed read per occurrence so a
 * broken file is reported once rather than retried in a loop.
 */
export class ComfyDesktopDocumentReading {
  readonly #failed = new Map<string, number>();
  #running = false;
  #disposed = false;

  constructor(private readonly options: ComfyDesktopDocumentReadingOptions) {}

  /** Start reading every unread workflow Comfy is not already reading, unless a pass is under way. */
  request(): void {
    if (this.#running || this.#disposed || !this.#next()) return;
    this.#running = true;
    void this.#drain();
  }

  /** Whether any open document is still waiting for Comfy to read it. */
  pending(): boolean {
    return this.options.documents.unread().length > 0;
  }

  /** Retire the pass; a read already in flight finishes inside Comfy but nothing observes it. */
  dispose(): void {
    this.#disposed = true;
    this.#failed.clear();
  }

  /** Read workflows until none remain that have not already failed for their current occurrence. */
  async #drain(): Promise<void> {
    try {
      for (let next = this.#next(); next && !this.#disposed; next = this.#next()) {
        try {
          await this.options.documents.read(next);
        } catch (cause: unknown) {
          this.#failed.set(next.sessionId, next.generation);
          this.options.report(
            new Error(`Workflow "${next.path}" could not be read for the desktop session.`, {
              cause,
            }),
          );
        }
      }
    } finally {
      this.#running = false;
      if (!this.#disposed) this.options.settled();
    }
  }

  /** The first unread workflow that Comfy is not reading and whose current occurrence has not failed. */
  #next(): WorkflowSessionContext | undefined {
    const { documents } = this.options;
    const unread = documents.unread();
    for (const id of [...this.#failed.keys()])
      if (!unread.some((context) => context.sessionId === id)) this.#failed.delete(id);
    return unread.find(
      (context) =>
        this.#failed.get(context.sessionId) !== context.generation &&
        !documents.isBeingRead(context),
    );
  }
}
