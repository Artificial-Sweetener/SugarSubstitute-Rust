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
import { isRecord, type UnknownRecord } from '../../types/common.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';

/** What pressing Generate does: queue the batch once, or keep queueing it until stopped. */
export type GenerationMode = 'generate' | 'continuous';

export interface GenerationStripState {
  readonly batch: number;
  readonly generateEnabled: boolean;
  readonly skipEnabled: boolean;
  readonly cancelEnabled: boolean;
  readonly mode: GenerationMode;
  /** Continuous generation is queueing the batch again whenever the queue empties. */
  readonly looping: boolean;
}

/** ComfyUI's run modes that queue the batch again once the queue is empty. */
const INSTANT_IDLE = 'instant-idle';
const INSTANT_RUNNING = 'instant-running';

/** Delegate generation policy and persistence to Comfy's existing command and settings owners. */
export class ComfyGenerationAdapter {
  readonly #manager: UnknownRecord | null;
  readonly #api: UnknownRecord | null;
  readonly #marked = new Set<HTMLElement>();
  readonly #submissions = new Set<Promise<void>>();
  readonly #listeners = new Set<() => void>();
  #canceling = false;

  /** Narrow external store access before exposing facts to the strip. */
  constructor(
    private readonly document: Document,
    app: unknown,
  ) {
    this.#manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
    this.#api = isRecord(app) && isRecord(app.api) ? app.api : null;
  }

  /** Project native availability while retaining the mounted controls for mode restoration. */
  read(): GenerationStripState {
    for (const element of this.#marked) {
      if (element.isConnected) continue;
      element.removeAttribute('data-sc-generation-native');
      this.#marked.delete(element);
    }
    const native = this.document.querySelector<HTMLElement>('.actionbar-container');
    const group = native?.querySelector<HTMLElement>('.queue-button-group');
    const run = group?.querySelector<HTMLButtonElement>('button');
    const cancel = native?.querySelector<HTMLButtonElement>(
      'button[aria-label="Cancel current run"]',
    );
    const batch = native?.querySelector<HTMLElement>('.batch-count');
    const settings = this.#settings();
    const ready = settings !== null && this.#commands() !== null;
    for (const element of ready ? [group, cancel, batch] : []) {
      if (!element) continue;
      element.setAttribute('data-sc-generation-native', '');
      this.#marked.add(element);
    }
    const count = settings?.batchCount;
    const mode = settings?.mode;
    return {
      mode: mode === INSTANT_IDLE || mode === INSTANT_RUNNING ? 'continuous' : 'generate',
      looping: mode === INSTANT_RUNNING,
      batch:
        typeof count === 'number' && Number.isFinite(count)
          ? Math.max(1, Math.min(999, Math.trunc(count)))
          : 1,
      generateEnabled: ready && !!run && !run.disabled && !this.#canceling,
      skipEnabled: ready && !!cancel && !cancel.disabled && !this.#canceling,
      // A running continuous loop can always be cancelled, also between two of its runs.
      cancelEnabled:
        ready &&
        ((!!cancel && !cancel.disabled) ||
          this.#submissions.size > 0 ||
          mode === INSTANT_RUNNING) &&
        !this.#canceling,
    };
  }

  /** Update the same batch count used by Comfy.QueuePrompt, independently of latent batch size. */
  setBatch(value: number): void {
    const settings = this.#settings();
    if (!settings || !Number.isFinite(value))
      throw new Error('Comfy generation batch setting is unavailable or invalid.');
    settings.batchCount = Math.max(1, Math.min(999, Math.trunc(value)));
  }

  /**
   * Continuous generation is ComfyUI's instant run mode: it queues the batch again each time the
   * queue empties, and stops after a failed run. Choosing Generate leaves that mode.
   */
  setMode(mode: GenerationMode): void {
    const settings = this.#settings();
    if (!settings) throw new Error('Comfy generation run mode is unavailable.');
    const current = this.read().mode;
    if (current === mode) return;
    settings.mode = mode === 'continuous' ? INSTANT_IDLE : 'disabled';
  }

