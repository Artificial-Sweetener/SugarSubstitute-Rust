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
/** Own editing commands and pushed edit state over a shared canvas client's session. */
import type { CanvasCancellationSource } from '../contracts/CanvasHost.js';
import type { CanvasChangeObservation } from '../contracts/CanvasChangeObservation.js';
import type { CanvasEditHost } from '../contracts/CanvasEditing.js';
import type {
  CanvasEditCommand,
  CanvasEditResult,
  CanvasEditState,
} from '../contracts/CanvasProtocol.generated.js';

/** Delegate correlation, transport and capability discovery to the canvas client. */
export interface CanvasEditConnectionPort {
  identity(): string;
  /** Report whether the current context advertises editing; other pushed states are refused. */
  advertised(): boolean;
  execute(
    command: CanvasEditCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasEditResult>;
}

/** Fan pushed states out in revision order without a watch request or a second request sequence. */
export class CanvasEditConnection implements CanvasEditHost {
  readonly changes: CanvasChangeObservation<CanvasEditState>;
  readonly #listeners = new Set<{ readonly notify: (state: CanvasEditState) => void }>();
  #revision = -1;
  #disposed = false;
  /** Borrow the session transport; the client remains the sole owner of request identities. */
  constructor(private readonly port: CanvasEditConnectionPort) {
    this.changes = {
      subscribe: (listener) => {
        if (this.#disposed) return { ready: Promise.resolve('unavailable'), stop: () => {} };
        // A distinct entry per subscription keeps one stop from retiring another's listener.
        const entry = { notify: listener };
        this.#listeners.add(entry);
        return {
          ready: Promise.resolve('observing'),
          stop: () => {
            this.#listeners.delete(entry);
          },
        };
      },
    };
  }
  /** Resolve identity from the connected canvas rather than an editor-specific name. */
  get canvasId(): string {
    return this.port.identity();
  }
  /** Execute once through the client; a closed connection answers without transport. */
  execute(
    command: CanvasEditCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasEditResult> {
    if (this.#disposed)
      return Promise.resolve({
        kind: 'failure',
        status: 'unavailable',
        message: 'Canvas edit connection closed.',
      });
    return this.port.execute(command, cancellation);
  }
  /** Publish one pushed state; stale revisions and states from a non-editing canvas are refused. */
  publish(state: CanvasEditState): boolean {
    if (this.#disposed || !this.port.advertised() || state.revision <= this.#revision) return false;
    this.#revision = state.revision;
    const snapshot = snapshotCanvasEditState(state);
    for (const entry of [...this.#listeners])
      if (this.#listeners.has(entry)) entry.notify(snapshot);
    return true;
  }
  /** Retire every listener before the client rejects its pending requests. */
  dispose(): void {
    this.#disposed = true;
    this.#listeners.clear();
  }
}

/** Freeze a validated state so one consumer cannot alter what another observes. */
export function snapshotCanvasEditState(state: CanvasEditState): CanvasEditState {
  return Object.freeze({
    ...state,
    tools: Object.freeze(state.tools.map((tool) => Object.freeze({ ...tool }))),
    options: Object.freeze({
      brush: Object.freeze({ ...state.options.brush }),
      eraser: Object.freeze({ ...state.options.eraser }),
    }),
    selection: state.selection
      ? Object.freeze({ bounds: Object.freeze({ ...state.selection.bounds }) })
      : null,
    history: Object.freeze({ ...state.history }),
    session: state.session ? Object.freeze({ ...state.session }) : null,
  });
}
