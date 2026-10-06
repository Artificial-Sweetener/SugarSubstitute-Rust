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
/** Keep native per-widget generation from splitting one shared seed into independent values. */
import { readNativeSeedControl } from '../../surface/appearance/ComfyNativeSeedControl.js';
import { linkedFieldMembers } from '../domain/RelationResolver.js';
/** Lease native generation controls so disconnection, deletion, and workflow switching restore them. */
export class NativeSharedSeedModes {
    #leases = new Map();
    /** Freeze enrolled controls and retain their dormant local generation choices in workflow metadata. */
    reconcile(snapshot, inventory) {
        let changed = false;
        const retain = new Set();
        for (const field of snapshot.fields.filter((candidate) => candidate.semantic === 'seed')) {
            const owner = inventory.owners.get(field.id);
            if (!owner)
                continue;
            const control = readNativeSeedControl(owner.node, owner.widget.name);
            if (!control)
                continue;
            const resolved = snapshot.resolved.find((item) => item.field.id === field.id);
            const controlled = Boolean(resolved?.override ||
                linkedFieldMembers(field.id, snapshot.fields, snapshot.nodes, snapshot.state).length > 1);
            if (!controlled)
                continue;
            if (!Object.hasOwn(snapshot.state.localSeedModes, field.id)) {
                snapshot.state.localSeedModes[field.id] = control.readMode();
                changed = true;
            }
            retain.add(owner.widget);
            this.#leases.set(owner.widget, {
                id: field.id,
                control,
                mode: snapshot.state.localSeedModes[field.id] ?? 'fixed',
            });
            control.writeMode('fixed');
        }
        for (const [widget, lease] of this.#leases) {
            if (retain.has(widget))
                continue;
            lease.control.writeMode(lease.mode);
            this.#leases.delete(widget);
            if (Object.hasOwn(snapshot.state.localSeedModes, lease.id)) {
                Reflect.deleteProperty(snapshot.state.localSeedModes, lease.id);
                changed = true;
            }
        }
        return changed;
    }
    /** Restore native controls before releasing references to the previous workflow. */
    dispose() {
        for (const lease of this.#leases.values())
            lease.control.writeMode(lease.mode);
        this.#leases.clear();
    }
}
