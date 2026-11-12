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
 * Describe the one update status the SugarCubes Updates entry presents, whoever produces it.
 *
 * A host that updates itself feeds it through the `sugarcubes.updates` protocol. SugarCubes'
 * own updater, used when no host updater exists, feeds the same entry from its backend. Both
 * implement {@link UpdateSource} and attach it to the page's one {@link UpdateSourceSlot}.
 */
import type {
  UpdatesChannel,
  UpdatesRejection,
  UpdatesReport,
  UpdatesRestartTiming,
  UpdatesStatus,
} from '../embedded/UpdatesRuntime.js';

export type {
  UpdatesChannel,
  UpdatesReport,
  UpdatesRestartTiming,
} from '../embedded/UpdatesRuntime.js';

/**
 * Where an update stands.
 *
 * The protocol states are shared by every source. `restart-comfyui` means an update was applied
 * and takes effect when the user restarts ComfyUI. `unmanaged` means this copy is a git checkout
 * or has local changes, so it is never updated in place.
 */
export type UpdateStatus =
  | UpdatesStatus
  | { readonly state: 'restart-comfyui'; readonly version: string }
  | { readonly state: 'unmanaged'; readonly reason: 'git-checkout' | 'local-changes' };

/** Everything the Updates entry shows; a new snapshot replaces the previous one whole. */
export interface UpdateSnapshot {
  readonly currentVersion: string;
  readonly channel: UpdatesChannel;
  /** Milliseconds since the Unix epoch of the last check that completed, or null before any. */
  readonly lastSuccessfulCheck: number | null;
  readonly status: UpdateStatus;
  /** The report of the latest failed update attempt, when there is one to show. */
  readonly report: UpdatesReport | null;
  /** A failed update was rolled back and the previous version is running. */
  readonly previousVersionRunning: boolean;
  /** A workflow is running, so restarting can wait for it to finish. */
  readonly workflowRunning: boolean;
  /** A restart already waits for the running workflow to finish. */
  readonly restartAfterWorkflow: boolean;
}

/** The answer to one request; a refused request leaves the update state unchanged. */
export type UpdateRequestOutcome =
  | { readonly accepted: true }
  | { readonly accepted: false; readonly reason: UpdatesRejection };

/**
 * One producer of update status and the requests it accepts.
 *
 * Requests settle once the source has acted on them or refused them, and settle as
 * `unavailable` when the source is retired first. They never reject.
 */
export interface UpdateSource {
  /**
   * How a ready update takes effect: the host restarts itself after its normal close, or the
   * source applies the update and the user restarts ComfyUI.
   */
  readonly applies: 'host-restart' | 'comfyui-restart';
  /** The latest snapshot, or null until the source has reported one. */
  read(): UpdateSnapshot | null;
  subscribe(changed: () => void): () => void;
  /** Check for an update now; the result arrives as a new snapshot. */
  check(): Promise<UpdateRequestOutcome>;
  /** Apply a ready update now, or once the running workflow finishes. */
  apply(when: UpdatesRestartTiming): Promise<UpdateRequestOutcome>;
}

/**
 * Hold the page's one update source and announce when it appears or retires.
 *
 * The host's source takes this slot when the host declares `sugarcubes.updates`. SugarCubes'
 * own updater attaches only once the host description has settled without that protocol, so
 * two sources never compete for the entry.
 */
export class UpdateSourceSlot {
  #source: UpdateSource | null = null;
  readonly #listeners = new Set<() => void>();

  /** The attached source, or null when nothing on this page updates. */
  read(): UpdateSource | null {
    return this.#source;
  }

  /** Observe a source attaching or retiring. */
  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    return () => this.#listeners.delete(changed);
  }

  /**
   * Attach the page's update source.
   *
   * @returns The release that retires it from the slot.
   * @throws Error when another source is already attached.
   */
  attach(source: UpdateSource): () => void {
    if (this.#source) throw new Error('An update source is already attached to this page.');
    this.#source = source;
    this.#publish();
    return () => {
      if (this.#source !== source) return;
      this.#source = null;
      this.#publish();
    };
  }

  #publish(): void {
    for (const changed of [...this.#listeners]) changed();
  }
}
