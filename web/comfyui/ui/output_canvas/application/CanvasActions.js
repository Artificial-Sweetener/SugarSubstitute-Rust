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
/** Route shared output controls through one selection owner and the active surface. */
/** Keep shared source and comparison policy independent of how the surface presents pixels. */
export async function applyCanvasAction(command, owners) {
    const { plan, snapshot, surface, scope } = owners;
    if (!scope.current())
        return false;
    if (command.type === 'reveal')
        return await owners.sources.reveal(command.target, scope);
    if (command.type === 'select-source')
        return await owners.sources.select(command.selection, scope);
    owners.sources.cancel();
    if (!scope.current())
        return false;
    switch (command.type) {
        case 'fit':
            return surface.fit(scope);
        case 'native':
            return surface.native(scope);
        case 'zoom':
            return surface.zoom(command.factor, scope);
        case 'pan':
            return surface.pan(command.x, command.y, scope);
        case 'retry':
            return plan.preview ? false : surface.retry(plan, scope);
        case 'admit-large-image':
            return surface.admitLargeImage(scope);
        case 'open-media': {
            const item = command.target === 'primary' ? plan.primary?.item : plan.secondary?.item;
            if (!item)
                return false;
            surface.open(item, scope);
            return true;
        }
        case 'copy-media':
        case 'file-media':
            return await surface.media(command, scope);
        case 'comparison':
            if (command.enabled && (snapshot.preview || snapshot.resource.state !== 'ready'))
                return false;
            break;
        default:
            break;
    }
    return owners.selection.apply(plan.view.viewId, owners.catalog, command);
}
