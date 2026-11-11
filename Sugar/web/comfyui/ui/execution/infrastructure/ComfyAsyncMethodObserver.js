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
/** Observe public asynchronous host methods without changing their call or error contracts. */
import { isRecord } from '../../types/common.js';
/** Keep each installation's original method alive for later extension wrappers and pending calls. */
export class ComfyAsyncMethodObserver {
    options;
    #host;
    #installation = null;
    /** Validate the dynamic host object at its single infrastructure boundary. */
    constructor(options) {
        this.options = options;
        if (!isRecord(options.host))
            throw new TypeError('Comfy application is unavailable.');
        this.#host = options.host;
    }
    /** Install one stable observer for this lifetime, preserving another extension's call chain. */
    install() {
        if (this.#installation)
            return;
        const original = this.#host[this.options.method];
        if (typeof original !== 'function')
            throw new TypeError(`Comfy ${this.options.method} is unavailable.`);
        const options = this.options;
        const observer = this;
        const pending = new Set();
        let active = true;
        const wrapper = async function (...args) {
            if (!active) {
                const value = original.apply(this, args);
                return await value;
            }
            let call = null;
            try {
                call = { context: options.enter(args), active: true };
                pending.add(call);
            }
            catch (error) {
                observer.#report('enter', error);
            }
            try {
                const value = original.apply(this, args);
                const result = await value;
                const observed = call;
                if (active && observed?.active)
                    observer.#observe('succeeded', () => options.succeeded(observed.context, result));
                return result;
            }
            catch (error) {
                const observed = call;
                if (active && observed?.active)
                    observer.#observe('failed', () => options.failed(observed.context, error));
                throw error;
            }
            finally {
                if (call)
                    pending.delete(call);
            }
        };
        const installation = {
            get active() {
                return active;
            },
            set active(value) {
                active = value;
            },
            original,
            wrapper,
            pending,
        };
        this.#host[options.method] = wrapper;
        this.#installation = installation;
    }
    /** Retire observation while preserving calls delegated through another extension's retained wrapper. */
    dispose() {
        const installation = this.#installation;
        if (!installation)
            return;
        installation.active = false;
        if (this.#host[this.options.method] === installation.wrapper)
            this.#host[this.options.method] = installation.original;
        for (const call of installation.pending) {
            call.active = false;
            this.#observe('cancelled', () => this.options.cancelled(call.context));
        }
        installation.pending.clear();
        this.#installation = null;
    }
    /** Contain metadata failures so they cannot fail a valid host operation. */
    #observe(stage, observe) {
        try {
            observe();
        }
        catch (error) {
            this.#report(stage, error);
        }
    }
    /** Preserve unexpected observation causes with actionable method/stage context. */
    #report(stage, error) {
        this.options.logger.error('SugarCubes host method observation failed.', {
            method: this.options.method,
            stage,
            error,
        });
    }
}
