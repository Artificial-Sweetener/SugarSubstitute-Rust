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
import { sendNativeCanvasOperation, releaseNativeCanvasBinding, createNativeCanvasBinding, } from './NativeCanvasBinding.js';
import { IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS, NativeCanvasPreview, } from './NativeCanvasPreview.js';
import { NativeCanvasViewportIntent } from './NativeCanvasViewportIntent.js';
import { nativeCanvasComparison } from './NativeCanvasCapabilities.js';
import { nativeCanvasDesired, nativeCanvasNavigable, nativeCanvasOverviewKey, nativeCanvasSelection, nativeCanvasWipe, nativeCanvasWipeKey, } from './NativeCanvasDesired.js';
import { NATIVE_CANVAS_PRESENTATION_FAILURE, NATIVE_CANVAS_RESULT_FAILURE, nativeCanvasFailureStatus, nativeCanvasPendingStatus, nativeCanvasSettledStatus, } from './NativeCanvasStatus.js';
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
    #preview = null;
    #viewportRecovery = null;
    /** Bind native presentation to the existing qualified view address. */
    constructor(options) {
        this.options = options;
    }
    /**
     * Reconcile only the selection, comparison, overview and geometry chosen by the view owners.
     * A divider-only comparison change keeps the presented pair and its camera; the newest
     * divider follows any in-flight placement rather than cancelling it. Overview layout changes
     * likewise follow the overview request in flight. A geometry-only change of shown content keeps
     * the surface shown with its status unchanged; only content without committed pixels loads.
     */
    update(geometry, item, preview = null, comparison = null, overview = null) {
        if (this.#disposed)
            return;
        this.#viewportRecovery = null;
        const preparing = this.options.acceptsPreparation();
        const host = this.options.accepts() || preparing ? this.options.host() : null;
        if (host !== (this.#binding?.host ?? null)) {
            void this.#retire();
            if (host) {
                this.#binding = createNativeCanvasBinding(host);
                const binding = this.#binding;
                this.#preview = new NativeCanvasPreview({
                    address: this.options.address,
                    host,
                    clock: this.options.clock,
                    active: () => this.#owns(binding) && this.options.accepts(),
                    finalRevision: () => binding.selectionRevision,
                    changed: () => this.#wake(),
                    cancelOperation: () => {
                        if (binding.active?.type.startsWith('surface.preview.'))
                            binding.active.cancellation.cancel();
                    },
                    displayed: (value, presentation) => this.#observed(binding, value, presentation),
                    report: this.options.report,
                });
            }
            this.#desired = null;
        }
        const previewChanged = this.#preview?.update(preview) ?? false;
        const desired = nativeCanvasDesired(geometry, item, nativeCanvasComparison(host) ? comparison : null, host?.capabilities.overview ? overview : null, this.options.accepts(), preparing);
        if (this.#desired?.key === desired.key) {
            // Focus and gesture occlusions change routing, never the presented allocation.
            const routing = JSON.stringify(this.#desired.geometry) !== JSON.stringify(geometry);
            const divider = nativeCanvasWipeKey(this.#desired.comparison) !== nativeCanvasWipeKey(desired.comparison);
            const layout = nativeCanvasOverviewKey(this.#desired.overview) !==
                nativeCanvasOverviewKey(desired.overview);
            if (routing)
                this.#desired.geometry = geometry;
            this.#desired.comparison = desired.comparison;
            this.#desired.overview = desired.overview;
            if (routing || divider || layout || previewChanged)
                this.#wake();
            return;
        }
        const retainedFinal = desired.previewable &&
            this.#preview?.observing &&
            this.#binding?.selectionRevision !== null &&
            this.#binding?.geometry === JSON.stringify(geometry);
        // Shown content whose allocation alone changes keeps its committed pixels: the host
        // re-presents them at the new viewport, so the surface stays shown and ready meanwhile.
        const resized = this.#visible &&
            this.#desired?.content === desired.content &&
            this.#binding?.committed === true &&
            desired.presentable &&
            (desired.shown || (desired.previewable && this.#preview?.observing === true));
        this.#desired = desired;
        if (!retainedFinal)
            this.options.viewport(null);
        if (this.#viewport && !retainedFinal)
            this.#cancelViewport(this.#viewport);
        // A previous allocation can no longer be presented after the native window resizes.
        // Cancel its acknowledgement wait so the current geometry can reach the host.
        if (this.#binding?.active?.dependsOnView && !retainedFinal)
            this.#binding.active.cancellation.cancel();
        if (resized) {
            this.#wake();
            return;
        }
        // The native owner retains acknowledged pixels while a same-allocation replacement loads.
        if ((!desired.shown && !(desired.previewable && this.#preview?.observing)) ||
            !this.#binding?.committed ||
            this.#binding.geometry !== JSON.stringify(geometry))
            this.#show(false);
        this.options.status(nativeCanvasPendingStatus(desired, host !== null, () => !!this.options.surfacePending?.()));
        this.#wake();
    }
    /** Refuse preview bytes until a committed, observed binding owns the logical interest. */
    wants() {
        return this.#preview?.wants() ?? false;
    }
    /** Forward immutable encoded bytes without constructing a second renderer or mutation queue. */
    receive(preview, blob) {
        this.#preview?.receive(preview, blob);
    }
    /** Expose only local encoded custody; native decoded pixels remain host-owned. */
    diagnostics() {
        return this.#preview?.diagnostics() ?? IDLE_NATIVE_CANVAS_PREVIEW_DIAGNOSTICS;
    }
    /** Report implemented live preview support independently of whether pixels have arrived. */
    get previewAvailable() {
        return this.#preview?.observing ?? false;
    }
    /** Admit one discrete intent into the existing presentation stream, replacing only older intent. */
    viewport(command, scope) {
        const binding = this.#binding;
        if (!binding ||
            !(this.options.inputShown?.() ||
                nativeCanvasNavigable(this.#desired, this.#preview, binding.selectionRevision)) ||
            !binding.host.capabilities.viewportState ||
            !scope.current() ||
            command.type === 'read' ||
            !binding.host.capabilities.viewportCommands.includes(command.type))
            return Promise.resolve(false);
        const previous = this.#viewport;
        if (previous)
            this.#cancelViewport(previous);
        const intent = new NativeCanvasViewportIntent(command, scope, (cancelled) => this.#cancelViewport(cancelled), (finished) => {
            if (this.#viewport === finished)
                this.#viewport = null;
        });
        this.#viewport = intent;
        intent.observe();
        if (this.#viewport === intent)
            this.#wake();
        return intent.result;
    }
    /**
     * Publish a host-settled gesture camera like a commanded one, only while its selection is the
     * bound one and the shown camera already belongs to the desired presentation; the caller has
     * matched the attachment address. A stale or superseded camera is ignored.
     */
    settled(selectionRevision, viewport) {
        const binding = this.#binding;
        if (binding?.selectionRevision === selectionRevision &&
            binding.viewportKey === this.#desired?.key &&
            this.#owns(binding))
            this.options.viewport(viewport);
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
    /** Mark the desired state changed and resume the operation stream if it is idle. */
    #wake() {
        this.#dirty = true;
        this.#start();
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
    /**
     * Converge the bound surface on the newest desired state, one native mutation at a time.
     * Each pass prepares, places, selects and commits in order, then places a comparison divider,
     * serves viewport intent and finally one preview transfer step; a change during any await
     * marks the run dirty again. A newer divider never cancels an in-flight placement, so one
     * placement is in flight and only the newest value waits behind it.
     */
    async #run() {
        while (this.#dirty && !this.#disposed) {
            this.#dirty = false;
            const binding = this.#binding;
            const desired = this.#desired;
            if (!binding || !desired || !this.#owns(binding))
                return;
            try {
                if ((desired.shown ||
                    (desired.empty &&
                        (this.options.acceptsPreparation() ||
                            (desired.previewable && this.#preview?.observing)))) &&
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
                const geometry = desired.shown || (desired.previewable && this.#preview?.observing)
                    ? lastGeometry
                    : { ...lastGeometry, visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } };
                const geometryKey = JSON.stringify(geometry);
                if (geometryKey !== binding.geometry) {
                    const result = await this.#send(binding, { type: 'surface.geometry', geometry });
                    if (!this.#owns(binding))
                        return;
                    if (result.status !== 'applied') {
                        // Pixels kept shown through a resize cannot present at a rejected allocation.
                        if (this.#current(binding, desired))
                            this.#show(false);
                        this.#failed(desired, result);
                        continue;
                    }
                    binding.geometry = geometryKey;
                    binding.lastGeometry = geometry;
                }
                if (!this.#current(binding, desired))
                    continue;
                if (!desired.shown) {
                    if (!desired.previewable || !this.#preview?.observing) {
                        if (desired.empty)
                            this.options.prepared();
                        continue;
                    }
                }
                const selection = nativeCanvasSelection(desired, this.options.inspection);
                if (selection && selection.key !== binding.selection) {
                    const preview = this.#preview;
                    preview?.selecting();
                    let selectionRevision = 0;
                    const result = await this.#send(binding, selection.operation, (revision) => {
                        selectionRevision = revision;
                    }).finally(() => preview?.selectionFinished());
                    if (!this.#owns(binding))
                        return;
                    if (result.status === 'error') {
                        // The host could not present this result, e.g. an undecodable file. The prepared
                        // surface stays usable: complete readiness, hide stale pixels, show the load error.
                        this.options.prepared();
                        this.#show(false);
                        this.#failed(desired, result, NATIVE_CANVAS_RESULT_FAILURE);
                        continue;
                    }
                    if (result.status !== 'applied') {
                        this.#failed(desired, result);
                        continue;
                    }
                    binding.selection = selection.key;
                    binding.selectionRevision = selectionRevision;
                    preview?.selected();
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
                const wipe = nativeCanvasWipe(desired);
                if (wipe && wipe.key !== binding.wipe) {
                    const result = await this.#send(binding, {
                        type: 'surface.viewport.request',
                        command: wipe.command,
                    });
                    if (!this.#owns(binding))
                        return;
                    const current = this.#current(binding, desired);
                    if (result.status === 'applied' && 'viewport' in result) {
                        // The host placed this divider even when a newer one is now pending behind it.
                        binding.wipe = wipe.key;
                        if (current) {
                            binding.viewportKey = desired.key;
                            this.options.viewport(result.viewport);
                        }
                    }
                    else if (result.status === 'cancelled')
                        this.#dirty ||= current;
                    else
                        this.#failed(desired, result);
                    if (!current || binding.wipe !== wipe.key)
                        continue;
                }
                const intent = this.#viewport;
                if (intent && !intent.scope.current())
                    intent.complete(false);
                const admitted = this.#viewport;
                if (admitted ||
                    ((desired.item ||
                        (desired.previewable &&
                            this.#preview?.observing &&
                            binding.selectionRevision !== null)) &&
                        binding.host.capabilities.viewportState &&
                        binding.viewportKey !== desired.key)) {
                    if (admitted) {
                        admitted.running = true;
                        binding.viewportKey = null;
                        this.options.viewport(null);
                    }
                    const displayEpoch = this.#preview?.epoch;
                    try {
                        const result = await this.#send(binding, {
                            type: 'surface.viewport.request',
                            command: admitted?.command ?? { type: 'read' },
                        });
                        const current = this.#current(binding, desired) &&
                            displayEpoch === this.#preview?.epoch &&
                            (!admitted || (this.#viewport === admitted && admitted.scope.current()));
                        if (current && result.status === 'applied' && 'viewport' in result) {
                            binding.viewportKey = desired.key;
                            this.options.viewport(result.viewport);
                            admitted?.complete(true);
                        }
                        else {
                            const recoverUnavailable = admitted !== null && current && result.status === 'unavailable';
                            const presentation = this.#preview?.pendingPresentation;
                            if (current && result.status === 'stale' && presentation)
                                this.#viewportRecovery = { binding, desired, presentation };
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
                            displayEpoch === this.#preview?.epoch &&
                            (!admitted || (this.#viewport === admitted && admitted.scope.current())))
                            throw error;
                        this.options.report(error);
                        continue;
                    }
                    finally {
                        admitted?.complete(false);
                    }
                }
                const previewStep = this.#preview?.next();
                if (previewStep) {
                    try {
                        const result = await this.#send(binding, previewStep.operation, previewStep.started);
                        if (!this.#owns(binding))
                            return;
                        previewStep.settled(result);
                    }
                    catch (error) {
                        previewStep.failed(error);
                    }
                    this.#dirty = true;
                    continue;
                }
                if (this.#current(binding, desired)) {
                    this.#show(true);
                    this.options.status(nativeCanvasSettledStatus(desired, this.#preview, binding.selectionRevision));
                }
            }
            catch (error) {
                if (this.#current(binding, desired)) {
                    this.#failedIntent();
                    this.options.status({ state: 'error', message: NATIVE_CANVAS_PRESENTATION_FAILURE });
                }
                this.options.report(error);
            }
        }
    }
    /** Delegate through the same binding record for every operation. */
    #send(binding, operation, assigned) {
        return sendNativeCanvasOperation(binding, this.options.address, operation, assigned);
    }
    /** Let a native display transition supersede any still-pending final-camera acknowledgement. */
    #observed(binding, display, presentation) {
        if (!this.#owns(binding))
            return;
        const recovery = this.#viewportRecovery;
        if (display &&
            this.#desired === null &&
            recovery?.binding === binding &&
            recovery.presentation === presentation) {
            this.#desired = recovery.desired;
            this.#viewportRecovery = null;
        }
        if (binding.active?.type === 'surface.viewport.request')
            binding.active.cancellation.cancel();
        if (this.#viewport) {
            this.#cancelViewport(this.#viewport);
            this.#viewport.complete(false);
        }
        this.options.preview(display?.type === 'preview');
        this.options.viewport(display && 'viewport' in display ? display.viewport : null);
        if (display) {
            binding.viewportKey = this.#desired?.key ?? null;
            this.#show(binding.committed && this.#desired?.geometry?.visible === true);
            this.options.status({ state: display.type === 'empty' ? 'empty' : 'ready', message: '' });
        }
    }
    #owns(binding) {
        return (!this.#disposed &&
            this.#binding === binding &&
            (this.options.accepts() || this.options.acceptsPreparation()));
    }
    #current(binding, desired) {
        return this.#owns(binding) && this.#desired === desired;
    }
    /** Publish a failed operation, using `message` instead of the host's when it names the result. */
    #failed(desired, result, message) {
        if (this.#desired !== desired)
            return;
        this.#failedIntent();
        // A staged sibling temporarily owns native mutation. Its release explicitly refreshes
        // the retained attachment; permit that refresh to reconcile previously rejected geometry.
        if (result.status === 'stale')
            this.#desired = null;
        this.options.status(nativeCanvasFailureStatus(result, message));
        if (result.status === 'error')
            this.options.report(new Error(result.message || NATIVE_CANVAS_PRESENTATION_FAILURE));
    }
    /** Settle unsent intent when its required presentation fails, without retrying that presentation. */
    #failedIntent() {
        this.#dirty = false;
        if (!this.#preview?.displayed)
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
        this.#viewportRecovery = null;
        this.#show(false);
        this.options.viewport(null);
        this.options.preview(false);
        this.#preview?.dispose();
        this.#preview = null;
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
