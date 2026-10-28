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
/** Assemble shared output controls from renderer-owned presentation status. */
/** Preserve the same workflow and selection authority for every presentation backend. */
export function canvasViewSnapshot(address, plan, catalog, sources, connected, mediaActions, presentation) {
    return Object.freeze({
        schema: 1,
        address,
        revision: 0,
        view: plan.view,
        sources,
        grid: plan.grid,
        comparisonAvailable: presentation.comparisonAvailable,
        selected: plan.primary?.item ?? null,
        secondary: plan.secondary?.item ?? null,
        preview: presentation.preview,
        resource: Object.freeze(presentation.resource),
        secondaryResource: presentation.secondaryResource
            ? Object.freeze(presentation.secondaryResource)
            : null,
        runStatus: catalog.result.runStatus,
        connected,
        mediaActions,
        zoom: Object.freeze(presentation.zoom),
    });
}
