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
/** Keep canvas views independent of their current rendering surface or window. */
/** Derive the result session's canvas identity without using a tab title. */
export function canvasSessionId(workflow) {
    return `canvas:${workflow}`;
}
/** Construct a view identity from an authority-generated key. */
export function canvasViewId(key) {
    return `view:${key}`;
}
/** Construct a presentation window identity at its adapter boundary. */
export function windowId(key) {
    return `window:${key}`;
}
/** Construct a single-use attachment identity at its lifecycle boundary. */
export function surfaceAttachmentId(key) {
    return `attachment:${key}`;
}
/** Compare every attachment coordinate when rejecting delayed delivery. */
export function sameCanvasAttachment(left, right) {
    return (left.sessionId === right.sessionId &&
        left.viewId === right.viewId &&
        left.windowId === right.windowId &&
        left.attachmentId === right.attachmentId &&
        left.generation === right.generation);
}
