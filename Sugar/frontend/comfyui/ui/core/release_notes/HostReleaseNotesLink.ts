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
 * Read a host's own release notes through the `sugarcubes.release-notes` protocol, version 1.
 *
 * `integrations/contracts/release-notes.schema.json` owns the wire shapes; every host message is
 * admitted through the generated guard before it is read. The host also asks the page to show
 * what's new when the user asks it to, which listeners of {@link onOpen} receive.
 */
import {
  isReleaseNotesHostMessage,
  type ReleaseNotesClientMessage,
} from '../embedded/ReleaseNotesRuntime.js';
import {
  unavailableNotes,
  type ReleaseNotesIdentity,
  type ReleaseNotesOutcome,
  type ReleaseNotesSource,
} from './ReleaseNotesSource.js';

/** Protocol identifier shared with desktop hosts. */
export const RELEASE_NOTES_PROTOCOL = 'sugarcubes.release-notes';
/** Protocol version; it advances independently of the panel transport. */
export const RELEASE_NOTES_PROTOCOL_VERSION = 1;

/** The host transport slice the link needs; it grants no host authority. */
export interface ReleaseNotesLinkTransport {
  readonly session: string;
  send(message: ReleaseNotesClientMessage): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/**
 * Follow the host's product and version, read its notes on request, and announce its asks to
 * show what's new.
 *
 * Each load carries its own name and settles on the host's answer naming it. A state with a
 * revision no higher than the last one is stale and ignored.
 */
export class HostReleaseNotesLink implements ReleaseNotesSource {
  readonly #listeners = new Set<() => void>();
  readonly #openListeners = new Set<() => void>();
  readonly #pending = new Map<string, (outcome: ReleaseNotesOutcome) => void>();
  #identity: ReleaseNotesIdentity | null = null;
  #revision = 0;
  #requests = 0;
  #stop: (() => void) | null = null;
  #disposed = false;

  constructor(
    private readonly transport: ReleaseNotesLinkTransport,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Listen before asking the host to watch, so its synchronous first state cannot be lost. */
  start(): void {
    if (this.#stop || this.#disposed) return;
    this.#stop = this.transport.subscribe((value) => this.#receive(value));
    this.#send({ ...this.#envelope(), type: 'watch' });
  }

  read(): ReleaseNotesIdentity | null {
    return this.#identity;
  }

  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    return () => this.#listeners.delete(changed);
  }

  /** Observe the host asking the page to show what's new. */
  onOpen(open: () => void): () => void {
    this.#openListeners.add(open);
    return () => this.#openListeners.delete(open);
  }

  load(since: string): Promise<ReleaseNotesOutcome> {
    if (this.#disposed || !this.#stop) return Promise.resolve(unavailableNotes());
    const request = `load-${String(++this.#requests)}`;
    return new Promise((resolve) => {
      this.#pending.set(request, resolve);
      if (!this.#send({ ...this.#envelope(), type: 'load', request, since }))
        this.#settle(request, unavailableNotes());
    });
  }

  /** Stop following the host; reads still waiting for an answer settle as unavailable. */
  dispose(): void {
    this.#disposed = true;
    this.#stop?.();
    this.#stop = null;
    for (const request of [...this.#pending.keys()]) this.#settle(request, unavailableNotes());
    this.#listeners.clear();
    this.#openListeners.clear();
  }

  #envelope() {
    return {
      protocol: RELEASE_NOTES_PROTOCOL,
      version: RELEASE_NOTES_PROTOCOL_VERSION,
      session: this.transport.session,
    } as const;
  }

  #receive(value: unknown): void {
    if (!isReleaseNotesHostMessage(value) || value.session !== this.transport.session) return;
    switch (value.type) {
      case 'notes':
        this.#settle(value.request, {
          kind: 'notes',
          releases: value.releases,
          omitted: value.omitted,
        });
        return;
      case 'failed':
        this.#settle(value.request, { kind: 'failed', reason: value.reason });
        return;
      case 'open':
        for (const open of [...this.#openListeners]) open();
        return;
      case 'state':
        if (value.revision <= this.#revision) return;
        this.#revision = value.revision;
        this.#identity = Object.freeze({
          product: value.product,
          currentVersion: value.currentVersion,
        });
        for (const changed of [...this.#listeners]) changed();
    }
  }

  #settle(request: string, outcome: ReleaseNotesOutcome): void {
    const resolve = this.#pending.get(request);
    if (!resolve) return;
    this.#pending.delete(request);
    resolve(outcome);
  }

  /** Send one message; a transport failure retires the link and is reported once. */
  #send(message: ReleaseNotesClientMessage): boolean {
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
