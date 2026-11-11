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

import { isRecord, type UnknownRecord } from '../../types/common.js';

export interface ComfyAsyncMethodObserverOptions<Context> {
  readonly host: unknown;
  readonly method: 'loadGraphData' | 'graphToPrompt';
  readonly logger: Pick<Console, 'error'>;
  enter(args: readonly unknown[]): Context;
  succeeded(context: Context, result: unknown): void;
  failed(context: Context, error: unknown): void;
  cancelled(context: Context): void;
}

interface ObservedCall<Context> {
  readonly context: Context;
  active: boolean;
}
interface Installation<Context> {
  active: boolean;
  readonly original: unknown;
  readonly wrapper: (this: unknown, ...args: unknown[]) => Promise<unknown>;
  readonly pending: Set<ObservedCall<Context>>;
}

/** Keep each installation's original method alive for later extension wrappers and pending calls. */
export class ComfyAsyncMethodObserver<Context> {
  readonly #host: UnknownRecord;
  #installation: Installation<Context> | null = null;

  /** Validate the dynamic host object at its single infrastructure boundary. */
  constructor(private readonly options: ComfyAsyncMethodObserverOptions<Context>) {
    if (!isRecord(options.host)) throw new TypeError('Comfy application is unavailable.');
    this.#host = options.host;
  }

  /** Install one stable observer for this lifetime, preserving another extension's call chain. */
  install(): void {
    if (this.#installation) return;
    const original = this.#host[this.options.method];
    if (typeof original !== 'function')
      throw new TypeError(`Comfy ${this.options.method} is unavailable.`);
    const options = this.options;
    const observer = this;
    const pending = new Set<ObservedCall<Context>>();
    let active = true;
    const wrapper = async function (this: unknown, ...args: unknown[]): Promise<unknown> {
      if (!active) {
        const value: unknown = original.apply(this, args);
        return await value;
      }
      let call: ObservedCall<Context> | null = null;
      try {
        call = { context: options.enter(args), active: true };
        pending.add(call);
      } catch (error: unknown) {
        observer.#report('enter', error);
      }
      try {
        const value: unknown = original.apply(this, args);
        const result = await value;
        const observed = call;
        if (active && observed?.active)
          observer.#observe('succeeded', () => options.succeeded(observed.context, result));
        return result;
      } catch (error: unknown) {
        const observed = call;
        if (active && observed?.active)
          observer.#observe('failed', () => options.failed(observed.context, error));
        throw error;
      } finally {
        if (call) pending.delete(call);
      }
    };
    const installation: Installation<Context> = {
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
  dispose(): void {
    const installation = this.#installation;
    if (!installation) return;
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
  #observe(stage: string, observe: () => void): void {
    try {
      observe();
    } catch (error: unknown) {
      this.#report(stage, error);
    }
  }

  /** Preserve unexpected observation causes with actionable method/stage context. */
  #report(stage: string, error: unknown): void {
    this.options.logger.error('SugarCubes host method observation failed.', {
      method: this.options.method,
      stage,
      error,
    });
  }
}