  /**
   * Submit through Comfy's normal generation command and execution hooks. In continuous mode the
   * first press starts the loop and a press while it runs stops it, as ComfyUI's own Run does.
   */
  async generate(): Promise<void> {
    const state = this.read();
    const settings = this.#settings();
    if (state.mode === 'continuous' && state.looping && settings) {
      settings.mode = INSTANT_IDLE;
      return;
    }
    if (!state.generateEnabled) return;
    if (state.mode === 'continuous' && settings) settings.mode = INSTANT_RUNNING;
    const submission = this.#execute('Comfy.QueuePrompt');
    this.#submissions.add(submission);
    this.#notify();
    try {
      await submission;
    } finally {
      this.#submissions.delete(submission);
      this.#notify();
    }
  }

  /** Interrupt the active generation while leaving pending host jobs intact. */
  async skip(): Promise<void> {
    if (!this.read().skipEnabled) return;
    await this.#interrupt();
  }

  /**
   * Stop a running continuous loop and pending work before interrupting the active host
   * generation. The chosen run mode stays: a continuous loop returns to waiting for the next
   * Generate.
   */
  async cancel(): Promise<void> {
    if (!this.read().cancelEnabled) return;
    this.#canceling = true;
    this.#notify();
    const settings = this.#settings();
    if (settings?.mode === INSTANT_RUNNING) settings.mode = INSTANT_IDLE;
    try {
      // Comfy submits a batch asynchronously; clear only after its already-admitted requests finish.
      await Promise.allSettled([...this.#submissions]);
      await this.#clearPending();
      await this.#interrupt();
    } finally {
      this.#canceling = false;
      this.#notify();
    }
  }

  /** Observe only mounted native generation surfaces and the public batch settings store. */
  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    const observer = new HostSurfaceObserver(this.document.body, '.actionbar-container', changed, {
      content: true,
      attributes: ['disabled'],
      exclude: '.sc-substitute-frame,.sc-generation-strip,.sc-plugin-toolbar-start',
    });
    const settings = this.#settings();
    const subscribe = settings?.$subscribe;
    const stop: unknown =
      typeof subscribe === 'function'
        ? subscribe.call(settings, changed, { detached: true, flush: 'post' })
        : null;
    return () => {
      this.#listeners.delete(changed);
      observer.disconnect();
      if (typeof stop === 'function') stop();
    };
  }

  /** Restore every native control, including surfaces retired during host renderer replacement. */
  dispose(): void {
    this.#listeners.clear();
    for (const element of this.#marked) element.removeAttribute('data-sc-generation-native');
    this.#marked.clear();
  }

  /** Reject malformed external settings rather than retaining a competing local batch count. */
  #settings(): UnknownRecord | null {
    const value = this.#manager?.queueSettings;
    return isRecord(value) && typeof value.batchCount === 'number' ? value : null;
  }

  /** Validate the host command port once per action so unavailable integration fails visibly. */
  #commands(): UnknownRecord | null {
    const value = this.#manager?.command;
    return isRecord(value) && typeof value.execute === 'function' ? value : null;
  }

  /** Publish only this boundary's in-flight command availability transitions. */
  #notify(): void {
    for (const listener of this.#listeners) listener();
  }

  /**
   * Interrupt the running generation through ComfyUI's API. The person pressed Skip or Stop, so
   * the toasts ComfyUI's own interrupt command shows would only repeat what they just did.
   */
  async #interrupt(): Promise<void> {
    const api = this.#api;
    if (!api || typeof api.interrupt !== 'function')
      throw new Error('Comfy generation API is unavailable.');
    await api.interrupt.call(api, null);
  }

  /** Clear the waiting jobs through ComfyUI's API, without its command's confirmation toast. */
  async #clearPending(): Promise<void> {
    const api = this.#api;
    if (!api || typeof api.clearItems !== 'function')
      throw new Error('Comfy generation API is unavailable.');
    await api.clearItems.call(api, 'queue');
  }

  /** Keep command failures intact for the shared SugarCubes feedback boundary. */
  async #execute(id: string): Promise<void> {
    const commands = this.#commands();
    if (!commands || typeof commands.execute !== 'function')
      throw new Error('Comfy generation commands are unavailable.');
    await commands.execute.call(commands, id);
  }
}
