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
/** Resolve Comfy's active node renderer from its authoritative host setting. */
import { isRecord } from '../types/common.js';
/** Read renderer state without allowing stale LiteGraph compatibility flags to win. */
export function resolveComfyRendererMode(app, liteGraph, documentRef) {
    const appRecord = isRecord(app) ? app : {};
    const ui = isRecord(appRecord.ui) ? appRecord.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    const getSettingValue = settings.getSettingValue;
    if (typeof getSettingValue === 'function') {
        const enabled = getSettingValue.call(settings, 'Comfy.VueNodes.Enabled');
        if (typeof enabled === 'boolean')
            return enabled ? 'vue' : 'litegraph';
    }
    if (documentRef?.querySelector('.lg-node[data-node-id]'))
        return 'vue';
    const liteGraphRecord = isRecord(liteGraph) ? liteGraph : {};
    return liteGraphRecord.vueNodesMode === true ? 'vue' : 'litegraph';
}
