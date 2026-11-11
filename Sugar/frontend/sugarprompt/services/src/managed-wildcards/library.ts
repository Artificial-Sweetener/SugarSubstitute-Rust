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
/** One finite managed library lifetime owns revision tickets, storage requests and catalog invalidation/publication. */
import { requireValue } from '../../../core/src/required.js';
import { LatestRequest } from '../requests/latest.js';
import { ManagedWildcardStorageAdmission } from './storage-admission.js';
import {
  captureManagedWildcardStorage,
  captureManagedWildcardFolder,
  captureManagedWildcardList,
  captureManagedWildcardRead,
  captureManagedWildcardChange,
  captureManagedWildcardFolderResult,
} from './capture.js';
import { managedWildcardFile, managedWildcardCreationPath } from './paths.js';
import type {
  ManagedWildcardStorage,
  ManagedWildcardFolderAdapter,
  ManagedWildcardSnapshot,
  ManagedWildcardRefreshOutcome,
  ManagedWildcardReadOutcome,
  ManagedWildcardChangeOutcome,
  ManagedWildcardFolderOutcome,
  ManagedWildcardChangeResult,
  ManagedWildcardFailure,
} from './contracts.js';
/** Unexpected post-commit refresh failures preserve both confirmed persistence identity and the original internal cause. */
export class ManagedWildcardCommittedError extends Error {
  readonly revision: string;
  /** Report a committed mutation without exposing its path/content in the error message. */
  constructor(revision: string, cause: unknown) {
    super('Managed wildcard mutation committed; metadata refresh failed', {
      cause,
    });
    this.name = 'ManagedWildcardCommittedError';
    this.revision = revision;
  }
}
/** Managed library actions delegate storage containment and candidates to their existing owners. */
export class ManagedWildcardLibrary {
  private readonly lane = new LatestRequest();
  private readonly folderLane = new LatestRequest();
  private storage: ManagedWildcardStorage | undefined;
  private folder: ManagedWildcardFolderAdapter | undefined;
  private observe: (snapshot: ManagedWildcardSnapshot) => void;
  private current: ManagedWildcardSnapshot = Object.freeze({ status: 'idle' });
  private disposed = false;
  /** One synchronous cancellation owns only its still-current publication and folder continuations. */
  private cancellation:
    | {
        publishStale: boolean;
        cancelFolder: boolean;
      }
    | undefined;
  private readonly storageRequests = new ManagedWildcardStorageAdmission();
  /** Capture explicit host capabilities; observation receives only current immutable library/catalog metadata. */
  constructor(
    storage: ManagedWildcardStorage,
    folder?: ManagedWildcardFolderAdapter,
    observe: (snapshot: ManagedWildcardSnapshot) => void = () => {},
  ) {
    this.storage = captureManagedWildcardStorage(storage);
    this.folder = captureManagedWildcardFolder(folder);
    if (typeof observe !== 'function') throw new TypeError('Invalid managed wildcard observer');
    this.observe = observe;
  }
  /** Observe current readiness; only the genuine current ready object can authorize subsequent commands. */
  snapshot(): ManagedWildcardSnapshot {
    return this.current;
  }
  /** Reject forged or retired tickets without issuing external work. */
  accepts(ticket: ManagedWildcardSnapshot): boolean {
    return !this.disposed && ticket === this.current && ticket.status === 'ready';
  }
  /** Publish one authoritative immutable state; host observers may synchronously cancel or dispose. */
  private publish(value: ManagedWildcardSnapshot): void {
    if (this.disposed) return;
    this.current = Object.freeze(value);
    this.observe(this.current);
  }
  /** Preserve an unexpected work cause even when its failure-state observer also fails. */
  private fail(signal: AbortSignal, cause: unknown): never {
    if (!signal.aborted && !this.disposed) {
      try {
        this.publish({ status: 'failed' });
      } catch (notification) {
        throw new AggregateError(
          [cause, notification],
          'Managed wildcard failure notification failed',
        );
      }
    }
    throw cause;
  }
  /** Enumerate managed files and current metadata without explicitly invalidating host caches. */
  list(): Promise<ManagedWildcardRefreshOutcome> {
    return this.reload(false);
  }
  /** Explicit metadata refresh invalidates host process caches before preparing a fresh catalog. */
  refresh(): Promise<ManagedWildcardRefreshOutcome> {
    return this.reload(true);
  }
  /** Latest reads/refreshes supersede each other; persistence remains exclusively admitted until its actual work settles. */
  private async reload(invalidate: boolean): Promise<ManagedWildcardRefreshOutcome> {
    if (this.disposed) return { status: 'disposed' };
    if (this.storageRequests.mutationActive) return { status: 'busy' };
    const storage = requireValue(this.storage, 'library');
    if (this.cancellation) this.cancellation.publishStale = false;
    const result = await this.lane.run((signal) =>
      this.storageRequests.run(async () => {
        try {
          this.publish({ status: 'loading' });
          if (signal.aborted) return { status: 'cancelled' as const };
          if (invalidate) storage.invalidate();
          if (signal.aborted) return { status: 'cancelled' as const };
          const value = captureManagedWildcardList(await storage.list(signal));
          if (signal.aborted) return { status: 'cancelled' as const };
          if (value.status !== 'ready') {
            this.publish({
              status: value.status === 'stale' ? 'stale' : 'unavailable',
            });
            return value;
          }
          this.publish(value);
          return { status: 'ready' as const, library: value };
        } catch (cause) {
          return this.fail(signal, cause);
        }
      }),
    );
    return result.status === 'ready' ? result.value : result;
  }
  /** Read exact TXT/CSV source only for the genuine accepted storage revision. */
  async read(ticket: ManagedWildcardSnapshot, path: string): Promise<ManagedWildcardReadOutcome> {
    const rejected = this.admission(ticket);
    if (rejected) return rejected;
    const file = managedWildcardFile(path);
    if (!file) return { status: 'invalid', reason: 'path' };
    const storage = requireValue(this.storage, 'library'),
      revision = ticket.status === 'ready' ? ticket.revision : '';
    if (this.cancellation) this.cancellation.publishStale = false;
    const result = await this.lane.run((signal) =>
      this.storageRequests.run(async () => {
        try {
          const value = captureManagedWildcardRead(await storage.read(file.path, revision, signal));
          if (signal.aborted) return { status: 'cancelled' as const };
          if (
            value.status === 'stale' ||
            (value.status === 'read' && value.revision !== revision)
          ) {
            this.publish({ status: 'stale' });
            return { status: 'stale' as const };
          }
          return value.status === 'read' ? Object.freeze({ ...value, path: file.path }) : value;
        } catch (cause) {
          return this.fail(signal, cause);
        }
      }),
    );
    return result.status === 'ready' ? result.value : result;
  }
  /** Create or replace exact authored contents; successful changes invalidate and refresh the existing catalog owner. */
  write(
    ticket: ManagedWildcardSnapshot,
    path: string,
    text: string,
  ): Promise<ManagedWildcardChangeOutcome> {
    const file = managedWildcardFile(path);
    if (!file) return Promise.resolve({ status: 'invalid', reason: 'path' });
    if (typeof text !== 'string' || text.length > 8000000)
      return Promise.resolve({ status: 'invalid', reason: 'content' });
    return this.change(ticket, (storage, revision, signal) =>
      storage.write(file.path, text, revision, signal),
    );
  }
  /** Separate TXT creation retains the donor's create-or-replace behavior and empty default source. */
  createText(
    ticket: ManagedWildcardSnapshot,
    identifier: string,
    text = '',
  ): Promise<ManagedWildcardChangeOutcome> {
    const path = managedWildcardCreationPath(identifier, 'text');
    return path
      ? this.write(ticket, path, text)
      : Promise.resolve({ status: 'invalid', reason: 'path' });
  }
  /** Separate CSV creation supplies one default value header; explicit empty contents remain exact. */
  createCsv(
    ticket: ManagedWildcardSnapshot,
    identifier: string,
    text = 'value\n',
  ): Promise<ManagedWildcardChangeOutcome> {
    const path = managedWildcardCreationPath(identifier, 'csv');
    return path
      ? this.write(ticket, path, text)
      : Promise.resolve({ status: 'invalid', reason: 'path' });
  }
  /** Rename managed relative files; hosts atomically enforce root containment, revision and destination collision policy. */
  rename(
    ticket: ManagedWildcardSnapshot,
    from: string,
    to: string,
  ): Promise<ManagedWildcardChangeOutcome> {
    const oldFile = managedWildcardFile(from),
      newFile = managedWildcardFile(to);
    if (!oldFile || !newFile) return Promise.resolve({ status: 'invalid', reason: 'path' });
    return this.change(ticket, (storage, revision, signal) =>
      storage.rename(oldFile.path, newFile.path, revision, signal),
    );
  }
  /** Delete only through the managed host boundary, retaining explicit missing/unavailable outcomes. */
  delete(ticket: ManagedWildcardSnapshot, path: string): Promise<ManagedWildcardChangeOutcome> {
    const file = managedWildcardFile(path);
    if (!file) return Promise.resolve({ status: 'invalid', reason: 'path' });
    return this.change(ticket, (storage, revision, signal) =>
      storage.delete(file.path, revision, signal),
    );
  }
  /** Admit latest reads independently; persistence additionally waits for all actual storage work. */
  private admission(
    ticket: ManagedWildcardSnapshot,
    mutation = false,
  ): ManagedWildcardFailure | null {
    if (this.disposed) return { status: 'disposed' };
    if (mutation ? this.storageRequests.mutationBlocked : this.storageRequests.mutationActive)
      return { status: 'busy' };
    return this.accepts(ticket) ? null : { status: 'stale' };
  }
  /** One atomic revision-bound mutation owns successful invalidation and its complete metadata handoff. */
  private async change(
    ticket: ManagedWildcardSnapshot,
    work: (
      storage: ManagedWildcardStorage,
      revision: string,
      signal: AbortSignal,
    ) => Promise<ManagedWildcardChangeResult>,
  ): Promise<ManagedWildcardChangeOutcome> {
    const rejected = this.admission(ticket, true);
    if (rejected) return rejected;
    const storage = requireValue(this.storage, 'library'),
      revision = ticket.status === 'ready' ? ticket.revision : '';
    if (!this.storageRequests.beginMutation()) return { status: 'busy' };
    if (this.cancellation) this.cancellation.publishStale = false;
    let started = false;
    try {
      const result = await this.lane.run(async (signal) => {
        started = true;
        let committed: string | null = null;
        try {
          this.publish({ status: 'stale' });
          if (signal.aborted) return { status: 'cancelled' as const };
          const changed = captureManagedWildcardChange(await work(storage, revision, signal));
          if (changed.status !== 'changed') return changed;
          committed = changed.revision;
          storage.invalidate();
          if (committed === revision)
            throw new TypeError('Managed wildcard mutation did not advance revision');
          if (signal.aborted) return { status: 'cancelled' as const };
          const value = captureManagedWildcardList(await storage.list(signal));
          if (signal.aborted) return { status: 'cancelled' as const };
          const library: ManagedWildcardSnapshot =
            value.status === 'ready' && value.revision === committed
              ? value
              : {
                  status:
                    value.status === 'stale' || value.status === 'ready' ? 'stale' : 'unavailable',
                };
          this.publish(library);
          return Object.freeze({
            status: 'changed' as const,
            revision: committed,
            library: this.current,
          });
        } catch (cause) {
          return this.fail(
            signal,
            committed === null ? cause : new ManagedWildcardCommittedError(committed, cause),
          );
        } finally {
          this.storageRequests.endMutation();
        }
      });
      return result.status === 'ready' ? result.value : result;
    } finally {
      if (!started) this.storageRequests.endMutation();
    }
  }
  /** Issue a path-free folder intent through its independent optional latest-wins host capability. */
  async openFolder(): Promise<ManagedWildcardFolderOutcome> {
    if (this.disposed) return { status: 'disposed' };
    if (this.cancellation) this.cancellation.cancelFolder = false;
    const folder = this.folder;
    const result = await this.folderLane.run(async (signal) =>
      folder
        ? captureManagedWildcardFolderResult(
            await folder.open(Object.freeze({ kind: 'open-managed-wildcards' }), signal),
          )
        : { status: 'unavailable' as const },
    );
    return result.status === 'ready' ? result.value : result;
  }
  /** Retire current work; synchronous newer lane intent supersedes its respective cancellation continuation. */
  cancel(): void {
    if (this.disposed) return;
    const cancellation = { publishStale: true, cancelFolder: true };
    this.cancellation = cancellation;
    try {
      this.lane.cancel();
      if (this.cancellation !== cancellation || this.disposed) return;
      if (cancellation.cancelFolder) this.folderLane.cancel();
      if (this.cancellation === cancellation && !this.disposed && cancellation.publishStale)
        this.publish({ status: 'stale' });
    } finally {
      if (this.cancellation === cancellation) this.cancellation = undefined;
    }
  }
  /** Settle owned requests, retire tickets and detach capabilities/observer before late external completion. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.cancellation = undefined;
    this.current = Object.freeze({ status: 'disposed' });
    this.lane.dispose();
    this.folderLane.dispose();
    this.storage = undefined;
    this.folder = undefined;
    const observe = this.observe;
    this.observe = () => {};
    observe(this.current);
  }
}
