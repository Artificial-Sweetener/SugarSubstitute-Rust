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
/** Coordinate SugarScript compilation and native workflow materialization. */
/** Report located compilation failures without collapsing them into plain text. */
export class SugarScriptCompileError extends Error {
    diagnostics;
    /** Retain every recovered diagnostic for future presentation surfaces. */
    constructor(diagnostics) {
        const first = diagnostics[0];
        super(first
            ? `${first.message} (line ${String(first.span.start.line)})`
            : 'SugarScript is invalid.');
        this.name = 'SugarScriptCompileError';
        this.diagnostics = diagnostics;
    }
}
/** Keep compilation optional and make the resulting native graph self-contained. */
export class SugarScriptImportService {
    #api;
    #materializer;
    /** Bind transport and graph mutation owners. */
    constructor(api, materializer) {
        this.#api = api;
        this.#materializer = materializer;
    }
    /** Compile and import source without queueing execution. */
    async import(source, origin) {
        const response = await this.#api.compile(source);
        if (!response.plan)
            throw new SugarScriptCompileError(response.diagnostics);
        return this.#materializer.materializeSugarScript(response.plan, source, origin);
    }
}
