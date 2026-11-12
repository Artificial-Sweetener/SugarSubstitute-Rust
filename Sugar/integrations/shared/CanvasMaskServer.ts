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
/** Serialize native mask operations and keep mutation replay protection inside one navigation. */
import type {
  CanvasMaskMessage,
  CanvasMaskQuery,
  CanvasMaskResult,
  CanvasMaskReply,
  CanvasMaskChanged,
} from '../contracts/CanvasMasksProtocol.generated.js';
import { isCanvasMaskMessage } from './CanvasMasksValidation.generated.js';
import { MAX_CANVAS_ENCODED_CHARACTERS } from './CanvasImageEncoding.js';
import { CanvasChangeRelay } from './CanvasChangeRelay.js';
import type { CanvasChangeObservation } from '../contracts/CanvasChangeObservation.js';

/** Supply editor operations without importing native dependencies into the shared runtime. */
export interface CanvasMaskServerPort {
  readonly changes: CanvasChangeObservation<CanvasMaskChanged> | null;
  execute(command: CanvasMaskQuery): Promise<CanvasMaskResult>;
  send(message: CanvasMaskMessage): void;
  report(error: unknown): void;
}

/** Bound retained responses and refuse forgotten or reused request identities. */
export class CanvasMaskServer {
  readonly #entries = new Map<number, { fingerprint: string; reply: CanvasMaskReply | null }>();
  #highest = 0;
  #tail: Promise<void> = Promise.resolve();
  #disposed = false;
  #retained = 0;
  readonly #changes: CanvasChangeRelay<CanvasMaskChanged>;
  /** Bind one authenticated session and one authoritative native mask owner. */
  constructor(
    private readonly session: string,
    private readonly port: CanvasMaskServerPort,
  ) {
    this.#changes = new CanvasChangeRelay(port.changes, (change) => {
      if (change.session === this.session) this.#send(change);
    });
  }
  /** Validate commands before placing them into the native operation sequence. */
  receive(value: unknown): boolean {
    if (
      this.#disposed ||
      !isCanvasMaskMessage(value) ||
      value.session !== this.session ||
      !('command' in value)
    )
      return false;
    const fingerprint = JSON.stringify(value.command);
    const existing = this.#entries.get(value.requestId);
    if (existing) {
      if (existing.fingerprint !== fingerprint)
        this.#reply(value.requestId, {
          kind: 'failure',
          status: 'stale',
          message: 'Mask request identity was reused.',
        });
      else if (existing.reply) this.#send(existing.reply);
      return true;
    }
    this.#trim(fingerprint.length);
    if (
      value.requestId <= this.#highest ||
      this.#entries.size >= 64 ||
      this.#retained + fingerprint.length > MAX_CANVAS_ENCODED_CHARACTERS
    ) {
      this.#reply(value.requestId, {
        kind: 'failure',
        status: 'stale',
        message: 'Mask request is stale or the canvas is busy.',
      });
      return true;
    }
    this.#highest = value.requestId;
    this.#entries.set(value.requestId, { fingerprint, reply: null });
    this.#retained += fingerprint.length;
    this.#tail = this.#tail.then(async () => {
      if (this.#disposed) return;
      let result: CanvasMaskResult;
      try {
        if (value.command.type === 'watch') {
          const status = await this.#changes.watch(value.command.enabled);
          result =
            status === 'unavailable'
              ? { kind: 'failure', status, message: 'Canvas mask observation is unavailable.' }
              : { kind: 'watching', supported: status === 'observing' };
        } else result = await this.port.execute(value.command);
      } catch (error: unknown) {
        this.port.report(error);
        result = { kind: 'failure', status: 'error', message: 'The canvas mask operation failed.' };
      }
      this.#reply(value.requestId, result);
    });
    return true;
  }
  /** Prevent queued operations and replies from escaping a destroyed panel. */
  dispose(): void {
    this.#disposed = true;
    this.#changes.dispose();
    this.#entries.clear();
    this.#retained = 0;
  }
  /** Retain only small acknowledgements; large immutable captures are never replayed. */
  #reply(requestId: number, result: CanvasMaskResult): void {
    if (this.#disposed) return;
    const reply: CanvasMaskReply = {
      protocol: 'sugarcubes.canvas.masks',
      version: 2,
      session: this.session,
      requestId,
      result,
    };
    const entry = this.#entries.get(requestId);
    if (entry) entry.reply = reply;
    this.#send(reply);
    if (result.kind === 'captured' && entry) {
      this.#entries.delete(requestId);
      this.#retained -= entry.fingerprint.length;
    }
    this.#trim(0);
  }
  /** Bound request bytes as well as count, retaining the replay frontier after payload eviction. */
  #trim(incoming: number): void {
    while (this.#entries.size > 16 || this.#retained + incoming > MAX_CANVAS_ENCODED_CHARACTERS) {
      const completed = [...this.#entries].find(([, item]) => item.reply !== null);
      if (!completed) break;
      this.#entries.delete(completed[0]);
      this.#retained -= completed[1].fingerprint.length;
    }
  }
  /** Keep transport errors observable without leaking rejected native tasks. */
  #send(message: CanvasMaskMessage): void {
    try {
      this.port.send(message);
    } catch (error: unknown) {
      this.port.report(error);
    }
  }
}
