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
 * Ask a host to show its operating system's file dialog, through the `sugarcubes.file-choice`
 * protocol (`integrations/contracts/file-choice.schema.json`).
 *
 * A page can open a file dialog of its own but cannot say where it starts; a host can. Each
 * question names its own request, and the host answers each exactly once: where the person
 * chose, that they dismissed the dialog, or that no dialog was shown. Every host message is
 * admitted through the generated guard before it is read. The host reads and writes nothing;
 * the page uses its own storage for the chosen file.
 */
import { isFileChoiceHostMessage, } from '../embedded/FileChoiceRuntime.js';
/** Protocol identifier shared with desktop hosts. */
export const FILE_CHOICE_PROTOCOL = 'sugarcubes.file-choice';
/** The host's file dialog as the page's features see it: there while a host serves it. */
export class HostFileChoice {
    #asker = null;
    /** Whether a host is there to show the dialog. */
    get available() {
        return this.#asker !== null;
    }
    /** Adopt the link to a host that serves the protocol, or `null` once it is gone. */
    attach(asker) {
        this.#asker = asker;
    }
    /**
     * Let the person choose one existing file of `extension`. The dialog starts in `start`, a
     * forward-slashed path below `folder` (empty for `folder` itself); the choice is placed
     * relative to `folder`.
     */
    open(folder, start, extension) {
        return this.#ask({ kind: 'open', folder, start, extension });
    }
    /** Let the person choose where a file of `extension` is saved, as `open` places its dialog. */
    save(folder, start, name, extension) {
        return this.#ask({ kind: 'save', folder, start, name, extension });
    }
    #ask(question) {
        return this.#asker
            ? this.#asker.ask(question)
            : Promise.reject(new Error('The file dialog is unavailable.'));
    }
}
/** Carry each question to the host and settle it with the host's one answer. */
export class HostFileChoiceLink {
    transport;
    report;
    #stop = null;
    #disposed = false;
    #asked = 0;
    #waiting = new Map();
    constructor(transport, report) {
        this.transport = transport;
        this.report = report;
    }
    /** Listen for the host's answers. */
    start() {
        if (this.#stop || this.#disposed)
            return;
        this.#stop = this.transport.subscribe((value) => this.#receive(value));
    }
    /** Stop listening; a question the host can no longer answer fails. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#stop?.();
        this.#stop = null;
        const waiting = [...this.#waiting.values()];
        this.#waiting.clear();
        for (const question of waiting)
            question.reject(new Error('The file dialog is unavailable.'));
    }
    /** Show one dialog; resolves `null` when dismissed and rejects when none was shown. */
    ask(question) {
        if (this.#disposed || !this.#stop)
            return Promise.reject(new Error('The file dialog is unavailable.'));
        this.#asked += 1;
        const request = `${question.kind}-${String(this.#asked)}`;
        const envelope = {
            protocol: FILE_CHOICE_PROTOCOL,
            version: 1,
            session: this.transport.session,
            request,
            folder: question.folder,
            start: question.start,
            extension: question.extension,
        };
        return new Promise((resolve, reject) => {
            this.#waiting.set(request, { resolve, reject });
            try {
                this.transport.send(question.kind === 'open'
                    ? { ...envelope, type: 'open' }
                    : { ...envelope, type: 'save', name: question.name });
            }
            catch (error) {
                this.report(error);
                this.dispose();
            }
        });
    }
    #receive(value) {
        if (!isFileChoiceHostMessage(value) || value.session !== this.transport.session)
            return;
        const waiting = this.#waiting.get(value.request);
        if (!waiting)
            return;
        this.#waiting.delete(value.request);
        if (value.type === 'chosen')
            waiting.resolve({ name: value.name, relative: value.relative });
        else if (value.type === 'dismissed')
            waiting.resolve(null);
        else
            waiting.reject(new Error(value.reason === 'busy'
                ? 'Another file dialog is already open.'
                : 'The file dialog could not be opened.'));
    }
}
