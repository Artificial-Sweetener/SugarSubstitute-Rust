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
/** Own one visible cube preview URL, qualified by its workflow and current frame identity. */
import { outputSourceId } from '../domain/ExecutionIdentity.js';
/** Release pixels on tab transitions and refuse background frames before allocating a URL. */
export class ComfyCubeLivePreview {
    sessions;
    urls;
    #unsubscribe;
    #listeners = new Set();
    #frame = null;
    #wanted = new Set();
    #active;
    #lastFrameAt = -Infinity;
    /** Keep allocation capabilities at the adapter boundary and limit accepted frames to fifteen per second. */
    constructor(sessions, results, delivery, urls, now, visibility) {
        this.sessions = sessions;
        this.urls = urls;
        this.#active = sessions.read().active;
        this.#unsubscribe = [
            visibility.subscribe(() => {
                if (!visibility.read())
                    this.#clear();
            }),
            sessions.subscribe(() => {
                const active = sessions.read().active;
                if (active?.sessionId !== this.#active?.sessionId ||
                    active?.generation !== this.#active?.generation) {
                    this.#active = active;
                    this.#wanted.clear();
                    this.#clear();
                }
                if (this.#frame &&
                    (active?.sessionId !== this.#frame.preview.sessionId ||
                        active.generation !== this.#frame.preview.generation))
                    this.#clear();
            }),
            results.subscribe((id) => {
                if (this.#frame?.preview.sessionId === id && !results.read(id)?.preview)
                    this.#clear();
            }),
            delivery.subscribe({
                wants: (source) => {
                    const active = sessions.read().active;
                    return (visibility.read() &&
                        this.#listeners.size > 0 &&
                        this.#wanted.has(source.source.id) &&
                        active?.sessionId === source.run.session.sessionId &&
                        active.generation === source.run.session.generation);
                },
                receive: (preview, blob) => {
                    const time = now();
                    if (time - this.#lastFrameAt < 1000 / 15)
                        return;
                    const url = urls.create(blob);
                    if (this.#frame)
                        urls.revoke(this.#frame.url);
                    this.#frame = { preview, url };
                    this.#lastFrameAt = time;
                    this.#notify();
                },
            }),
        ];
    }
    /** Return only the owned current source frame; host-global preview maps are never consulted. */
    read(path) {
        this.#wanted.add(outputSourceId(path));
        const frame = this.#frame;
        const active = this.sessions.read().active;
        if (!frame ||
            active?.sessionId !== frame.preview.sessionId ||
            active.generation !== frame.preview.generation ||
            frame.preview.sourceId !== outputSourceId(path))
            return [];
        return [frame.url];
    }
    /** Refresh mounted cube presentation after an admitted frame or release. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
            if (this.#listeners.size === 0) {
                this.#wanted.clear();
                this.#clear();
            }
        };
    }
    /** Detach byte delivery before releasing the final owned allocation. */
    dispose() {
        for (const unsubscribe of this.#unsubscribe)
            unsubscribe();
        this.#clear();
        this.#listeners.clear();
        this.#wanted.clear();
    }
    /** Drop the allocation and reset admission when its visible workflow ends. */
    #clear() {
        if (this.#frame)
            this.urls.revoke(this.#frame.url);
        this.#frame = null;
        this.#lastFrameAt = -Infinity;
        this.#notify();
    }
    /** Publish allocation changes without retaining byte payloads in listeners. */
    #notify() {
        for (const listener of this.#listeners)
            listener();
    }
}
