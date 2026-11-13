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
/** Send editing commands one at a time, in order, coalescing superseded continuous edits. */
import type { CanvasEditHost } from '../../../../../integrations/contracts/CanvasEditing.js';
import type {
  CanvasEditCommand,
  CanvasEditResult,
  CanvasEditState,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

interface QueuedCommand {
  readonly command: CanvasEditCommand;
  readonly key: string | null;
  readonly settle: (result: CanvasEditResult | null) => void;
}

/** Receive each successful state and every unexpected failure. */
export interface CanvasEditCommandOutcomes {
  adopt(state: CanvasEditState): void;
  report(error: unknown, command: CanvasEditCommand): void;
}

/**
 * Keep one command in flight so the host observes user intent in order.
 *
 * Slider drags and amount previews pass a coalescing key: a queued, unsent command with the same
 * key is replaced in place and settles with null, so the host receives only the latest value
 * without reordering it around commands such as Apply.
 */
export class CanvasEditCommandQueue {
  readonly #queued: QueuedCommand[] = [];
  #running = false;
  #disposed = false;

  /** Borrow the host's correlated transport; the queue owns no request identities. */
  constructor(
    private readonly host: CanvasEditHost,
    private readonly outcomes: CanvasEditCommandOutcomes,
  ) {}

  /** Queue a command; resolves with its reply, or null when superseded, closed or failed. */
  send(command: CanvasEditCommand, key: string | null = null): Promise<CanvasEditResult | null> {
    if (this.#disposed) return Promise.resolve(null);
    return new Promise((settle) => {
      const entry = { command, key, settle };
      const replaced = key === null ? -1 : this.#queued.findIndex((queued) => queued.key === key);
      if (replaced >= 0) {
        this.#queued[replaced]?.settle(null);
        this.#queued[replaced] = entry;
      } else this.#queued.push(entry);
      void this.#pump();
    });
  }

  /** Settle every unsent command without sending it; an in-flight reply is still adopted. */
  dispose(): void {
    this.#disposed = true;
    for (const entry of this.#queued.splice(0)) entry.settle(null);
  }

  async #pump(): Promise<void> {
    if (this.#running) return;
    this.#running = true;
    try {
      for (let entry = this.#queued.shift(); entry; entry = this.#queued.shift())
        entry.settle(await this.#execute(entry.command));
    } finally {
      this.#running = false;
    }
  }

  async #execute(command: CanvasEditCommand): Promise<CanvasEditResult | null> {
    try {
      const result = await this.host.execute(command);
      if (result.kind === 'state') this.outcomes.adopt(result.state);
      else if (result.status !== 'cancelled')
        this.outcomes.report(new Error(result.message), command);
      return result;
    } catch (error: unknown) {
      this.outcomes.report(error, command);
      return null;
    }
  }
}
