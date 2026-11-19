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
 * Learn whether a host has its own report of an update to show, through the
 * `sugarcubes.update-report` protocol (`integrations/contracts/update-report.schema.json`), and ask
 * the host to show it.
 *
 * The page sends `watch` once the host lists the protocol; the host answers with a state and sends
 * a new one whenever it changes. Every host message is admitted through the generated guard before
 * it is read. Showing the report is the host's decision: the page only asks, naming the state it
 * was shown.
 */
import {
  isUpdateReportStateMessage,
  type UpdateReportClientMessage,
} from '../embedded/UpdateReportRuntime.js';

/** Protocol identifier shared with desktop hosts. */
export const UPDATE_REPORT_PROTOCOL = 'sugarcubes.update-report';

/** The host transport slice the link needs; it grants no host authority. */
export interface UpdateReportLinkTransport {
  readonly session: string;
  send(message: UpdateReportClientMessage): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/** Hears whether the host has a report; `open` asks the host to show the one that state names. */
export interface UpdateReportWatcher {
  state(available: boolean, open: () => void): void;
}

/** Whether the host has an update report to show now, and the means to ask for it. */
export class HostUpdateReport implements UpdateReportWatcher {
  #open: (() => void) | null = null;
  readonly #listeners = new Set<() => void>();

  /** Whether asking would show a report. */
  get available(): boolean {
    return this.#open !== null;
  }

  /** Adopt the host's newest state. */
  state(available: boolean, open: () => void): void {
    const was = this.available;
    this.#open = available ? open : null;
    if (was !== available) for (const listener of [...this.#listeners]) listener();
  }

  /** Ask the host to show its report; nothing happens while it has none. */
  open(): void {
    this.#open?.();
  }

  /** Hear each change of availability until the returned function is called. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
}

/** Keep one watcher informed of the host's update report, newest state only. */
export class HostUpdateReportLink {
  #stop: (() => void) | null = null;
  #revision = -1;
  #disposed = false;

  constructor(
    private readonly transport: UpdateReportLinkTransport,
    private readonly watcher: UpdateReportWatcher,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Listen before asking the host to watch, so its synchronous answer cannot be lost. */
  start(): void {
    if (this.#stop || this.#disposed) return;
    this.#stop = this.transport.subscribe((value) => this.#receive(value));
    this.#send({ ...this.#envelope(), type: 'watch' });
  }

  /** Stop listening; a host that can no longer be asked has no report to offer. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#stop?.();
    this.#stop = null;
    this.#tell(false, 0);
  }

  #envelope() {
    return {
      protocol: UPDATE_REPORT_PROTOCOL,
      version: 1,
      session: this.transport.session,
    } as const;
  }

  #receive(value: unknown): void {
    if (!isUpdateReportStateMessage(value) || value.session !== this.transport.session) return;
    // An answer to a repeated watch repeats a state already heard.
    if (value.revision <= this.#revision) return;
    this.#revision = value.revision;
    this.#tell(value.available, value.revision);
  }

  #tell(available: boolean, revision: number): void {
    try {
      this.watcher.state(available, () =>
        this.#send({ ...this.#envelope(), type: 'open', revision }),
      );
    } catch (error: unknown) {
      this.report(error);
    }
  }

  /** Send one message; a transport failure retires the link and is reported once. */
  #send(message: UpdateReportClientMessage): void {
    if (this.#disposed) return;
    try {
      this.transport.send(message);
    } catch (error: unknown) {
      this.report(error);
      this.dispose();
    }
  }
}
