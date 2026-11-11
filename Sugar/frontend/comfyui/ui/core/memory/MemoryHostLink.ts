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
/** Join the page's cache registry to a desktop host's application memory budget. */
import type { MemoryBudgetRegistry } from './MemoryBudgetRegistry.js';
import {
  type MemoryClientMessage,
  type MemoryPreference,
  preferenceMessage,
  readAllowance,
  usageMessage,
} from './MemoryHostProtocol.js';

/** The host transport slice the link needs; it grants no host authority. */
export interface MemoryLinkTransport {
  readonly session: string;
  send(message: MemoryClientMessage): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/** The stored memory preference and its changes. */
export interface MemoryPreferenceSource {
  read(): MemoryPreference;
  subscribe(listener: (preference: MemoryPreference) => void): () => void;
}

/**
 * Report cache usage and the stored preference to the host, and apply the host's allowance.
 *
 * A host that does not speak the protocol rejects the first report; the link then stops and
 * the registry keeps its standalone allowance.
 */
export class MemoryHostLink {
  #stops: (() => void)[] = [];
  #generation = 0;
  #enabled = true;

  constructor(
    private readonly transport: MemoryLinkTransport,
    private readonly registry: MemoryBudgetRegistry,
    private readonly preferences: MemoryPreferenceSource,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Listen for allowances before announcing the preference and current usage. */
  start(): void {
    if (this.#stops.length > 0) return;
    const session = this.transport.session;
    this.#stops.push(
      this.transport.subscribe((value) => {
        const allowance = readAllowance(value, session);
        if (!allowance || allowance.generation <= this.#generation) return;
        this.#generation = allowance.generation;
        this.registry.setAllowance(allowance.allowanceBytes, allowance.level);
      }),
      this.registry.subscribe((usage) => this.#send(usageMessage(session, usage))),
      this.preferences.subscribe((preference) =>
        this.#send(preferenceMessage(session, preference)),
      ),
    );
    this.#send(preferenceMessage(session, this.preferences.read()));
    this.#send(usageMessage(session, this.registry.usage()));
  }

  /** Stop reporting; the registry keeps the last allowance it received. */
  dispose(): void {
    for (const stop of this.#stops.splice(0)) stop();
  }

  #send(message: MemoryClientMessage): void {
    if (!this.#enabled) return;
    try {
      this.transport.send(message);
    } catch (error: unknown) {
      this.#enabled = false;
      this.dispose();
      this.report(error);
    }
  }
}
