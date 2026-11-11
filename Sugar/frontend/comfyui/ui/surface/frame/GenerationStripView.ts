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
/** Present Substitute's numeric tray and icon segments independently of host generation policy. */
import type { GenerationStripState } from './ComfyGenerationAdapter.js';

export interface GenerationStripActions {
  setBatch(value: number): void;
  generate(): Promise<void>;
  cancel(): Promise<void>;
  skip(): Promise<void>;
  report(error: unknown): void;
}

/** Keep controls and keyboard focus stable through native execution and preview updates. */
export class GenerationStripView {
  readonly element: HTMLElement;
  readonly queueSlot: HTMLElement;
  readonly #input: HTMLInputElement;
  readonly #up: HTMLButtonElement;
  readonly #down: HTMLButtonElement;
  readonly #generate: HTMLButtonElement;
  readonly #cancel: HTMLButtonElement;
  readonly #skip: HTMLButtonElement;
  #state: GenerationStripState = {
    batch: 1,
    generateEnabled: false,
    cancelEnabled: false,
    skipEnabled: false,
  };

  /** Build accessible segments in the source order: batch, cancel, generate, skip. */
  constructor(
    private readonly document: Document,
    private readonly actions: GenerationStripActions,
  ) {
    this.element = document.createElement('div');
    this.element.className = 'sc-generation-strip';
    this.element.setAttribute('role', 'group');
    this.element.setAttribute('aria-label', 'Generation controls');
    const batch = document.createElement('div');
    batch.className = 'sc-generation-batch';
    this.#input = document.createElement('input');
    this.#input.type = 'text';
    this.#input.inputMode = 'numeric';
    this.#input.setAttribute('role', 'spinbutton');
    this.#input.setAttribute('aria-label', 'Generation batch count');
    this.#input.setAttribute('aria-valuemin', '1');
    this.#input.setAttribute('aria-valuemax', '999');
    this.#input.value = '1';
    this.#input.addEventListener('focus', () => this.#input.select());
    this.#input.addEventListener('change', () => this.#commit());
    this.#input.addEventListener('blur', () => this.#commit());
    this.#input.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        this.#input.value = String(this.#state.batch);
        this.#input.blur();
      }
      if (event.key === 'Enter') {
        this.#commit();
        this.#input.blur();
      }
      if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
        event.preventDefault();
        this.#adjust(event.key === 'ArrowUp' ? 1 : -1);
      }
    });
    this.#up = this.#step('up', 1);
    this.#down = this.#step('down', -1);
    batch.append(this.#input, this.#up, this.#down);
    batch.addEventListener(
      'wheel',
      (event) => {
        if (event.deltaY === 0 || !this.#state.generateEnabled) return;
        event.preventDefault();
        this.#adjust(event.deltaY < 0 ? 1 : -1);
      },
      { passive: false },
    );
    const cluster = document.createElement('div');
    cluster.className = 'sc-generation-actions';
    this.#cancel = this.#segment('cancel', 'Cancel generation', 'stop.svg', () => actions.cancel());
    this.#generate = this.#segment('generate', 'Generate', 'play.svg', () => actions.generate());
    this.#skip = this.#segment('skip', 'Skip generation', 'next.svg', () => actions.skip());
    cluster.append(this.#cancel, this.#generate, this.#skip);
    this.queueSlot = document.createElement('span');
    this.queueSlot.className = 'sc-generation-queue-slot';
    this.queueSlot.setAttribute('aria-hidden', 'true');
    cluster.append(this.queueSlot);
    this.element.append(batch, cluster);
  }

  /** Project execution availability without replacing controls or overwriting an in-progress edit. */
  update(state: GenerationStripState): void {
    const batchChanged = state.batch !== this.#state.batch;
    this.#state = state;
    if (batchChanged || this.document.activeElement !== this.#input)
      this.#input.value = String(state.batch);
    this.#input.setAttribute('aria-valuenow', String(state.batch));
    this.#input.disabled = !state.generateEnabled;
    this.#generate.disabled = !state.generateEnabled;
    this.#cancel.disabled = !state.cancelEnabled;
    this.#skip.disabled = !state.skipEnabled;
    this.#up.disabled = !state.generateEnabled || state.batch >= 999;
    this.#down.disabled = !state.generateEnabled || state.batch <= 1;
  }

  /** Release only owned presentation nodes. */
  dispose(): void {
    this.element.remove();
  }

  /** Commit valid numeric edits and restore the canonical value for empty or invalid text. */
  #commit(): void {
    const value = this.#input.value.trim();
    if (/^\d+$/.test(value)) this.#set(Number(value));
    this.#input.value = String(this.#state.batch);
  }

  /** Apply one accessory step against the current host value. */
  #adjust(delta: number): void {
    this.#set(this.#state.batch + delta);
  }

  /** Route synchronous settings failures through the shared feedback boundary. */
  #set(value: number): void {
    if (
      !this.#state.generateEnabled ||
      Math.max(1, Math.min(999, Math.trunc(value))) === this.#state.batch
    )
      return;
    try {
      this.actions.setBatch(value);
    } catch (error: unknown) {
      this.actions.report(error);
    }
  }

  /** Draw the source-matched chevrons with separate accessible hit targets. */
  #step(direction: 'up' | 'down', delta: number): HTMLButtonElement {
    const button = this.document.createElement('button');
    button.type = 'button';
    button.className = 'sc-generation-step';
    button.dataset.direction = direction;
    button.setAttribute(
      'aria-label',
      delta > 0 ? 'Increase generation batch' : 'Decrease generation batch',
    );
    const glyph = this.document.createElement('span');
    glyph.setAttribute('aria-hidden', 'true');
    button.append(glyph);
    button.addEventListener('click', () => this.#adjust(delta));
    return button;
  }

  /** Keep trusted packaged SVG artwork theme-tinted and retain asynchronous action errors. */
  #segment(
    id: string,
    label: string,
    filename: string,
    action: () => Promise<void>,
  ): HTMLButtonElement {
    const button = this.document.createElement('button');
    button.type = 'button';
    button.className = 'sc-generation-action';
    button.dataset.action = id;
    button.setAttribute('aria-label', label);
    button.title = label;
    const glyph = this.document.createElement('span');
    glyph.className = 'sc-generation-glyph';
    glyph.setAttribute('aria-hidden', 'true');
    glyph.style.setProperty(
      '--sc-generation-glyph',
      `url("${new URL(`../../../../orb/${filename}`, import.meta.url).href}")`,
    );
    button.append(glyph);
    button.addEventListener('click', () => {
      void action().catch(this.actions.report);
    });
    return button;
  }
}
