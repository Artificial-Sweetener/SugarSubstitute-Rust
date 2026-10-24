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
/** Deliver ephemeral preview bytes to interested renderers without retaining a background pixel cache. */

import type { WorkflowCanvasPreview } from '../domain/WorkflowCanvas.js';
import type { WorkflowPreview } from '../domain/WorkflowPreview.js';
import type { WorkflowResultStore } from '../application/WorkflowResultStore.js';
import type { AttributedWorkflowSource } from '../application/WorkflowExecutionIngestor.js';

export interface WorkflowPreviewConsumer {
  wants(source: AttributedWorkflowSource): boolean;
  receive(preview: WorkflowPreview, blob: Blob): void;
}

/** Own preview delivery identities while each interested renderer owns its admitted pixels and URLs. */
export class ComfyWorkflowPreviewDelivery {
  readonly #consumers = new Set<WorkflowPreviewConsumer>();
  #sequence = 0;
  #disposed = false;

  /** Bind metadata ownership and diagnostics without borrowing Comfy's shared object URLs. */
  constructor(
    private readonly results: WorkflowResultStore,
    private readonly logger: Pick<Console, 'error'>,
    private readonly canvasPreview: (frame: WorkflowCanvasPreview<Blob>) => void = () => {},
  ) {}

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
    this.#consumers.clear();
  }
}
