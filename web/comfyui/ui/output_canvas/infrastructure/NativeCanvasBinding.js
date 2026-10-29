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
/** Keep per-binding revision and cancellation custody at the presentation adapter boundary. */
import { CanvasCancellation } from '../../core/embedded/CanvasRuntime.js';
/** Allocate once across preparation, presentation, viewport commands and release. */
function revision(binding) {
    if (binding.revision === Number.MAX_SAFE_INTEGER)
        throw new Error('Canvas surface revision exhausted.');
    return ++binding.revision;
}
/** Send through the presentation's single binding without a second queue or deadline owner. */
export async function sendNativeCanvasOperation(binding, address, operation) {
    const next = revision(binding);
    const cancellation = new CanvasCancellation();
    binding.active = { cancellation, dependsOnView: operation.type !== 'surface.prepare' };
    if (operation.type === 'surface.geometry')
        binding.geometry = null;
    if (operation.type === 'surface.select')
        binding.selection = null;
    // Presentation keys already fence physical geometry; focus-only updates preserve the camera.
    if (operation.type === 'surface.select')
        binding.viewportKey = null;
    binding.attempted = true;
    try {
        return await binding.host.execute({ ...operation, address, revision: next }, cancellation);
    }
    finally {
        if (binding.active?.cancellation === cancellation)
            binding.active = null;
    }
}
/** Observe release through the same revision stream while retaining unexpected failure causes. */
export async function releaseNativeCanvasBinding(binding, address) {
    try {
        const result = await binding.host.execute({
            type: 'surface.release',
            address,
            revision: revision(binding),
        });
        if (result.status !== 'applied' && result.status !== 'stale')
            throw new Error(result.message || 'Native canvas release failed.');
    }
    catch (error) {
        throw error instanceof Error
            ? error
            : new Error('Native canvas release failed.', { cause: error });
    }
}
