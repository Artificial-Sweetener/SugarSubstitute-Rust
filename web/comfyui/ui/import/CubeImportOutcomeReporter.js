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
/** Own user-visible reporting for every prepared Cube import path. */
import { buildImportSummary, readImportPayload } from './PlacementPayload.js';
/** Report warnings, completion, and optional container focus through one policy owner. */
export class CubeImportOutcomeReporter {
    #dependencies;
    /** Bind feedback and viewport collaborators. */
    constructor(dependencies) {
        this.#dependencies = dependencies;
    }
    /** Report one import result consistently for commands and pointer placement. */
    report(defaultAlias, backendWarnings, importResult, payloadValue, options = {}) {
        const payload = readImportPayload(payloadValue) ?? {};
        const backendMessages = backendWarnings.filter((warning) => typeof warning === 'string' && Boolean(warning));
        if (backendMessages.length) {
            this.#dependencies.pushToast('warn', 'SugarCube import warnings', backendMessages.join('\n'));
        }
        const frontendWarnings = (importResult?.warnings ?? []).filter(Boolean);
        if (Array.isArray(importResult?.missingTypes) && importResult.missingTypes.length) {
            frontendWarnings.push(`Missing node types: ${importResult.missingTypes.join(', ')}`);
        }
        if (importResult?.message && importResult.success) {
            frontendWarnings.push(importResult.message);
        }
        if (frontendWarnings.length) {
            this.#dependencies.pushToast('warn', 'SugarCube import notes', frontendWarnings.join('\n'));
        }
        const summary = importResult?.summary ?? buildImportSummary(payload);
        if (!importResult?.success) {
            this.#dependencies.pushToast('warn', `SugarCube ${defaultAlias} import incomplete`, importResult?.message || summary);
            return { summary, frontendWarnings };
        }
        this.#dependencies.pushToast('success', `Imported ${defaultAlias}`, summary);
        if (options.focus !== false) {
            this.#dependencies.focusImportedCube(importResult);
        }
        return { summary, frontendWarnings };
    }
}
