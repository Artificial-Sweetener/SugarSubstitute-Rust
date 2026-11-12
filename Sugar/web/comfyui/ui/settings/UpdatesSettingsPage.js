/** The settings row that hosts the page. */
export const UPDATES_SETTING_ID = 'SugarCubes.Updates.Status';
const UNAVAILABLE = 'Updates are unavailable right now.';
/** Say what a refused request means for the user. */
const REFUSALS = {
    busy: 'Updates are busy right now. Try again in a moment.',
    'not-ready': 'No update is ready to apply yet.',
    unavailable: UNAVAILABLE,
};
/** Say why this copy of SugarCubes is left alone. */
const UNMANAGED = {
    'git-checkout': "This copy of SugarCubes is a git checkout, so it isn't updated here.",
    'local-changes': "This copy of SugarCubes has local changes, so it isn't updated here.",
    'linked-folder': "SugarCubes is installed from a linked folder, so it isn't updated here.",
    'not-registry-install': "SugarCubes wasn't installed from ComfyUI Manager, so it isn't updated here. Reinstall it from ComfyUI Manager to get updates.",
};
/** Say where an update stands in one plain sentence. */
export function updateStatusText(status, applies) {
    switch (status.state) {
        case 'up-to-date':
            return 'SugarCubes is up to date.';
        case 'checking':
            return 'Checking for updates…';
        case 'available':
            return `Version ${status.version} is available.`;
        case 'ready':
            return applies === 'host-restart'
                ? `Version ${status.version} is ready. Restart to apply it.`
                : `Version ${status.version} is ready. Apply it, then restart ComfyUI.`;
        case 'failed':
            return status.during === 'check'
                ? "Couldn't check for updates."
                : "The update couldn't be installed.";
        case 'restart-comfyui':
            return `Version ${status.version} will be installed when you restart ComfyUI.`;
        case 'unmanaged':
            return UNMANAGED[status.reason];
    }
}
/** Keep one stable page whose rows follow every snapshot the attached source reports. */
export class UpdatesSettingsPage {
    document;
    slot;
    report;
    formatTime;
    element;
    #version;
    #channel;
    #checked;
    #status;
    #notes;
    #causes;
    #check;
    #restart;
    #restartLater;
    #source = null;
    #stopSource = null;
    #stopSlot;
    #refusal = null;
    #requesting = false;
    #disposed = false;
    /**
     * Build the page once; it follows the slot until disposed.
     *
     * @param formatTime Formats a completed check, given in milliseconds since the Unix epoch.
     */
    constructor(document, slot, report, formatTime = (epochMs) => new Date(epochMs).toLocaleString()) {
        this.document = document;
        this.slot = slot;
        this.report = report;
        this.formatTime = formatTime;
        this.element = document.createElement('section');
        this.element.className = 'sc-settings-about sc-settings-updates';
        this.element.setAttribute('aria-label', 'SugarCubes updates');
        const details = document.createElement('div');
        details.className = 'sc-settings-about__versions';
        this.#version = this.#detail(details, 'Current version');
        this.#channel = this.#detail(details, 'Channel');
        this.#checked = this.#detail(details, 'Last successful check');
        this.#status = document.createElement('p');
        this.#status.className = 'sc-settings-updates__status';
        this.#status.setAttribute('role', 'status');
        this.#status.setAttribute('aria-live', 'polite');
        this.#notes = document.createElement('p');
        this.#notes.className = 'sc-settings-about__status';
        this.#causes = document.createElement('ul');
        this.#causes.className = 'sc-settings-updates__causes';
        this.#causes.setAttribute('aria-label', 'What went wrong');
        const actions = document.createElement('div');
        actions.className = 'sc-settings-about__actions';
        this.#check = this.#button('Check for updates', () => this.#run((source) => source.check()));
        this.#restart = this.#button('Restart to apply', () => this.#apply('now'));
        this.#restartLater = this.#button('Restart after this workflow finishes', () => this.#apply('after-workflow'));
        actions.append(this.#check, this.#restart, this.#restartLater);
        this.element.append(details, this.#status, this.#notes, this.#causes, actions);
        this.#stopSlot = slot.subscribe(() => this.#follow());
        this.#follow();
        this.#render();
    }
    /** Release the slot and source subscriptions with the settings page lifetime. */
    dispose() {
        this.#disposed = true;
        this.#stopSlot();
        this.#stopSource?.();
        this.#stopSource = null;
        this.#source = null;
    }
    /** Follow the slot's current source, replacing any previous subscription. */
    #follow() {
        if (this.#disposed)
            return;
        const source = this.slot.read();
        if (source === this.#source)
            return;
        this.#stopSource?.();
        this.#source = source;
        this.#refusal = null;
        this.#stopSource = source?.subscribe(() => this.#render()) ?? null;
        this.#render();
    }
    #apply(when) {
        this.#run((source) => source.apply(when));
    }
    /** Send one request at a time; a refusal stays shown until the next request. */
    #run(request) {
        const source = this.#source;
        if (!source || this.#requesting)
            return;
        this.#requesting = true;
        this.#refusal = null;
        this.#render();
        request(source)
            .then((outcome) => {
            if (!outcome.accepted)
                this.#refusal = REFUSALS[outcome.reason];
        })
            .catch(this.report)
            .finally(() => {
            this.#requesting = false;
            if (!this.#disposed)
                this.#render();
        });
    }
    #render() {
        const source = this.#source;
        const snapshot = source?.read() ?? null;
        if (!source || !snapshot) {
            this.#version.textContent = '—';
            this.#channel.textContent = '—';
            this.#checked.textContent = '—';
            this.#status.textContent = source ? 'Reading update status…' : UNAVAILABLE;
            this.#notes.textContent = '';
            this.#causes.replaceChildren();
            this.#show(this.#check, false, false);
            this.#show(this.#restart, false, false);
            this.#show(this.#restartLater, false, false);
            return;
        }
        this.#version.textContent = snapshot.currentVersion;
        this.#channel.textContent = snapshot.channel === 'beta' ? 'Beta' : 'Stable';
        this.#checked.textContent =
            snapshot.lastSuccessfulCheck === null
                ? 'Not checked yet'
                : this.formatTime(snapshot.lastSuccessfulCheck);
        this.#status.textContent = updateStatusText(snapshot.status, source.applies);
        this.#notes.textContent = this.#notesFor(snapshot);
        this.#causes.replaceChildren(...(snapshot.report?.causes ?? []).map((cause) => {
            const item = this.document.createElement('li');
            item.textContent = cause.message;
            return item;
        }));
        const ready = snapshot.status.state === 'ready';
        const hostRestart = source.applies === 'host-restart';
        this.#restart.textContent = hostRestart ? 'Restart to apply' : 'Apply update';
        this.#show(this.#check, snapshot.status.state !== 'unmanaged', !this.#requesting && snapshot.status.state !== 'checking');
        this.#show(this.#restart, ready, !this.#requesting);
        this.#show(this.#restartLater, ready && hostRestart && snapshot.workflowRunning && !snapshot.restartAfterWorkflow, !this.#requesting);
    }
    /** Collect the secondary lines: a refused request, a rollback and a waiting restart. */
    #notesFor(snapshot) {
        const notes = [];
        if (this.#refusal)
            notes.push(this.#refusal);
        if (snapshot.previousVersionRunning)
            notes.push('The previous version is running.');
        if (snapshot.restartAfterWorkflow)
            notes.push('The restart will happen when the running workflow finishes.');
        return notes.join(' ');
    }
    #show(button, visible, enabled) {
        button.hidden = !visible;
        button.disabled = !enabled;
    }
    #detail(parent, label) {
        const card = this.document.createElement('article');
        card.className = 'sc-settings-about__card';
        const body = this.document.createElement('div');
        const name = this.document.createElement('strong');
        name.textContent = label;
        const value = this.document.createElement('small');
        body.append(name, value);
        card.append(body);
        parent.append(card);
        return value;
    }
    #button(label, click) {
        const button = this.document.createElement('button');
        button.type = 'button';
        button.className = 'p-button p-button-secondary';
        button.textContent = label;
        button.addEventListener('click', click);
        return button;
    }
}
