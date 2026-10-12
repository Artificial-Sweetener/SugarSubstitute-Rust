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
/** Define workflow-qualified identities independently of host graph objects. */
/** Construct an in-memory workflow identity from an adapter-generated key. */
export function workflowSessionId(key) {
    return `workflow:${key}`;
}
/** Qualify a backend job identifier without changing its transport spelling. */
export function promptId(key) {
    return `prompt:${key}`;
}
/** Preserve a complete execution path rather than its final numeric node ID. */
export function outputSourceId(executionPath) {
    return `source:${executionPath}`;
}
/** Identify an immutable image within one accepted job and source revision. */
export function outputResultId(session, prompt, source, revision, batch) {
    return `result:${JSON.stringify([session, prompt, source, revision, batch])}`;
}
