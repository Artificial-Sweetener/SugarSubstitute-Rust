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
/** Tell the person when the ComfyUI their host runs has stopped, and offer to start it again. */
import type { BackendState, BackendWatcher } from './HostBackendLink.js';

/** What the outage notice shows through; each piece already exists in the page. */
export interface BackendOutagePresenter {
  /**
   * Ask before something happens; resolves true when the person agrees. Aborting `withdrawn`
   * takes the question away unanswered.
   */
  confirm(title: string, message: string, agree: string, withdrawn: AbortSignal): Promise<boolean>;
  toast(severity: 'success' | 'info' | 'warn', summary: string, detail: string): void;
  /** Show a notice that stays; answers with its withdrawal, or `null` when nothing was shown. */
  standingToast(severity: 'warn', summary: string, detail: string): (() => void) | null;
  report(error: unknown): void;
}

const STOPPED = 'ComfyUI stopped';

/**
 * Follow one host's backend and keep the person informed.
 *
 * When ComfyUI stops they are told; once the host can start it again they are asked whether to,
 * and the offer stays in the application menu (`restart`) if they would rather wait. Starting it is
 * never done without them. They are told when a restart begins and when ComfyUI is back. Only
 * the newest state stands on screen: a notice or question about an earlier one is withdrawn.
 */
export class BackendOutageNotice implements BackendWatcher {
  #retry: (() => void) | null = null;
  #down = false;
  #asked = -1;
  /** Takes back the notice or question now standing for an earlier state. */
  #withdraw: (() => void) | null = null;

  constructor(private readonly present: BackendOutagePresenter) {}

  /** Whether the host can start ComfyUI again now. */
  get canRestart(): boolean {
    return this.#retry !== null;
  }

  /** Ask the host to start ComfyUI again; does nothing unless it can. */
  restart(): void {
    const retry = this.#retry;
    this.#retry = null;
    retry?.();
  }

  /** Hear the backend's newest state from the host. */
  state(state: BackendState, retry: () => void): void {
    this.#retry = state.status === 'retry-ready' ? retry : null;
    switch (state.status) {
      case 'running':
        this.#stand(null);
        if (this.#down) this.present.toast('success', 'ComfyUI is running again', '');
        this.#down = false;
        return;
      case 'stopped':
        if (!this.#down)
          this.#stand(this.present.standingToast('warn', STOPPED, 'It cannot be restarted yet.'));
        this.#down = true;
        return;
      case 'retrying':
        this.#stand(null);
        this.#down = true;
        this.present.toast('info', 'Restarting ComfyUI…', '');
        return;
      case 'retry-ready':
        this.#down = true;
        if (this.#asked === state.revision) return;
        this.#asked = state.revision;
        void this.#offer(state.revision);
    }
  }

  /** Withdraw what stood for the earlier state and remember what stands now. */
  #stand(withdraw: (() => void) | null): void {
    const earlier = this.#withdraw;
    this.#withdraw = withdraw;
    earlier?.();
  }

  async #offer(revision: number): Promise<void> {
    const question = new AbortController();
    const withdraw = (): void => question.abort();
    this.#stand(withdraw);
    try {
      const agreed = await this.present.confirm(
        STOPPED,
        'ComfyUI is not running, so nothing can be generated until it starts again.',
        'Restart ComfyUI',
        question.signal,
      );
      // An answered question no longer stands.
      if (this.#withdraw === withdraw) this.#withdraw = null;
      // The state may have moved on while the question was open.
      if (question.signal.aborted || this.#asked !== revision || !this.#retry) return;
      if (agreed) this.restart();
      else
        this.#stand(
          this.present.standingToast(
            'warn',
            STOPPED,
            'Choose Restart ComfyUI in the application menu when you want it back.',
          ),
        );
    } catch (error: unknown) {
      this.present.report(error);
    }
  }
}
