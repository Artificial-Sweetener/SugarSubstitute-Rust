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
/**
 * Show where SugarCubes' updates stand and let the user check now or apply a ready update.
 *
 * The page reads whichever {@link UpdateSource} the page's slot holds: the host's updater, or
 * SugarCubes' own when no host updater exists. Words stay plain; the page never names update
 * mechanics.
 */
import type {
  UpdateRequestOutcome,
  UpdateSnapshot,
  UpdateSource,
  UpdateSourceSlot,
  UpdateStatus,
  UpdatesRestartTiming,
} from '../core/updates/UpdateSource.js';

/** The settings row that hosts the page. */
export const UPDATES_SETTING_ID = 'SugarCubes.Updates.Status';

const UNAVAILABLE = 'Updates are unavailable right now.';

/** Say what a refused request means for the user. */
const REFUSALS = {
  busy: 'Updates are busy right now. Try again in a moment.',
  'not-ready': 'No update is ready to apply yet.',
  unavailable: UNAVAILABLE,
} as const;

/** Say why this copy of SugarCubes is left alone. */
const UNMANAGED = {
  'git-checkout': "This copy of SugarCubes is a git checkout, so it isn't updated here.",
  'local-changes': "This copy of SugarCubes has local changes, so it isn't updated here.",
  'linked-folder': "SugarCubes is installed from a linked folder, so it isn't updated here.",
  'not-registry-install':
    "SugarCubes wasn't installed from ComfyUI Manager, so it isn't updated here. Reinstall it from ComfyUI Manager to get updates.",
} as const;

/** Say where an update stands in one plain sentence. */
export function updateStatusText(status: UpdateStatus, applies: UpdateSource['applies']): string {
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
  readonly element: HTMLElement;
  readonly #version: HTMLElement;
  readonly #channel: HTMLElement;
  readonly #checked: HTMLElement;
  readonly #status: HTMLElement;
  readonly #notes: HTMLElement;
  readonly #causes: HTMLElement;
  readonly #check: HTMLButtonElement;
  readonly #restart: HTMLButtonElement;
  readonly #restartLater: HTMLButtonElement;
  #source: UpdateSource | null = null;
  #stopSource: (() => void) | null = null;
  readonly #stopSlot: () => void;
  #refusal: string | null = null;
  #requesting = false;
  #disposed = false;

  /**
   * Build the page once; it follows the slot until disposed.
   *
   * @param formatTime Formats a completed check, given in milliseconds since the Unix epoch.
   */
  constructor(
    private readonly document: Document,
    private readonly slot: UpdateSourceSlot,
    private readonly report: (error: unknown) => void,
    private readonly formatTime: (epochMs: number) => string = (epochMs) =>
      new Date(epochMs).toLocaleString(),
  ) {
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
    this.#restartLater = this.#button('Restart after this workflow finishes', () =>
      this.#apply('after-workflow'),
    );
    actions.append(this.#check, this.#restart, this.#restartLater);
    this.element.append(details, this.#status, this.#notes, this.#causes, actions);
    this.#stopSlot = slot.subscribe(() => this.#follow());
    this.#follow();
    this.#render();
  }

  /** Release the slot and source subscriptions with the settings page lifetime. */
  dispose(): void {
    this.#disposed = true;
    this.#stopSlot();
    this.#stopSource?.();
    this.#stopSource = null;
    this.#source = null;
  }

  /** Follow the slot's current source, replacing any previous subscription. */
  #follow(): void {
    if (this.#disposed) return;
    const source = this.slot.read();
    if (source === this.#source) return;
    this.#stopSource?.();
    this.#source = source;
    this.#refusal = null;
    this.#stopSource = source?.subscribe(() => this.#render()) ?? null;
    this.#render();
  }

  #apply(when: UpdatesRestartTiming): void {
    this.#run((source) => source.apply(when));
  }

  /** Send one request at a time; a refusal stays shown until the next request. */
  #run(request: (source: UpdateSource) => Promise<UpdateRequestOutcome>): void {
    const source = this.#source;
    if (!source || this.#requesting) return;
    this.#requesting = true;
    this.#refusal = null;
    this.#render();
    request(source)
      .then((outcome) => {
        if (!outcome.accepted) this.#refusal = REFUSALS[outcome.reason];
      })
      .catch(this.report)
      .finally(() => {
        this.#requesting = false;
        if (!this.#disposed) this.#render();
      });
  }

  #render(): void {
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
    this.#causes.replaceChildren(
      ...(snapshot.report?.causes ?? []).map((cause) => {
        const item = this.document.createElement('li');
        item.textContent = cause.message;
        return item;
      }),
    );
    const ready = snapshot.status.state === 'ready';
    const hostRestart = source.applies === 'host-restart';
    this.#restart.textContent = hostRestart ? 'Restart to apply' : 'Apply update';
    this.#show(
      this.#check,
      snapshot.status.state !== 'unmanaged',
      !this.#requesting && snapshot.status.state !== 'checking',
    );
    this.#show(this.#restart, ready, !this.#requesting);
    this.#show(
      this.#restartLater,
      ready && hostRestart && snapshot.workflowRunning && !snapshot.restartAfterWorkflow,
      !this.#requesting,
    );
  }

  /** Collect the secondary lines: a refused request, a rollback and a waiting restart. */
  #notesFor(snapshot: UpdateSnapshot): string {
    const notes: string[] = [];
    if (this.#refusal) notes.push(this.#refusal);
    if (snapshot.previousVersionRunning) notes.push('The previous version is running.');
    if (snapshot.restartAfterWorkflow)
      notes.push('The restart will happen when the running workflow finishes.');
    return notes.join(' ');
  }

  #show(button: HTMLButtonElement, visible: boolean, enabled: boolean): void {
    button.hidden = !visible;
    button.disabled = !enabled;
  }

  #detail(parent: HTMLElement, label: string): HTMLElement {
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

  #button(label: string, click: () => void): HTMLButtonElement {
    const button = this.document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-button-secondary';
    button.textContent = label;
    button.addEventListener('click', click);
    return button;
  }
}
