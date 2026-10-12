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
/** Normalize Comfy media records into immutable descriptors before they enter result ownership. */
import { isRecord } from '../../types/common.js';
import { freezeWorkflowMedia, } from '../domain/WorkflowMedia.js';
/** Preserve declared array positions and accept only finite, relative host media references. */
export function decodeComfyMedia(value) {
    if (!record(value))
        return null;
    const animated = Array.isArray(value.animated) ? value.animated : [];
    const images = Array.isArray(value.images)
        ? value.images.map((image, index) => media(image, animated[index] === true || (animated[0] === true && animated.length === 1)
            ? 'animated'
            : 'image'))
        : [];
    const videos = Array.isArray(value.video)
        ? value.video.map((video) => media(video, 'video'))
        : [];
    const text = typeof value.text === 'string'
        ? [value.text]
        : Array.isArray(value.text)
            ? value.text.filter((entry) => typeof entry === 'string')
            : [];
    const payload = freezeWorkflowMedia({ images, videos, text });
    const containsImages = Array.isArray(value.images) && value.images.length > 0;
    return Object.freeze({
        payload,
        containsImages,
        delivery: JSON.stringify([payload, containsImages]),
    });
}
/** Decode one file reference while keeping failed positions explicit instead of shifting batch indices. */
function media(value, kind) {
    if (!record(value))
        return null;
    const filename = relativePath(value.filename, false);
    const subfolder = value.subfolder === undefined ? '' : relativePath(value.subfolder, true);
    const type = value.type ?? 'output';
    if (!filename || subfolder === null || (type !== 'input' && type !== 'output' && type !== 'temp'))
        return null;
    const file = Object.freeze({ filename, subfolder, type });
    const suffix = filename.split('.').at(-1)?.toLowerCase();
    const mediaKind = suffix === 'svg'
        ? 'unsupported'
        : suffix === 'mp4' || suffix === 'webm'
            ? 'video'
            : kind === 'image' && (suffix === 'gif' || suffix === 'apng')
                ? 'animated'
                : kind;
    return Object.freeze({ file, dimensions: dimensions(value), mediaKind });
}
/** Reject traversal, URL/absolute paths and control characters at the host-media adapter boundary. */
function relativePath(value, allowEmpty) {
    if (typeof value !== 'string' || value.length > 4096 || value.includes(':'))
        return null;
    for (const character of value)
        if (character.charCodeAt(0) < 32)
            return null;
    const path = value.replaceAll('\\', '/');
    if (path.startsWith('/') || (!path && !allowEmpty))
        return null;
    if (path.split('/').some((part) => part === '..' || part === '.'))
        return null;
    return path;
}
/** Read trustworthy dimensions when a host provides them, leaving unknown geometry for decode qualification. */
function dimensions(value) {
    const width = value.width;
    const height = value.height;
    if (typeof width !== 'number' ||
        typeof height !== 'number' ||
        !Number.isSafeInteger(width) ||
        !Number.isSafeInteger(height) ||
        width <= 0 ||
        height <= 0 ||
        width > 1048576 ||
        height > 1048576)
        return null;
    return Object.freeze({ width, height });
}
/** Accept structured records rather than arrays masquerading as node output objects. */
function record(value) {
    return isRecord(value) && !Array.isArray(value);
}
