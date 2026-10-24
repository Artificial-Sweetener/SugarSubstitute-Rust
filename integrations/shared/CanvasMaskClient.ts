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
/** Correlate bounded optional mask operations within one authenticated panel lifetime. */
import type { CanvasMaskHost } from '../contracts/CanvasMasks.js';
import type {
  CanvasMaskMessage,
  CanvasMaskCommand,
  CanvasMaskResult,
  CanvasMaskChanged,
} from '../contracts/CanvasMasksProtocol.generated.js';
import { isCanvasMaskMessage } from './CanvasMasksValidation.generated.js';

/** Inject transport and deadlines from the owning runtime. */
export interface CanvasMaskClientPort {
  send(message: CanvasMaskMessage): void;
  later(milliseconds: number, callback: () => void): () => void;
}

/** Keep request failures explicit and never retry a native mask mutation. */
export class CanvasMaskClient implements CanvasMaskHost {
  readonly #pending = new Map<number, { resolve(result: CanvasMaskResult): void; stop(): void }>();
  readonly #listeners = new Set<(change: CanvasMaskChanged) => void>();
  #next = 0;
  #disposed = false;
  /** Bind a single trusted navigation and its runtime clock. */
  constructor(
    private readonly session: string,
    private readonly port: CanvasMaskClientPort,
    private readonly identity: () => string = () => '',
  ) {}
  /** Read the negotiated canvas identity without guessing from transport or editor names. */
  get canvasId(): string {
    return this.identity();
  }
  /** Submit one capability operation and retire it on timeout or connection loss. */
  execute(command: CanvasMaskCommand): Promise<CanvasMaskResult> {
    if (this.#disposed)
      return Promise.resolve({
        kind: 'failure',
        status: 'unavailable',
        message: 'Canvas mask connection closed.',
      });
    const requestId = ++this.#next;
    return new Promise((resolve) => {
      const stop = this.port.later(30000, () => {
        this.#pending.delete(requestId);
        resolve({
          kind: 'failure',
          status: 'unavailable',
          message: 'Canvas mask operation timed out; it was not retried.',
        });
      });
      this.#pending.set(requestId, { resolve, stop });
      try {
        this.port.send({
          protocol: 'sugarcubes.canvas.masks',
          version: 1,
          session: this.session,
          requestId,
          command,
        });
      } catch (error: unknown) {
        this.#pending.delete(requestId);
        stop();
        resolve({
          kind: 'failure',
          status: 'unavailable',
          message:
            error instanceof Error ? error.message.slice(0, 512) : 'Canvas mask transport failed.',
        });
      }
    });
  }
  /** Receive committed changes independently of in-flight replies. */
  subscribe(listener: (change: CanvasMaskChanged) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }
  /** Validate the complete bounded envelope before exposing any native value. */
  receive(value: unknown): boolean {
    if (this.#disposed || !isCanvasMaskMessage(value) || value.session !== this.session)
      return false;
    if ('type' in value) {
      for (const listener of this.#listeners) listener(value);
      return true;
    }
    if (!('result' in value)) return false;
    const pending = this.#pending.get(value.requestId);
    if (!pending) return false;
    this.#pending.delete(value.requestId);
    pending.stop();
    pending.resolve(value.result);
    return true;
  }
  /** Settle outstanding operations before releasing native-change consumers. */
  dispose(): void {
    this.#disposed = true;
    for (const pending of this.#pending.values()) {
      pending.stop();
      pending.resolve({
        kind: 'failure',
        status: 'unavailable',
        message: 'Canvas mask connection closed.',
      });
    }
    this.#pending.clear();
    this.#listeners.clear();
  }
}
