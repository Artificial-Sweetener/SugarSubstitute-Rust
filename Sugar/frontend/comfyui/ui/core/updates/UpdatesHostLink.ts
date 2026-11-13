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
 * Present a host's own updater to the page through the `sugarcubes.updates` protocol, version 1.
 *
 * `integrations/contracts/updates.schema.json` owns the wire shapes; every host message is
 * admitted through the generated guard before it is read.
 */
import {
  isUpdatesHostMessage,
  type UpdatesClientMessage,
  type UpdatesRestartTiming,
  type UpdatesState,
} from '../embedded/UpdatesRuntime.js';
import type { UpdateRequestOutcome, UpdateSnapshot, UpdateSource } from './UpdateSource.js';

/** Protocol identifier shared with desktop hosts. */
export const UPDATES_PROTOCOL = 'sugarcubes.updates';
/** Protocol version; it advances independently of the panel transport. */
export const UPDATES_PROTOCOL_VERSION = 1;

/** The host transport slice the link needs; it grants no host authority. */
export interface UpdatesLinkTransport {
  readonly session: string;
  send(message: UpdatesClientMessage): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/** Translate the host's wire state into the snapshot every update source shares. */
function snapshot(state: UpdatesState): UpdateSnapshot {
  return Object.freeze({
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
    return {
      protocol: UPDATES_PROTOCOL,
      version: UPDATES_PROTOCOL_VERSION,
      session: this.transport.session,
    } as const;
  }

  #request(
    kind: 'check' | 'restart',
    message: (request: string) => UpdatesClientMessage,
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
    if (!isUpdatesHostMessage(value) || value.session !== this.transport.session) return;
    if (value.type === 'accepted') this.#settle(value.request, { accepted: true });
    else if (value.type === 'rejected')
      this.#settle(value.request, { accepted: false, reason: value.reason });
    else if (value.revision > this.#revision) {
      this.#revision = value.revision;
      this.#snapshot = snapshot(value.update);
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
  #send(message: UpdatesClientMessage): boolean {
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
