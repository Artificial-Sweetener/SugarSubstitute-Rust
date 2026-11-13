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
/** Adapt the public Comfy generation settings and mounted action controls without a second queue. */
import { isRecord } from '../../types/common.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
/** Delegate generation policy and persistence to Comfy's existing command and settings owners. */
export class ComfyGenerationAdapter {
    document;
    #manager;
    #marked = new Set();
    #submissions = new Set();
    #listeners = new Set();
    #canceling = false;
    /** Narrow external store access before exposing facts to the strip. */
    constructor(document, app) {
        this.document = document;
        this.#manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
    }
    /** Project native availability while retaining the mounted controls for mode restoration. */
    read() {
        for (const element of this.#marked) {
            if (element.isConnected)
                continue;
            element.removeAttribute('data-sc-generation-native');
            this.#marked.delete(element);
        }
        const native = this.document.querySelector('.actionbar-container');
        const group = native?.querySelector('.queue-button-group');
        const run = group?.querySelector('button');
        const cancel = native?.querySelector('button[aria-label="Cancel current run"]');
        const batch = native?.querySelector('.batch-count');
        const settings = this.#settings();
        const ready = settings !== null && this.#commands() !== null;
        for (const element of ready ? [group, cancel, batch] : []) {
            if (!element)
                continue;
            element.setAttribute('data-sc-generation-native', '');
            this.#marked.add(element);
        }
        const count = settings?.batchCount;
        return {
            batch: typeof count === 'number' && Number.isFinite(count)
                ? Math.max(1, Math.min(999, Math.trunc(count)))
                : 1,
            generateEnabled: ready && !!run && !run.disabled && !this.#canceling,
            skipEnabled: ready && !!cancel && !cancel.disabled && !this.#canceling,
            cancelEnabled: ready && ((!!cancel && !cancel.disabled) || this.#submissions.size > 0) && !this.#canceling,
        };
    }
    /** Update the same batch count used by Comfy.QueuePrompt, independently of latent batch size. */
    setBatch(value) {
        const settings = this.#settings();
        if (!settings || !Number.isFinite(value))
            throw new Error('Comfy generation batch setting is unavailable or invalid.');
        settings.batchCount = Math.max(1, Math.min(999, Math.trunc(value)));
    }
    /** Submit through Comfy's normal generation command and execution hooks. */
    async generate() {
        if (!this.read().generateEnabled)
            return;
        const submission = this.#execute('Comfy.QueuePrompt');
        this.#submissions.add(submission);
        this.#notify();
        try {
            await submission;
        }
        finally {
            this.#submissions.delete(submission);
            this.#notify();
        }
    }
    /** Interrupt the active generation while leaving pending host jobs intact. */
    async skip() {
        if (!this.read().skipEnabled)
            return;
        await this.#execute('Comfy.Interrupt');
    }
    /** Stop automatic submission and pending work before interrupting the active host generation. */
    async cancel() {
        if (!this.read().cancelEnabled)
            return;
        this.#canceling = true;
        this.#notify();
        const settings = this.#settings();
        if (settings)
            settings.mode = 'disabled';
        try {
            // Comfy submits a batch asynchronously; clear only after its already-admitted requests finish.
            await Promise.allSettled([...this.#submissions]);
            await this.#execute('Comfy.ClearPendingTasks');
            await this.#execute('Comfy.Interrupt');
        }
        finally {
            this.#canceling = false;
            this.#notify();
        }
    }
    /** Observe only mounted native generation surfaces and the public batch settings store. */
    subscribe(changed) {
        this.#listeners.add(changed);
        const observer = new HostSurfaceObserver(this.document.body, '.actionbar-container', changed, {
            content: true,
            attributes: ['disabled'],
            exclude: '.sc-substitute-frame,.sc-generation-strip,.sc-plugin-toolbar-start',
        });
        const settings = this.#settings();
        const subscribe = settings?.$subscribe;
        const stop = typeof subscribe === 'function'
            ? subscribe.call(settings, changed, { detached: true, flush: 'post' })
            : null;
        return () => {
            this.#listeners.delete(changed);
            observer.disconnect();
            if (typeof stop === 'function')
                stop();
        };
    }
    /** Restore every native control, including surfaces retired during host renderer replacement. */
    dispose() {
        this.#listeners.clear();
        for (const element of this.#marked)
            element.removeAttribute('data-sc-generation-native');
        this.#marked.clear();
    }
    /** Reject malformed external settings rather than retaining a competing local batch count. */
    #settings() {
        const value = this.#manager?.queueSettings;
        return isRecord(value) && typeof value.batchCount === 'number' ? value : null;
    }
    /** Validate the host command port once per action so unavailable integration fails visibly. */
    #commands() {
        const value = this.#manager?.command;
        return isRecord(value) && typeof value.execute === 'function' ? value : null;
    }
    /** Publish only this boundary's in-flight command availability transitions. */
    #notify() {
        for (const listener of this.#listeners)
            listener();
    }
    /** Keep command failures intact for the shared SugarCubes feedback boundary. */
    async #execute(id) {
        const commands = this.#commands();
        if (!commands || typeof commands.execute !== 'function')
            throw new Error('Comfy generation commands are unavailable.');
        await commands.execute.call(commands, id);
    }
}
