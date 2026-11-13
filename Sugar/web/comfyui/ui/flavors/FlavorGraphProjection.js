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
/** Own Flavor value collection and application against the live graph. */
import { filterTrackedSurfaceValues, trackedSurfaceControls } from '../core/SurfaceValuePolicy.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import { getGroupSugarcubes, setGroupSugarcubes } from '../graph/GroupMetadata.js';
import { buildSurfaceNodesBySymbol } from '../graph/SurfaceNodeResolver.js';
import { applyNodeValue, asFlavorMetadata, cloneValue, readNodePropertyValue, readWidgetValue, } from './FlavorSupport.js';
/** Project tracked Flavor controls between metadata and live graph nodes. */
export class FlavorGraphProjection {
    adapter;
    constructor(adapter) {
        this.adapter = adapter;
    }
    getGraph() {
        return this.adapter?.getApp?.()?.graph || null;
    }
    findGroup(graph, metadata) {
        const instanceId = metadata.instance_id?.trim() || '';
        if (!graph || !instanceId)
            return null;
        return (getGraphGroups(graph).find((group) => getGroupSugarcubes(group)?.instance_id === instanceId) || null);
    }
    buildNodesBySymbol(graph, metadata) {
        return buildSurfaceNodesBySymbol(graph, metadata.nodes, metadata.surface);
    }
    collectValues(graph, metadata) {
        const nodes = this.buildNodesBySymbol(graph, metadata);
        const values = {};
        for (const control of trackedSurfaceControls(metadata.surface)) {
            const controlId = typeof control.control_id === 'string' ? control.control_id.trim() : '';
            const symbol = typeof control.symbol === 'string' ? control.symbol.trim() : '';
            const inputName = typeof control.input_name === 'string' ? control.input_name.trim() : '';
            if (!controlId || !symbol || !inputName)
                continue;
            const node = nodes.get(symbol);
            if (!node)
                continue;
            const widgetValue = readWidgetValue(node, inputName);
            const propertyValue = readNodePropertyValue(node, inputName);
            values[controlId] = cloneValue(widgetValue !== '' || propertyValue == null ? widgetValue : propertyValue);
        }
        return values;
    }
    applyValues(graph, metadata, flavor) {
        const nodes = this.buildNodesBySymbol(graph, metadata);
        const trackedValues = filterTrackedSurfaceValues(metadata.surface, flavor.values);
        for (const control of trackedSurfaceControls(metadata.surface)) {
            const controlId = typeof control.control_id === 'string' ? control.control_id.trim() : '';
            const symbol = typeof control.symbol === 'string' ? control.symbol.trim() : '';
            const inputName = typeof control.input_name === 'string' ? control.input_name.trim() : '';
            if (!controlId ||
                !symbol ||
                !inputName ||
                !Object.prototype.hasOwnProperty.call(trackedValues, controlId))
                continue;
            const node = nodes.get(symbol);
            if (node)
                applyNodeValue(node, inputName, cloneValue(trackedValues[controlId]));
        }
        const group = this.findGroup(graph, metadata);
        if (group) {
            const current = asFlavorMetadata(getGroupSugarcubes(group));
            const options = (current.flavor_options || []).map((entry) => ({
                ...entry,
                selected: entry.id === flavor.id && entry.scope === flavor.scope,
            }));
            setGroupSugarcubes(group, {
                ...current,
                flavor: flavor.id,
                flavor_scope: flavor.scope,
                flavor_options: options,
                flavors: options.map((entry) => entry.name),
                active_flavor_values: cloneValue(trackedValues),
            });
        }
        graph.setDirtyCanvas?.(true, true);
        this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
    }
    selectionNeedsApplication(metadata, flavor) {
        if (!flavor)
            return false;
        if (metadata.flavor !== flavor.id || metadata.flavor_scope !== flavor.scope)
            return true;
        try {
            return (JSON.stringify(filterTrackedSurfaceValues(metadata.surface, metadata.active_flavor_values)) !== JSON.stringify(flavor.values || {}));
        }
        catch (_error) {
            return true;
        }
    }
}
