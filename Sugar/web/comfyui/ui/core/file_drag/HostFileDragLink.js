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
 * Receive the names of dragged files from a host through the `sugarcubes.file-drag` protocol
 * (`integrations/contracts/file-drag.schema.json`).
 *
 * The page asks for nothing: a host that lists the protocol says when a drag of files comes over
 * its window and when it goes. Every host message is admitted through the generated guard before
 * it is read.
 */
import { isFileDragHostMessage } from '../embedded/FileDragRuntime.js';
/** Keep the page's record of the dragged files in step with what the host reports. */
export class HostFileDragLink {
    transport;
    dragged;
    #stop = null;
    constructor(transport, dragged) {
        this.transport = transport;
        this.dragged = dragged;
    }
    /** Start listening to the host. */
    start() {
        this.#stop ??= this.transport.subscribe((value) => {
            if (!isFileDragHostMessage(value) || value.session !== this.transport.session)
                return;
            this.dragged.set(value.type === 'entered' ? value.files.map((file) => file.name) : null);
        });
    }
    /** Stop listening and forget the drag, which the page can no longer follow. */
    dispose() {
        this.#stop?.();
        this.#stop = null;
        this.dragged.set(null);
    }
}
