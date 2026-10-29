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
/** Hand an existing view's selected result to a negotiated native surface. */
import { sendNativeCanvasOperation, releaseNativeCanvasBinding, } from './NativeCanvasBinding.js';
/** Keep one in-flight operation and coalesce newer view or geometry changes. */
export class NativeCanvasPresentation {
    options;
    #binding = null;
    #viewport = null;
    #desired = null;
    #running = false;
    #dirty = false;
    #visible = false;
    #disposed = false;
    #retirement = null;
    /** Bind native presentation to the existing qualified view address. */
    constructor(options) {
        this.options = options;
    }
    /** Reconcile only the selection and geometry already chosen by the view owners. */
    update(geometry, item) {
        if (this.#disposed)
            return;
        const preparing = this.options.acceptsPreparation();
        const host = this.options.accepts() || preparing ? this.options.host() : null;
        if (host !== (this.#binding?.host ?? null)) {
            void this.#retire();
            if (host) {
                this.#binding = {
                    host,
                    revision: 0,
                    prepared: false,
                    committed: false,
                    attempted: false,
                    geometry: null,
                    lastGeometry: null,
                    selection: null,
                    viewportKey: null,
                    active: null,
                };
            }
            this.#desired = null;
        }
        const eligible = geometry &&
            geometry.viewport.width > 0 &&
            geometry.viewport.height > 0 &&
            (preparing || (geometry.visible && geometry.clip.width > 0 && geometry.clip.height > 0)) &&
            item?.mediaKind === 'image'
            ? item
            : null;
        const key = JSON.stringify([
            geometry && { ...geometry, focused: false },
            item && [item.id, item.file, item.mediaKind],
            this.options.accepts(),
        ]);
        if (this.#desired?.key === key) {
            if (this.#desired.geometry?.focused === geometry?.focused)
                return;
            this.#desired.geometry = geometry;
            this.#dirty = true;
            this.#start();
            return;
        }
        this.#desired = { geometry, item: eligible, key, empty: item?.mediaKind !== 'image' };
        this.options.viewport(null);
        if (this.#viewport)
            this.#cancelViewport(this.#viewport);
        // A previous allocation can no longer be presented after the native window resizes.
        // Cancel its acknowledgement wait so the current geometry can reach the host.
        if (this.#binding?.active?.dependsOnView)
            this.#binding.active.cancellation.cancel();
        // The native owner retains acknowledged pixels while a same-allocation replacement loads.
        if (!eligible ||
            !this.#binding?.committed ||
            this.#binding.geometry !== JSON.stringify(geometry))
            this.#show(false);
        this.options.status(eligible
            ? host
                ? { state: 'loading', message: 'Loading the selected image.' }
                : { state: 'unavailable', message: 'The native canvas is unavailable.' }
            : { state: 'empty', message: '' });
        this.#dirty = true;
        this.#start();
    }
    /** Admit one discrete intent into the existing presentation stream, replacing only older intent. */
    viewport(command, scope) {
        const binding = this.#binding;
        if (!binding ||
            !this.#desired?.item ||
            !binding.host.capabilities.viewportState ||
            !scope.current() ||
            command.type === 'read' ||
            !binding.host.capabilities.viewportCommands.includes(command.type))
            return Promise.resolve(false);
        const previous = this.#viewport;
        if (previous)
            this.#cancelViewport(previous);
        return new Promise((resolve) => {
            let complete = false;
            const intent = {
                command,
                scope,
                running: false,
                complete: (applied) => {
                    if (complete)
                        return;
                    complete = true;
                    scope.signal.removeEventListener('abort', cancel);
                    if (this.#viewport === intent)
                        this.#viewport = null;
                    resolve(applied);
                },
            };
            const cancel = () => this.#cancelViewport(intent);
            this.#viewport = intent;
            scope.signal.addEventListener('abort', cancel, { once: true });
            if (!scope.current()) {
                intent.complete(false);
                return;
            }
            this.#dirty = true;
            this.#start();
        });
    }
    /** Keep a forwarded operation awaiting native acknowledgement; withdraw unsent intent locally. */
    #cancelViewport(intent) {
        if (intent.running)
            this.#binding?.active?.cancellation.cancel();
        else
            intent.complete(false);
    }
    /** Revoke the exact surface before releasing its transport and late completions. */
    dispose() {
        if (this.#disposed)
            return this.#retirement ?? Promise.resolve();
        this.#disposed = true;
        this.#desired = null;
        return this.#retire();
    }
    #start() {
        if (this.#running || this.#retirement || !this.#binding || !this.#desired || this.#disposed)
            return;
        this.#running = true;
        void this.#run().finally(() => {
            this.#running = false;
            if (this.#dirty && !this.#disposed)
                this.#start();
        });
    }
    async #run() {
        while (this.#dirty && !this.#disposed) {
            this.#dirty = false;
            const binding = this.#binding;
            const desired = this.#desired;
            if (!binding || !desired || !this.#owns(binding))
                return;
            try {
                if ((desired.item || (this.options.acceptsPreparation() && desired.empty)) &&
                    desired.geometry &&
                    !binding.prepared) {
                    const result = await this.#send(binding, { type: 'surface.prepare' });
                    if (!this.#owns(binding))
                        return;
                    if (result.status !== 'ready') {
                        this.#failed(desired, result);
                        continue;
                    }
                    binding.prepared = true;
                }
                if (!this.#current(binding, desired))
                    continue;
                const lastGeometry = desired.geometry ?? binding.lastGeometry;
                if (!binding.prepared || !lastGeometry)
                    continue;
                const geometry = desired.item
                    ? lastGeometry
                    : { ...lastGeometry, visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } };
                const geometryKey = JSON.stringify(geometry);
                if (geometryKey !== binding.geometry) {
                    const result = await this.#send(binding, { type: 'surface.geometry', geometry });
                    if (!this.#owns(binding))
                        return;
                    if (result.status !== 'applied') {
                        this.#failed(desired, result);
                        continue;
                    }
                    binding.geometry = geometryKey;
                    binding.lastGeometry = geometry;
                }
                if (!this.#current(binding, desired))
                    continue;
                if (!desired.item) {
                    if (desired.empty)
                        this.options.prepared();
                    continue;
                }
                const selection = JSON.stringify([desired.item.id, desired.item.file]);
                if (selection !== binding.selection) {
                    const result = await this.#send(binding, {
                        type: 'surface.select',
                        itemId: desired.item.id,
                        file: desired.item.file,
                    });
                    if (!this.#owns(binding))
                        return;
                    if (result.status !== 'applied') {
                        this.#failed(desired, result);
                        continue;
                    }
                    binding.selection = selection;
                }
                if (!this.#current(binding, desired))
                    continue;
                this.options.prepared();
                if (!this.options.accepts())
                    continue;
                if (!binding.committed) {
                    const result = await this.#send(binding, { type: 'surface.commit' });
                    if (!this.#owns(binding))
                        return;
                    if (result.status !== 'applied') {
                        this.#failed(desired, result);
                        continue;
                    }
                    binding.committed = true;
                }
                if (!this.#current(binding, desired))
                    continue;
                const intent = this.#viewport;
                if (intent && !intent.scope.current())
                    intent.complete(false);
                const admitted = this.#viewport;
                if (admitted ||
                    (binding.host.capabilities.viewportState && binding.viewportKey !== desired.key)) {
                    if (admitted) {
                        admitted.running = true;
                        binding.viewportKey = null;
                        this.options.viewport(null);
                    }
                    try {
                        const result = await this.#send(binding, {
                            type: 'surface.viewport.request',
                            command: admitted?.command ?? { type: 'read' },
                        });
                        const current = this.#current(binding, desired) &&
                            (!admitted || (this.#viewport === admitted && admitted.scope.current()));
                        if (current && result.status === 'applied' && 'viewport' in result) {
                            binding.viewportKey = desired.key;
                            this.options.viewport(result.viewport);
                            admitted?.complete(true);
                        }
                        else {
                            const recoverUnavailable = admitted !== null && current && result.status === 'unavailable';
                            admitted?.complete(false);
                            if (admitted &&
                                this.#current(binding, desired) &&
                                (!current || result.status === 'cancelled' || recoverUnavailable))
                                this.#dirty = true;
                            if (current && result.status !== 'cancelled' && !recoverUnavailable)
                                this.#failed(desired, result);
                            continue;
                        }
                    }
                    catch (error) {
                        if (this.#current(binding, desired) &&
                            (!admitted || (this.#viewport === admitted && admitted.scope.current())))
                            throw error;
                        this.options.report(error);
                        continue;
                    }
                    finally {
                        admitted?.complete(false);
                    }
                }
                if (this.#current(binding, desired)) {
                    this.#show(true);
                    this.options.status({ state: 'ready', message: '' });
                }
            }
            catch (error) {
                if (this.#current(binding, desired)) {
                    this.#failedIntent();
                    this.options.status({ state: 'error', message: 'Native canvas presentation failed.' });
                }
                this.options.report(error);
            }
        }
    }
    /** Delegate through the same binding record for every operation. */
    #send(binding, operation) {
        return sendNativeCanvasOperation(binding, this.options.address, operation);
    }
    #owns(binding) {
        return (!this.#disposed &&
            this.#binding === binding &&
            (this.options.accepts() || this.options.acceptsPreparation()));
    }
    #current(binding, desired) {
        return this.#owns(binding) && this.#desired === desired;
    }
    #failed(desired, result) {
        if (this.#desired !== desired)
            return;
        this.#failedIntent();
        // A staged sibling temporarily owns native mutation. Its release explicitly refreshes
        // the retained attachment; permit that refresh to reconcile previously rejected geometry.
        if (result.status === 'stale')
            this.#desired = null;
        const error = result.status === 'error';
        this.options.status({
            state: error ? 'error' : 'unavailable',
            message: result.message ||
                (error ? 'Native canvas presentation failed.' : 'The native canvas is unavailable.'),
        });
        if (error)
            this.options.report(new Error(result.message || 'Native canvas presentation failed.'));
    }
    /** Settle unsent intent when its required presentation fails, without retrying that presentation. */
    #failedIntent() {
        this.#dirty = false;
        this.options.viewport(null);
        if (this.#viewport && !this.#viewport.running)
            this.#viewport.complete(false);
    }
    #show(visible) {
        if (visible === this.#visible)
            return;
        this.#visible = visible;
        this.options.visible(visible);
    }
    /** Observe the host's release acknowledgement before allowing another binding to prepare. */
    #retire() {
        this.#show(false);
        this.options.viewport(null);
        if (this.#viewport)
            this.#cancelViewport(this.#viewport);
        const binding = this.#binding;
        this.#binding = null;
        if (!binding)
            return this.#retirement ?? Promise.resolve();
        binding.active?.cancellation.cancel();
        if (!binding.attempted)
            return this.#retirement ?? Promise.resolve();
        const release = releaseNativeCanvasBinding(binding, this.options.address);
        const retirement = this.#retirement
            ? Promise.all([this.#retirement, release]).then(() => { })
            : release;
        this.#retirement = retirement;
        void retirement.then(() => {
            if (this.#retirement === retirement)
                this.#retirement = null;
            this.#start();
        }, this.options.report);
        return retirement;
    }
}
