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
/**
 * Own live preview bytes: deliver each frame to interested renderers, and keep exactly the bytes of
 * each workflow's current preview, the one the result store names, until the store stops naming it.
 */

import type { WorkflowCanvasPreview } from '../domain/WorkflowCanvas.js';
import type { WorkflowPreview } from '../domain/WorkflowPreview.js';
import type { WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { WorkflowResultStore } from '../application/WorkflowResultStore.js';
import type { AttributedWorkflowSource } from '../application/WorkflowExecutionIngestor.js';

export interface WorkflowPreviewConsumer {
  wants(source: AttributedWorkflowSource): boolean;
  receive(preview: WorkflowPreview, blob: Blob): void;
}

/** Own preview delivery identities and the current preview's bytes; renderers keep no copies. */
export class ComfyWorkflowPreviewDelivery {
  readonly #consumers = new Set<WorkflowPreviewConsumer>();
  /** The current preview's bytes per workflow, released once the result store stops naming it. */
  readonly #current = new Map<WorkflowSessionId, { readonly token: string; readonly blob: Blob }>();
  readonly #stopResults: () => void;
  #sequence = 0;
  #disposed = false;

  /** Bind metadata ownership and diagnostics without borrowing Comfy's shared object URLs. */
  constructor(
    private readonly results: WorkflowResultStore,
    private readonly logger: Pick<Console, 'error'>,
    private readonly canvasPreview: (frame: WorkflowCanvasPreview<Blob>) => void = () => {},
  ) {
    this.#stopResults = results.subscribe((sessionId) => {
      const current = this.#current.get(sessionId);
      if (current && results.read(sessionId)?.preview?.token !== current.token)
        this.#current.delete(sessionId);
    });
  }

  /**
   * The bytes of the preview the result store names as current, for a renderer whose plan names
   * it before its live frame was taken: the first frame of a stage arrives before its tab is
   * chosen, and a stopped run's newest image shows when its tab is chosen again. Renderers keep no
   * copies. A stage's final closes its lane, so nothing is handed out for it after that final.
   */
  frame(preview: WorkflowPreview): Blob | null {
    const current = this.#current.get(preview.sessionId);
    return current?.token === preview.token &&
      this.results.read(preview.sessionId)?.preview?.token === preview.token
      ? current.blob
      : null;
  }

  /** Subscribe one explicit renderer interest; inactive surfaces refuse bytes before admission. */
  subscribe(consumer: WorkflowPreviewConsumer): () => void {
    if (this.#disposed) throw new Error('Workflow preview delivery is disposed.');
    this.#consumers.add(consumer);
    return () => {
      this.#consumers.delete(consumer);
    };
  }

  /** Forward a current frame synchronously without holding its Blob after delivery. */
  deliver(source: AttributedWorkflowSource, blob: Blob): void {
    if (this.#disposed) return;
    const sequence = ++this.#sequence;
    const preview: WorkflowPreview = Object.freeze({
      sessionId: source.run.session.sessionId,
      generation: source.run.session.generation,
      promptId: source.run.promptId,
      sourceId: source.source.id,
      runOrder: source.run.order,
      sequence,
      token: `preview:${sequence}`,
      dimensions: null,
    });
    if (!this.results.publishPreview(preview)) return;
    this.#current.set(preview.sessionId, { token: preview.token, blob });
    this.canvasPreview({ source, preview, bytes: blob });
    for (const consumer of this.#consumers) {
      try {
        if (consumer.wants(source)) consumer.receive(preview, blob);
      } catch (error: unknown) {
        this.logger.error('SugarCubes workflow preview delivery failed.', {
          promptId: source.run.promptId,
          sourceId: source.source.id,
          error,
        });
      }
    }
  }

  /** Retire consumers without taking ownership of renderer-held allocations. */
  dispose(): void {
    this.#disposed = true;
    this.#stopResults();
    this.#current.clear();
    this.#consumers.clear();
  }
}
