const STOPPED = 'ComfyUI stopped';
/**
 * Follow one host's backend and keep the person informed.
 *
 * When ComfyUI stops they are told; once the host can start it again they are asked whether to,
 * and the offer stays in the application menu (`restart`) if they would rather wait. Starting it is
 * never done without them. They are told when a restart begins and when ComfyUI is back. Only
 * the newest state stands on screen: a notice or question about an earlier one is withdrawn.
 */
export class BackendOutageNotice {
    present;
    #retry = null;
    #down = false;
    #asked = -1;
    /** Takes back the notice or question now standing for an earlier state. */
    #withdraw = null;
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
                this.#stand(null);
                if (this.#down)
                    this.present.toast('success', 'ComfyUI is running again', '');
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
                if (this.#asked === state.revision)
                    return;
                this.#asked = state.revision;
                void this.#offer(state.revision);
        }
    }
    /** Withdraw what stood for the earlier state and remember what stands now. */
    #stand(withdraw) {
        const earlier = this.#withdraw;
        this.#withdraw = withdraw;
        earlier?.();
    }
    async #offer(revision) {
        const question = new AbortController();
        const withdraw = () => question.abort();
        this.#stand(withdraw);
        try {
            const agreed = await this.present.confirm(STOPPED, 'ComfyUI is not running, so nothing can be generated until it starts again.', 'Restart ComfyUI', question.signal);
            // An answered question no longer stands.
            if (this.#withdraw === withdraw)
                this.#withdraw = null;
            // The state may have moved on while the question was open.
            if (question.signal.aborted || this.#asked !== revision || !this.#retry)
                return;
            if (agreed)
                this.restart();
            else
                this.#stand(this.present.standingToast('warn', STOPPED, 'Choose Restart ComfyUI in the application menu when you want it back.'));
        }
        catch (error) {
            this.present.report(error);
        }
    }
}
