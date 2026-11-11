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
/** Project shared canvas deliveries into existing web output-view state and preview operations. */
import type {
  WorkflowCanvasAdapter,
  WorkflowCanvasState,
  WorkflowCanvasPreview,
  WorkflowCanvasResults,
} from '../../execution/domain/WorkflowCanvas.js';
import type { WorkflowResultSession } from '../../execution/domain/WorkflowResultSession.js';
import type { WorkflowSessionId } from '../../execution/domain/ExecutionIdentity.js';

/** Keep web view projection separate from execution attribution and document-layer insertion. */
export class WebCanvasAdapter<Bytes> implements WorkflowCanvasResults {
  readonly adapter: WorkflowCanvasAdapter<Bytes>;
  readonly #results = new Map<WorkflowSessionId, WorkflowResultSession>();
  readonly #listeners = new Set<(session: WorkflowSessionId) => void>();

  /** Supply existing preview presentation while the common coordinator owns event routing. */
  constructor(previews: {
    wants(frame: WorkflowCanvasPreview<Bytes>): boolean;
    receive(frame: WorkflowCanvasPreview<Bytes>): void;
  }) {
    this.adapter = {
      id: 'web',
      capabilities: {
        output: {
          ordering: 'immediate',
          bind: () => ({
            receive: (output) => {
              if (output.state) this.#update(output.state);
            },
          }),
        },
        outputView: {
          update: (state) => this.#update(state),
          restore: (state) => this.#update(state),
        },
        preview: previews,
      },
    };
  }
  /** Read only metadata delivered to this canvas through its declared capabilities. */
  read(sessionId: WorkflowSessionId): WorkflowResultSession | null {
    return this.#results.get(sessionId) ?? null;
  }
  /** Refresh existing tabs and batches after a scoped canvas delivery. */
  subscribe(listener: (session: WorkflowSessionId) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }
  /** Release lightweight projection state with the web canvas lifetime. */
  dispose(): void {
    this.#results.clear();
    this.#listeners.clear();
  }
  /** Apply an authoritative view snapshot without recreating output or merge policies. */
  #update(state: WorkflowCanvasState): void {
    if (state.result === this.read(state.sessionId)) return;
    if (state.result) this.#results.set(state.sessionId, state.result);
    else this.#results.delete(state.sessionId);
    for (const listener of this.#listeners) listener(state.sessionId);
  }
}
