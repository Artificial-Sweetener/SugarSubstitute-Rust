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

export interface ComfyPromptQueueOptions {
  partialExecutionTargets?: unknown;
  previewMethod?: unknown;
}

type QueuePrompt = (
  position: number,
  payload: unknown,
  options?: ComfyPromptQueueOptions,
) => Promise<unknown>;

export interface ComfyPromptQueueApi {
  queuePrompt?: QueuePrompt;
}

/** Describe one captured submission without sharing mutable context between queue requests. */
export interface PromptQueueObservation {
  prepared(payload: unknown): void;
  accepted(response: unknown): void;
  rejected(error: unknown): void;
  cancelled(): void;
}

export interface PromptQueueObserver {
  readonly logger: Pick<Console, 'error'>;
  capture(payload: unknown): PromptQueueObservation | null;
}

export interface ComfyPromptQueueBridgeOptions {
  api: ComfyPromptQueueApi;
  preflight?(): void;
  prepare?(payload: unknown): unknown;
  transform(payload: unknown): Promise<unknown>;
  executeDirect?(
    position: number,
    payload: unknown,
    options?: ComfyPromptQueueOptions,
  ): Promise<unknown>;
  shouldExecuteDirect?(payload: unknown): boolean;
  observer?: PromptQueueObserver;
}

interface QueueInstallation {
  active: boolean;
  readonly original: QueuePrompt;
  readonly wrapper: QueuePrompt;
  readonly pending: Set<PromptQueueObservation>;
}

/** Own the sole SugarCubes host queue interaction across workflow lifecycles. */
export class ComfyPromptQueueBridge {
  readonly #api: ComfyPromptQueueApi;
  readonly #preflight: () => void;
  readonly #transform: (payload: unknown) => Promise<unknown>;
  readonly #prepare: (payload: unknown) => unknown;
  readonly #executeDirect:
    | ((position: number, payload: unknown, options?: ComfyPromptQueueOptions) => Promise<unknown>)
    | null;
  readonly #shouldExecuteDirect: (payload: unknown) => boolean;
  readonly #observer: PromptQueueObserver | undefined;
  #installation: QueueInstallation | null = null;

  /** Bind one stable host API and one application-level prompt pipeline. */
  constructor(options: ComfyPromptQueueBridgeOptions) {
    this.#api = options.api;
    this.#preflight = options.preflight ?? (() => undefined);
    this.#transform = options.transform;
    this.#prepare = options.prepare ?? ((payload) => payload);
    this.#executeDirect = options.executeDirect ?? null;
    this.#shouldExecuteDirect = options.shouldExecuteDirect ?? (() => true);
    this.#observer = options.observer;
  }

  /** Install the bridge exactly once. */
  install(): void {
    if (this.#installation) return;
    const original = this.#api.queuePrompt;
    if (!original) throw new TypeError('Comfy queuePrompt is unavailable.');
    const installation: QueueInstallation = {
      active: true,
      original,
      pending: new Set(),
      wrapper: async (position, payload, options) => {
        if (!installation.active) return await original.call(this.#api, position, payload, options);
        let observation = this.#capture(payload);
        if (observation) installation.pending.add(observation);
        try {
          const direct = Boolean(this.#executeDirect && this.#shouldExecuteDirect(payload));
          if (direct) this.#preflight();
          payload = this.#prepare(payload);
          if (!direct) payload = await this.#transform(payload);
          if (
            installation.active &&
            observation &&
            !this.#observe('prepared', () => observation?.prepared(payload))
          ) {
            this.#observe('cancelled', () => observation?.cancelled());
            installation.pending.delete(observation);
            observation = null;
          }
          const result =
            direct && this.#executeDirect
              ? await this.#executeDirect(position, payload, options)
              : await original.call(this.#api, position, payload, options);
          if (installation.active && observation)
            this.#observe('accepted', () => observation?.accepted(result));
          return result;
        } catch (error: unknown) {
          if (installation.active && observation)
            this.#observe('rejected', () => observation?.rejected(error));
          throw error;
        } finally {
          if (observation) installation.pending.delete(observation);
        }
      },
    };
    this.#api.queuePrompt = installation.wrapper;
    this.#installation = installation;
  }

  /** Restore the host queue only while this bridge remains its current owner. */
  dispose(): void {
    const installation = this.#installation;
    if (!installation) return;
    installation.active = false;
    if (this.#api.queuePrompt === installation.wrapper)
      this.#api.queuePrompt = installation.original;
    for (const observation of installation.pending)
      this.#observe('cancelled', () => observation.cancelled());
    installation.pending.clear();
    this.#installation = null;
  }

  /** Capture before the asynchronous pipeline without allowing metadata to fail execution. */
  #capture(payload: unknown): PromptQueueObservation | null {
    try {
      return this.#observer?.capture(payload) ?? null;
    } catch (error: unknown) {
      this.#report('capture', error);
      return null;
    }
  }

  /** Contain diagnostic failures while retaining the original host result or exception. */
  #observe(stage: string, observe: () => void): boolean {
    try {
      observe();
      return true;
    } catch (error: unknown) {
      this.#report(stage, error);
      return false;
    }
  }

  /** Report original observation causes with the precise submission stage. */
  #report(stage: string, error: unknown): void {
    this.#observer?.logger.error('SugarCubes prompt submission observation failed.', {
      stage,
      error,
    });
  }
}
