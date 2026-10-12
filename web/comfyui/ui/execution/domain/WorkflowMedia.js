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
/** Describe validated media payloads and preserve source batch positions independently of decoding. */
/** Copy media references at ownership boundaries without retaining mutable caller collections. */
export function freezeWorkflowMedia(payload) {
    const copy = (entries) => Object.freeze(entries.map((entry) => entry
        ? Object.freeze({
            ...entry,
            file: Object.freeze({ ...entry.file }),
            dimensions: entry.dimensions ? Object.freeze({ ...entry.dimensions }) : null,
        })
        : null));
    return Object.freeze({
        images: copy(payload.images),
        videos: copy(payload.videos),
        text: Object.freeze([...payload.text]),
    });
}
/** Merge declared incremental arrays and preserve only host-owned input previews on empty image updates. */
export function combineWorkflowMedia(previous, next, merge, containsImages) {
    const inputPreview = previous !== null &&
        previous.images.length > 0 &&
        previous.images.every((entry) => entry?.file.type === 'input');
    const images = !containsImages && inputPreview ? previous.images : next.images;
    return freezeWorkflowMedia({
        images: Object.freeze(merge && previous ? [...previous.images, ...images] : [...images]),
        videos: Object.freeze(merge && previous ? [...previous.videos, ...next.videos] : [...next.videos]),
        text: Object.freeze(merge && previous ? [...previous.text, ...next.text] : [...next.text]),
    });
}
