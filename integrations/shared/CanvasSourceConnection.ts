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
/** Own source observation through a shared canvas client's authenticated request lifetime. */
import type { CanvasSourceHost } from '../contracts/CanvasSources.js';
import type {
  CanvasSourceChange,
  CanvasSourceCommand,
  CanvasSourceQuery,
  CanvasSourceResult,
} from '../contracts/CanvasProtocol.generated.js';

/** Delegate correlation and transport to the canvas connection's authoritative owners. */
export interface CanvasSourceConnectionPort {
  identity(): string;
  execute(command: CanvasSourceCommand): Promise<CanvasSourceResult>;
  listen(listener: (change: CanvasSourceChange) => void): () => void;
  report(error: unknown): void;
}

/** Retain native observation only while a consumer subscribes to committed source changes. */
export class CanvasSourceConnection implements CanvasSourceHost {
  readonly #listeners = new Set<(change: CanvasSourceChange) => void>();
  #stop: (() => void) | null = null;
  #disposed = false;
  #context: Promise<CanvasSourceResult> | null = null;
  /** Borrow the session transport without creating a second request-ID sequence. */
  constructor(private readonly port: CanvasSourceConnectionPort) {}
  /** Resolve identity from the actual connected canvas rather than an editor-specific name. */
  get canvasId(): string {
    return this.port.identity();
  }
  /** Share simultaneous metadata reads while keeping later reads and captures current. */
  execute(command: CanvasSourceQuery): Promise<CanvasSourceResult> {
    if (this.#disposed)
      return Promise.resolve({
        kind: 'failure',
        status: 'unavailable',
        message: 'Canvas source connection closed.',
      });
    if (command.type !== 'context') return this.port.execute(command);
    if (this.#context) return this.#context;
    const pending = this.port.execute(command).finally(() => {
      if (this.#context === pending) this.#context = null;
    });
    this.#context = pending;
    return pending;
  }
  /** Share one native watch across all interested views and application collaborators. */
  subscribe(listener: (change: CanvasSourceChange) => void): () => void {
    if (this.#disposed) return () => {};
    this.#listeners.add(listener);
    if (!this.#stop) {
      this.#stop = this.port.listen((change) => {
        this.#context = null;
        for (const notify of this.#listeners) notify(change);
      });
      this.#watch(true);
    }
    return () => {
      this.#listeners.delete(listener);
      if (!this.#listeners.size && this.#stop) {
        this.#stop();
        this.#stop = null;
        this.#watch(false);
      }
    };
  }
  /** Release observation before the shared request client tears down its pending work. */
  dispose(): void {
    if (this.#disposed) return;
    const watching = this.#stop !== null;
    this.#disposed = true;
    this.#context = null;
    this.#stop?.();
    this.#stop = null;
    this.#listeners.clear();
    if (watching) this.#watch(false);
  }
  /** Report live transport failures while discarding obsolete teardown feedback. */
  #watch(enabled: boolean): void {
    void this.port
      .execute({ type: 'watch', enabled })
      .then((result) => {
        if (!this.#disposed && result.kind === 'failure')
          this.port.report(new Error(result.message));
      })
      .catch((error: unknown) => {
        if (!this.#disposed) this.port.report(error);
      });
  }
}
