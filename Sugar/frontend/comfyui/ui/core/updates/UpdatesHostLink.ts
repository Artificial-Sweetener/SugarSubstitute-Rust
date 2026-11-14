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
 * Present a host's own updater to the page through the `sugarcubes.updates` protocol.
 *
 * Version 2 (`integrations/contracts/updates-v2.schema.json`) names the product each state
 * describes; version 1 (`updates.schema.json`) does not, so its snapshots name none. Every host
 * message is admitted through its version's generated guard before it is read.
 */
import {
  isNamedUpdatesHostMessage,
  isUpdatesHostMessage,
  type NamedUpdatesClientMessage,
  type NamedUpdatesState,
  type UpdatesClientMessage,
  type UpdatesRestartTiming,
  type UpdatesState,
} from '../embedded/UpdatesRuntime.js';
import type { UpdateRequestOutcome, UpdateSnapshot, UpdateSource } from './UpdateSource.js';

/** Protocol identifier shared with desktop hosts. */
export const UPDATES_PROTOCOL = 'sugarcubes.updates';
/** The protocol versions the page speaks; they advance independently of the panel transport. */
export type UpdatesProtocolVersion = 1 | 2;

/** The host transport slice the link needs; it grants no host authority. */
export interface UpdatesLinkTransport {
  readonly session: string;
  send(message: UpdatesClientMessage | NamedUpdatesClientMessage): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/** Translate the host's wire state into the snapshot every update source shares. */
function snapshot(state: UpdatesState | NamedUpdatesState): UpdateSnapshot {
  return Object.freeze({
    product: 'product' in state && state.product.trim() !== '' ? state.product : null,
    currentVersion: state.currentVersion,
    channel: state.channel,
    lastSuccessfulCheck: state.lastSuccessfulCheck?.completedAt ?? null,
    status: state.status,
    report: state.report ?? null,
    previousVersionRunning: state.previousVersionRunning,
    workflowRunning: state.workflowRunning,
    restartAfterWorkflow: state.scheduledRestart === 'after-workflow',
  });
}

/**
 * Watch the host's update state and forward the page's check and restart requests.
 *
 * Each request carries its own identity and settles on the host's answer naming it. A state
 * with a revision no higher than the last one is stale and ignored.
 */
export class UpdatesHostLink implements UpdateSource {
  readonly applies = 'host-restart';
  readonly #listeners = new Set<() => void>();
  readonly #pending = new Map<string, (outcome: UpdateRequestOutcome) => void>();
  #snapshot: UpdateSnapshot | null = null;
  #revision = 0;
  #requests = 0;
  #stop: (() => void) | null = null;
  #disposed = false;

  constructor(
    private readonly transport: UpdatesLinkTransport,
    private readonly report: (error: unknown) => void,
    private readonly version: UpdatesProtocolVersion,
  ) {}

  /** Listen before asking the host to watch, so its synchronous first state cannot be lost. */
  start(): void {
    if (this.#stop || this.#disposed) return;
    this.#stop = this.transport.subscribe((value) => this.#receive(value));
    this.#send({ ...this.#envelope(), type: 'watch' });
  }

  read(): UpdateSnapshot | null {
    return this.#snapshot;
  }

  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    return () => this.#listeners.delete(changed);
  }

  check(): Promise<UpdateRequestOutcome> {
    return this.#request('check', (request) => ({ ...this.#envelope(), type: 'check', request }));
  }

  apply(when: UpdatesRestartTiming): Promise<UpdateRequestOutcome> {
    return this.#request('restart', (request) => ({
      ...this.#envelope(),
      type: 'restart',
      request,
      when,
    }));
  }

  /** Stop watching; requests still waiting for an answer settle as unavailable. */
  dispose(): void {
    this.#disposed = true;
    this.#stop?.();
    this.#stop = null;
    this.#settleAll({ accepted: false, reason: 'unavailable' });
    this.#listeners.clear();
  }

  #envelope() {
    const session = this.transport.session;
    return this.version === 2
      ? ({ protocol: UPDATES_PROTOCOL, version: 2, session } as const)
      : ({ protocol: UPDATES_PROTOCOL, version: 1, session } as const);
  }

  /** Admit a host message through the guard of the version this link speaks. */
  #admit(value: unknown) {
    if (this.version === 2) return isNamedUpdatesHostMessage(value) ? value : null;
    return isUpdatesHostMessage(value) ? value : null;
  }

  #request(
    kind: 'check' | 'restart',
    message: (request: string) => UpdatesClientMessage | NamedUpdatesClientMessage,
  ): Promise<UpdateRequestOutcome> {
    if (this.#disposed || !this.#stop)
      return Promise.resolve({ accepted: false, reason: 'unavailable' });
    const request = `${kind}-${String(++this.#requests)}`;
    return new Promise((resolve) => {
      this.#pending.set(request, resolve);
      if (!this.#send(message(request)))
        this.#settle(request, { accepted: false, reason: 'unavailable' });
    });
  }

  #receive(value: unknown): void {
    const message = this.#admit(value);
    if (!message || message.session !== this.transport.session) return;
    if (message.type === 'accepted') this.#settle(message.request, { accepted: true });
    else if (message.type === 'rejected')
      this.#settle(message.request, { accepted: false, reason: message.reason });
    else if (message.revision > this.#revision) {
      this.#revision = message.revision;
      this.#snapshot = snapshot(message.update);
      for (const changed of [...this.#listeners]) changed();
    }
  }

  #settle(request: string, outcome: UpdateRequestOutcome): void {
    const resolve = this.#pending.get(request);
    if (!resolve) return;
    this.#pending.delete(request);
    resolve(outcome);
  }

  #settleAll(outcome: UpdateRequestOutcome): void {
    for (const request of [...this.#pending.keys()]) this.#settle(request, outcome);
  }

  /** Send one message; a transport failure retires the link and is reported once. */
  #send(message: UpdatesClientMessage | NamedUpdatesClientMessage): boolean {
    if (this.#disposed) return false;
    try {
      this.transport.send(message);
      return true;
    } catch (error: unknown) {
      this.report(error);
      this.dispose();
      return false;
    }
  }
}
