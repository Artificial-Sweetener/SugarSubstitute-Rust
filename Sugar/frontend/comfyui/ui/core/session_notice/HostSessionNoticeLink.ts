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
 * Receive what a host has to tell the person about their session through the
 * `sugarcubes.session-notice` protocol.
 *
 * Version 1 (`integrations/contracts/session-notice.schema.json`) carries the notice of unsaved
 * work; version 2 (`session-notice-v2.schema.json`) adds lines the host words itself and may send
 * at any time. Every host message is admitted through its version's generated guard before it is
 * read. The host keeps owing a notice until the page says it was shown, so the page answers only
 * after the person could see it.
 */
import {
  isLinedSessionNoticeHostMessage,
  isSessionNoticeHostMessage,
  type LinedSessionNoticeClientMessage,
  type SessionNoticeClientMessage,
  type SessionNoticeTone,
} from '../embedded/SessionNoticeRuntime.js';
import type { UnsavedWork } from './UnsavedWorkNotice.js';

/** Protocol identifier shared with desktop hosts. */
export const SESSION_NOTICE_PROTOCOL = 'sugarcubes.session-notice';
/** The protocol versions the page speaks; they advance independently of the panel transport. */
export type SessionNoticeProtocolVersion = 1 | 2;

/** One sentence the host has for the person, in the host's own words. */
export interface HostLine {
  readonly tone: SessionNoticeTone;
  readonly text: string;
  /** Whether the line stays on screen until the person closes it. */
  readonly lasting: boolean;
}

/** The host transport slice the link needs; it grants no host authority. */
export interface SessionNoticeLinkTransport {
  readonly session: string;
  send(message: SessionNoticeClientMessage | LinedSessionNoticeClientMessage): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/** Show the host's notices to the person; each answers whether it reached them. */
export interface SessionNoticePresenter {
  notSaved(lost: UnsavedWork): boolean;
  line(line: HostLine): boolean;
}

/**
 * Ask the host for the notice it owes, show it once, and tell the host it was shown.
 *
 * A notice the page could not show is left owed: the host offers it again to the next page.
 */
export class HostSessionNoticeLink {
  readonly #shown = new Set<number>();
  #stop: (() => void) | null = null;
  #disposed = false;

  constructor(
    private readonly transport: SessionNoticeLinkTransport,
    private readonly present: SessionNoticePresenter,
    private readonly report: (error: unknown) => void,
    private readonly version: SessionNoticeProtocolVersion = 1,
  ) {}

  /** Listen before asking the host to watch, so its synchronous answer cannot be lost. */
  start(): void {
    if (this.#stop || this.#disposed) return;
    this.#stop = this.transport.subscribe((value) => this.#receive(value));
    this.#send({ ...this.#envelope(), type: 'watch' });
  }

  /** Stop listening; a notice not yet shown stays owed by the host. */
  dispose(): void {
    this.#disposed = true;
    this.#stop?.();
    this.#stop = null;
  }

  #envelope() {
    return {
      protocol: SESSION_NOTICE_PROTOCOL,
      version: this.version,
      session: this.transport.session,
    } as const;
  }

  #receive(value: unknown): void {
    const message =
      this.version === 2
        ? isLinedSessionNoticeHostMessage(value)
          ? value
          : null
        : isSessionNoticeHostMessage(value)
          ? value
          : null;
    if (!message || message.session !== this.transport.session) return;
    if (message.type === 'none' || this.#shown.has(message.notice)) return;
    let reached = false;
    try {
      reached =
        message.type === 'line'
          ? this.present.line({
              tone: message.tone,
              text: message.text,
              lasting: message.lasting,
            })
          : this.present.notSaved({ workspace: message.workspace, inputs: message.inputs });
    } catch (error: unknown) {
      this.report(error);
    }
    if (!reached) return;
    this.#shown.add(message.notice);
    this.#send({ ...this.#envelope(), type: 'shown', notice: message.notice });
  }

  /** Send one message; a transport failure retires the link and is reported once. */
  #send(message: SessionNoticeClientMessage | LinedSessionNoticeClientMessage): void {
    if (this.#disposed) return;
    try {
      this.transport.send(message);
    } catch (error: unknown) {
      this.report(error);
      this.dispose();
    }
  }
}
