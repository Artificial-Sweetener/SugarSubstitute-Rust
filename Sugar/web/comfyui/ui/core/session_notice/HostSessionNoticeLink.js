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
import { isLinedSessionNoticeHostMessage, isSessionNoticeHostMessage, } from '../embedded/SessionNoticeRuntime.js';
/** Protocol identifier shared with desktop hosts. */
export const SESSION_NOTICE_PROTOCOL = 'sugarcubes.session-notice';
/**
 * Ask the host for the notice it owes, show it once, and tell the host it was shown.
 *
 * A notice the page could not show is left owed: the host offers it again to the next page.
 */
export class HostSessionNoticeLink {
    transport;
    present;
    report;
    version;
    #shown = new Set();
    #stop = null;
    #disposed = false;
    constructor(transport, present, report, version = 1) {
        this.transport = transport;
        this.present = present;
        this.report = report;
        this.version = version;
    }
    /** Listen before asking the host to watch, so its synchronous answer cannot be lost. */
    start() {
        if (this.#stop || this.#disposed)
            return;
        this.#stop = this.transport.subscribe((value) => this.#receive(value));
        this.#send({ ...this.#envelope(), type: 'watch' });
    }
    /** Stop listening; a notice not yet shown stays owed by the host. */
    dispose() {
        this.#disposed = true;
        this.#stop?.();
        this.#stop = null;
    }
    #envelope() {
        return {
            protocol: SESSION_NOTICE_PROTOCOL,
            version: this.version,
            session: this.transport.session,
        };
    }
    #receive(value) {
        const message = this.version === 2
            ? isLinedSessionNoticeHostMessage(value)
                ? value
                : null
            : isSessionNoticeHostMessage(value)
                ? value
                : null;
        if (!message || message.session !== this.transport.session)
            return;
        if (message.type === 'none' || this.#shown.has(message.notice))
            return;
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
        }
        catch (error) {
            this.report(error);
        }
        if (!reached)
            return;
        this.#shown.add(message.notice);
        this.#send({ ...this.#envelope(), type: 'shown', notice: message.notice });
    }
    /** Send one message; a transport failure retires the link and is reported once. */
    #send(message) {
        if (this.#disposed)
            return;
        try {
            this.transport.send(message);
        }
        catch (error) {
            this.report(error);
            this.dispose();
        }
    }
}
