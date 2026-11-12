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
/** Isolate Comfy's host-owned model viewer and serialize uncancellable loader work. */
import { isRecord } from '../../types/common.js';
/** Invoke only an explicitly qualified method at the dynamic Comfy boundary. */
function invoke(owner, method, ...args) {
    if (!isRecord(owner) || typeof owner[method] !== 'function')
        throw new Error(`ComfyUI's 3D viewer does not provide ${method}.`);
    return Reflect.apply(owner[method], owner, args);
}
/** Use Comfy's current factory without bundling its viewer, loaders, or Three.js. */
export class ComfyCanvasModelRuntime {
    host;
    #pending = Promise.resolve();
    /** Resolve the host API lazily so images and video do not initialize 3D. */
    constructor(host) {
        this.host = host;
    }
    /** Serialize host loads because the host loader has no cancellation contract. */
    async load(container, url, filename, signal) {
        const previous = this.#pending;
        let finish = () => { };
        this.#pending = new Promise((resolve) => {
            finish = resolve;
        });
        await previous;
        let raw = null;
        try {
            if (signal.aborted)
                throw signal.reason;
            const api = this.host();
            const factory = isRecord(api) ? api.createLoad3d : null;
            raw = invoke(factory, 'createLoad3d', container, {
                isViewerMode: true,
                disablePreview: true,
            });
            await invoke(raw, 'loadModel', url, filename);
            if (signal.aborted)
                throw signal.reason;
            if (!invoke(raw, 'getCurrentModel'))
                throw new Error('ComfyUI could not load this 3D output.');
            let disposed = false;
            const viewer = raw;
            /** Let Comfy's own activity policy render during pointer or keyboard interaction. */
            const active = () => {
                if (!disposed)
                    invoke(viewer, 'updateStatusMouseOnViewer', true);
            };
            const inactive = () => {
                if (!disposed)
                    invoke(viewer, 'updateStatusMouseOnViewer', false);
            };
            container.addEventListener('pointerenter', active);
            container.addEventListener('pointerleave', inactive);
            container.addEventListener('focusin', active);
            container.addEventListener('focusout', inactive);
            if (container.matches(':hover'))
                active();
            return {
                resize: () => {
                    if (disposed)
                        return;
                    const bounds = container.getBoundingClientRect();
                    if (bounds.width > 0 && bounds.height > 0)
                        invoke(viewer, 'setTargetSize', Math.round(bounds.width), Math.round(bounds.height));
                    else
                        invoke(viewer, 'handleResize');
                },
                capture: async () => {
                    const result = await invoke(viewer, 'captureThumbnail', 384, 384);
                    if (typeof result !== 'string' || !/^data:image\/(png|jpeg|webp);base64,/.test(result))
                        throw new Error('ComfyUI returned an invalid model preview.');
                    return result;
                },
                camera: () => (disposed ? null : invoke(viewer, 'getCameraState')),
                restore: (state) => {
                    if (!disposed && isRecord(state))
                        invoke(viewer, 'setCameraState', state);
                },
                dispose: () => {
                    if (!disposed) {
                        container.removeEventListener('pointerenter', active);
                        container.removeEventListener('pointerleave', inactive);
                        container.removeEventListener('focusin', active);
                        container.removeEventListener('focusout', inactive);
                        disposed = true;
                        invoke(viewer, 'remove');
                    }
                },
            };
        }
        catch (error) {
            if (raw)
                invoke(raw, 'remove');
            throw error;
        }
        finally {
            finish();
        }
    }
}
