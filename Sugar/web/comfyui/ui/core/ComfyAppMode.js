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
/** Adapt Comfy's public workflow store into validated app-mode lifecycle notifications. */
import { isRecord } from '../types/common.js';
/** Read the same active workflow mode that drives Comfy's graph and app surfaces. */
export function createComfyAppModeSource(app) {
    const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : {};
    const workflow = isRecord(manager.workflow) ? manager.workflow : {};
    const read = () => {
        const active = isRecord(workflow.activeWorkflow) ? workflow.activeWorkflow : {};
        const mode = active.activeMode ?? active.initialMode ?? 'graph';
        return mode === 'app' || mode === 'builder:arrange';
    };
    return {
        get active() {
            return read();
        },
        subscribe(listener) {
            const subscribe = workflow.$subscribe;
            if (typeof subscribe !== 'function')
                return () => undefined;
            let active = true;
            let previous = read();
            const unsubscribe = subscribe.call(workflow, () => {
                const next = read();
                if (!active || previous === next)
                    return;
                previous = next;
                listener(next);
            }, { detached: true, flush: 'sync' });
            return () => {
                active = false;
                if (typeof unsubscribe === 'function')
                    unsubscribe();
            };
        },
    };
}
