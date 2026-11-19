const STOPPED = 'ComfyUI stopped';
/**
 * Follow one host's backend and keep the person informed.
 *
 * When ComfyUI stops they are told; once the host can start it again they are asked whether to,
 * and the offer stays in the application menu (`restart`) if they would rather wait. Starting it is
 * never done without them. They are told when a restart begins and when ComfyUI is back.
 */
export class BackendOutageNotice {
    present;
    #retry = null;
    #down = false;
    #asked = -1;
    constructor(present) {
        this.present = present;
    }
    /** Whether the host can start ComfyUI again now. */
    get canRestart() {
        return this.#retry !== null;
    }
    /** Ask the host to start ComfyUI again; does nothing unless it can. */
    restart() {
        const retry = this.#retry;
        this.#retry = null;
        retry?.();
    }
    /** Hear the backend's newest state from the host. */
    state(state, retry) {
        this.#retry = state.status === 'retry-ready' ? retry : null;
        switch (state.status) {
            case 'running':
                if (this.#down)
                    this.present.toast('success', 'ComfyUI is running again', '');
                this.#down = false;
                return;
            case 'stopped':
                if (!this.#down)
                    this.present.lastingToast('warn', STOPPED, 'It cannot be restarted yet.');
                this.#down = true;
                return;
            case 'retrying':
                this.#down = true;
                this.present.toast('info', 'Restarting ComfyUI…', '');
                return;
            case 'retry-ready':
                this.#down = true;
                if (this.#asked === state.revision)
                    return;
                this.#asked = state.revision;
                void this.#offer(state.revision);
        }
    }
    async #offer(revision) {
        try {
            const agreed = await this.present.confirm(STOPPED, 'ComfyUI is not running, so nothing can be generated until it starts again.', 'Restart ComfyUI');
            // The state may have moved on while the question was open.
            if (this.#asked !== revision || !this.#retry)
                return;
            if (agreed)
                this.restart();
            else
                this.present.lastingToast('warn', STOPPED, 'Choose Restart ComfyUI in the application menu when you want it back.');
        }
        catch (error) {
            this.present.report(error);
        }
    }
}
