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
/** Resolve missing or newly scoped Cube fields without rewriting compatible host selections. */
import { cubeModelRequirement } from './CubeModelRequirements.js';
import { comfyModelCompatibilityContext } from './ComfyModelCompatibilityContext.js';
import { resolveInstalledModel } from './ModelCompatibility.js';
import { modelPickerChoices } from './ModelPickerChoices.js';
import { isScopedModelKind } from './ModelPickerCompatibilityScope.js';
/** Fill Cube-owned host options using shared facts; leave unresolved models available for discovery. */
export function resolveCubeInstalledModel(app, binding, snapshot) {
    if (!binding.anchor.closest('.sugarcubes-cube-face') ||
        binding.disabled() ||
        !isScopedModelKind(binding.kind))
        return;
    const requirement = cubeModelRequirement(app, binding);
    if (!requirement || (!requirement.architecture && !requirement.family))
        return;
    const context = comfyModelCompatibilityContext(app, binding, snapshot);
    if (!context)
        return;
    const selected = binding.readValue();
    const match = resolveInstalledModel(modelPickerChoices(snapshot.items, binding.kind, binding.options), selected, context);
    if (match && match.value !== selected)
        binding.writeValue(match.value);
}
