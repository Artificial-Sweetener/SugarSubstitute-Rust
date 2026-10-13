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
/** Install one stable SugarCubes boundary around Comfy's prompt queue. */
/** Own the sole SugarCubes host queue interaction across workflow lifecycles. */
export class ComfyPromptQueueBridge {
    #api;
    #preflight;
    #transform;
    #prepare;
    #executeDirect;
    #shouldExecuteDirect;
    #observer;
    #installation = null;
    /** Bind one stable host API and one application-level prompt pipeline. */
    constructor(options) {
        this.#api = options.api;
        this.#preflight = options.preflight ?? (() => undefined);
        this.#transform = options.transform;
        this.#prepare = options.prepare ?? ((payload) => payload);
        this.#executeDirect = options.executeDirect ?? null;
        this.#shouldExecuteDirect = options.shouldExecuteDirect ?? (() => true);
        this.#observer = options.observer;
    }
    /** Install the bridge exactly once. */
    install() {
        if (this.#installation)
            return;
        const original = this.#api.queuePrompt;
        if (!original)
            throw new TypeError('Comfy queuePrompt is unavailable.');
        const installation = {
            active: true,
            original,
            pending: new Set(),
            wrapper: async (position, payload, options) => {
                if (!installation.active)
                    return await original.call(this.#api, position, payload, options);
                let observation = this.#capture(payload);
                if (observation)
                    installation.pending.add(observation);
                try {
                    const direct = Boolean(this.#executeDirect && this.#shouldExecuteDirect(payload));
                    if (direct)
                        this.#preflight();
                    payload = this.#prepare(payload);
                    if (!direct)
                        payload = await this.#transform(payload);
                    if (installation.active &&
                        observation &&
                        !this.#observe('prepared', () => observation?.prepared(payload))) {
                        this.#observe('cancelled', () => observation?.cancelled());
                        installation.pending.delete(observation);
                        observation = null;
                    }
                    const result = direct && this.#executeDirect
                        ? await this.#executeDirect(position, payload, options, (prepared) => original.call(this.#api, position, prepared, options))
                        : await original.call(this.#api, position, payload, options);
                    if (installation.active && observation)
                        this.#observe('accepted', () => observation?.accepted(result));
                    return result;
                }
                catch (error) {
                    if (installation.active && observation)
                        this.#observe('rejected', () => observation?.rejected(error));
                    throw error;
                }
                finally {
                    if (observation)
                        installation.pending.delete(observation);
                }
            },
        };
        this.#api.queuePrompt = installation.wrapper;
        this.#installation = installation;
    }
    /** Restore the host queue only while this bridge remains its current owner. */
    dispose() {
        const installation = this.#installation;
        if (!installation)
            return;
        installation.active = false;
        if (this.#api.queuePrompt === installation.wrapper)
            this.#api.queuePrompt = installation.original;
        for (const observation of installation.pending)
            this.#observe('cancelled', () => observation.cancelled());
        installation.pending.clear();
        this.#installation = null;
    }
    /** Capture before the asynchronous pipeline without allowing metadata to fail execution. */
    #capture(payload) {
        try {
            return this.#observer?.capture(payload) ?? null;
        }
        catch (error) {
            this.#report('capture', error);
            return null;
        }
    }
    /** Contain diagnostic failures while retaining the original host result or exception. */
    #observe(stage, observe) {
        try {
            observe();
            return true;
        }
        catch (error) {
            this.#report(stage, error);
            return false;
        }
    }
    /** Report original observation causes with the precise submission stage. */
    #report(stage, error) {
        this.#observer?.logger.error('SugarCubes prompt submission observation failed.', {
            stage,
            error,
        });
    }
}
