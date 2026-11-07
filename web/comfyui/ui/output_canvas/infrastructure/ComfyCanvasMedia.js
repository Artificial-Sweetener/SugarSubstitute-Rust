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
/** Resolve full-fidelity workflow files through the same-origin Comfy media boundary. */
import { parseSessionJson, isSessionBinding } from '../../core/embedded/SessionRuntime.js';
/** Preserve exact file identity and stable revision caching without requesting lossy preview conversion. */
export function comfyCanvasMediaUrl(origin, item) {
    if (item.presentationUrl) {
        const retained = new URL(item.presentationUrl, origin);
        const encoded = retained.searchParams.get('binding');
        const binding = encoded ? parseSessionJson(encoded, 4096) : null;
        if (retained.origin !== origin ||
            !['http:', 'https:'].includes(retained.protocol) ||
            retained.href.length > 4096 ||
            [...retained.searchParams.keys()].length !== 1 ||
            retained.searchParams.getAll('binding').length !== 1 ||
            !isSessionBinding(binding) ||
            binding.runtimeSession !== item.provenance.sessionId ||
            binding.generation !== item.provenance.generation ||
            retained.hash ||
            !retained.pathname.endsWith(`/${item.id}`) ||
            !/^\/__sugarsubstitute\/session-media\/v1\/[0-9a-f]{32}\/[0-9a-f-]{36}$/.test(retained.pathname))
            throw new TypeError('Retained canvas media escaped its admitted current-page origin.');
        return retained.href;
    }
    const url = new URL('/view', origin);
    if (!['http:', 'https:'].includes(url.protocol))
        throw new TypeError('Canvas media origin is unsupported.');
    url.search = new URLSearchParams({
        filename: item.file.filename,
        subfolder: item.file.subfolder,
        type: item.file.type,
        revision: item.id,
    }).toString();
    return url.href;
}
/** Open attributed media while isolating the external window and preserving blocked-popup feedback. */
export function openComfyCanvasMedia(window, item) {
    // Create an isolated blank window first so a media response never receives its opener.
    const url = comfyCanvasMediaUrl(window.location.origin, item);
    const opened = window.open('about:blank', '_blank');
    if (!opened)
        throw new Error('The browser blocked opening this output. Allow a new tab and try again.');
    opened.opener = null;
    opened.location.replace(url);
}
