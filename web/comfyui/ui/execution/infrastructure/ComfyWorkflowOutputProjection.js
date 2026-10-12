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
/** Project shared workflow results into the currently committed Comfy surface without a second cache. */
/** Qualify cube and thumbnail lookups through explicit active-session commitment. */
export class ComfyWorkflowOutputProjection {
    previews;
    sessions;
    results;
    /** Bind host preview access separately from authoritative final descriptor ownership. */
    constructor(previews, sessions, results) {
        this.previews = previews;
        this.sessions = sessions;
        this.results = results;
    }
    /** Read one exact full source path within the committed workflow generation. */
    output(executionPath) {
        const active = this.sessions.read().active;
        if (!active)
            return null;
        const session = this.results.read(active.sessionId);
        if (session?.context.generation !== active.generation)
            return null;
        return session.outputs.find((output) => output.source.executionPath === executionPath) ?? null;
    }
    /** Read the active cube frame from its allocation owner rather than a shared host URL map. */
    livePreview(executionPath) {
        return this.previews.read(executionPath);
    }
    /** Refresh only mounted consumers of the active workflow, including a committed tab transition. */
    subscribe(listener) {
        const results = this.results.subscribe((id) => {
            if (this.sessions.read().active?.sessionId === id)
                listener();
        });
        const previews = this.previews.subscribe(listener);
        let previous = this.sessions.read().active;
        const sessions = this.sessions.subscribe((snapshot) => {
            const active = snapshot.active;
            if (previous?.sessionId !== active?.sessionId ||
                previous?.generation !== active?.generation) {
                previous = active;
                listener();
            }
        });
        return () => {
            results();
            sessions();
            previews();
        };
    }
}
