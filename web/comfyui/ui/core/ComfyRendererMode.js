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
const VUE_NODES_SETTING_ID = 'Comfy.VueNodes.Enabled';
const VUE_NODES_CHANGE_EVENT = `${VUE_NODES_SETTING_ID}.change`;
/** Read renderer state without allowing stale LiteGraph compatibility flags to win. */
export function resolveComfyRendererMode(app, liteGraph, documentRef) {
    const appRecord = isRecord(app) ? app : {};
    const ui = isRecord(appRecord.ui) ? appRecord.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    const getSettingValue = settings.getSettingValue;
    if (typeof getSettingValue === 'function') {
        const enabled = getSettingValue.call(settings, VUE_NODES_SETTING_ID);
        if (typeof enabled === 'boolean')
            return enabled ? 'vue' : 'litegraph';
    }
    if (documentRef?.querySelector('.lg-node[data-node-id]'))
        return 'vue';
    const liteGraphRecord = isRecord(liteGraph) ? liteGraph : {};
    return liteGraphRecord.vueNodesMode === true ? 'vue' : 'litegraph';
}
/** Publish Comfy's renderer setting transition through its host event contract. */
export function createComfyRendererModeChangeSource(app) {
    const appRecord = isRecord(app) ? app : {};
    const ui = isRecord(appRecord.ui) ? appRecord.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    const addEventListener = settings.addEventListener;
    const removeEventListener = settings.removeEventListener;
    return {
        subscribe(listener) {
            if (typeof addEventListener !== 'function' || typeof removeEventListener !== 'function') {
                return () => undefined;
            }
            const handleChange = () => listener();
            addEventListener.call(settings, VUE_NODES_CHANGE_EVENT, handleChange);
            return () => {
                removeEventListener.call(settings, VUE_NODES_CHANGE_EVENT, handleChange);
            };
        },
    };
}
