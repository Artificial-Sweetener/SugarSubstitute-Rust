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
/** One finite management lifetime publishes the existing completion/knowledge owner atomically. */
import { requireValue } from '../../../core/src/required.js';
import type { AutocompleteListCommitReceipt } from './committed.js';
import { AutocompleteListCommittedError } from './committed.js';
import { LatestRequest } from '../requests/latest.js';
import { prepareTagCompletionCatalog } from '../completion/catalog.js';
import type { BundledTag } from '../completion/contracts.js';
import type {
  AutocompleteListCommand,
  AutocompleteListOptions,
  AutocompleteListOutcome,
  AutocompleteListSettlement,
  AutocompleteListStorage,
  PreparedAutocompleteLists,
} from './contracts.js';
import { prepareLists, admitListMutation, readListCommand, readListResult } from './preparation.js';
import { assertListMutation, sameListSnapshot } from './admission.js';
/** Cancellation settles acknowledgement immediately while actual unsettled host persistence retains admission. */
export class AutocompleteListSession {
  private readonly lane = new LatestRequest();
  private readonly commits = new WeakMap<
    object,
    AutocompleteListCommitReceipt & {
      readonly command: AutocompleteListCommand;
    }
  >();
  private bundled: readonly BundledTag[];
  private current: PreparedAutocompleteLists | undefined;
  private fresh = false;
  private busy = false;
  private disposed = false;
  private generation = 0;
  private storage: AutocompleteListStorage | undefined;
  private observe: AutocompleteListOptions['observe'];
  private settlement: Promise<AutocompleteListSettlement> = Promise.resolve({
    status: 'settled',
  });
  /** Validate/copy bundled data now; host storage and observer remain injected lifetime boundaries. */
  constructor(options: AutocompleteListOptions) {
    if (
      typeof options.storage?.read !== 'function' ||
      typeof options.storage.mutate !== 'function' ||
      (options.observe !== undefined && typeof options.observe !== 'function')
    )
      throw new TypeError('Invalid autocomplete list options');
    prepareTagCompletionCatalog({
      revision: 'initial',
      bundled: options.bundled,
      customTags: [],
      censoredTags: [],
    });
    this.bundled = Object.freeze(options.bundled.map((row) => Object.freeze({ ...row })));
    this.storage = options.storage;
    this.observe = options.observe;
  }
  /** Retain the last accepted publication for display; freshness explicitly governs new persistence admission. */
  snapshot(): {
    readonly value: PreparedAutocompleteLists | undefined;
    readonly fresh: boolean;
    readonly pending: boolean;
  } {
    return Object.freeze({
      value: this.current,
      fresh: this.fresh,
      pending: this.busy,
    });
  }
  /** Await actual admitted storage settlement, including persistence that outlives cancelled acknowledgement. */
  whenSettled(): Promise<AutocompleteListSettlement> {
    return this.settlement;
  }
  /** Read exact accepted list text without filesystem work or normalization; stale publications cannot masquerade as current reads. */
  readText(
    id: string,
    revision: string,
  ):
    | {
        readonly status: 'ready';
        readonly text: string;
      }
    | {
        readonly status: 'stale' | 'not-found' | 'disposed';
      } {
    if (this.disposed) return { status: 'disposed' };
    if (!this.fresh || this.current?.revision !== revision) return { status: 'stale' };
    const list = this.current.lists.find((row) => row.id === id);
    return list ? { status: 'ready', text: list.text } : { status: 'not-found' };
  }
  /** Refresh through one atomic host read; pending persistence cannot be bypassed by a parallel refresh. */
  refresh(): Promise<AutocompleteListOutcome> {
    return this.request();
  }
  /** Admit a validated command only against the exact currently accepted host revision. */
  mutate(command: AutocompleteListCommand, revision: string): Promise<AutocompleteListOutcome> {
    if (this.disposed) return Promise.resolve({ status: 'disposed' });
    const captured = readListCommand(command);
    if (!captured) return Promise.resolve({ status: 'invalid' });
    if (!this.fresh || !this.current || this.current.revision !== revision)
      return Promise.resolve({ status: 'stale' });
    if (captured.kind !== 'create' && !this.current.lists.some((row) => row.id === captured.id))
      return Promise.resolve({ status: 'not-found' });
    if (this.busy) return Promise.resolve({ status: 'busy' });
    const before = this.current,
      generation = this.generation;
    if (!admitListMutation(before, captured, this.bundled))
      return Promise.resolve({ status: 'invalid' });
    if (this.disposed) return Promise.resolve({ status: 'disposed' });
    if (this.current !== before || generation !== this.generation || !this.fresh)
      return Promise.resolve({ status: 'stale' });
    return this.request(captured);
  }
  /** Consume only this session's issued receipt for the exact captured targeted command/prior revision, including after observer reentry; copies and host snapshots grant no authority. */
  takeCommitted(
    cause: unknown,
    command: AutocompleteListCommand,
    priorRevision: string,
  ): AutocompleteListCommitReceipt | null {
    if (!cause || typeof cause !== 'object') return null;
    const issued = this.commits.get(cause);
    if (!issued || issued.priorRevision !== priorRevision) return null;
    const captured = readListCommand(command);
    if (
      !captured ||
      JSON.stringify(captured) !== JSON.stringify(issued.command) ||
      this.commits.get(cause) !== issued
    )
      return null;
    this.commits.delete(cause);
    return Object.freeze({
      value: issued.value,
      priorRevision: issued.priorRevision,
    });
  }
  /** Cancel acknowledgement, retire current freshness and preserve admission until the actual host promise settles. */
  cancel(): void {
    ++this.generation;
    this.fresh = false;
    this.lane.cancel();
  }
  /** Retire before abort callbacks; no host result or observer can publish into a disposed lifetime. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    ++this.generation;
    this.fresh = false;
    this.current = undefined;
    this.storage = undefined;
    this.observe = undefined;
    this.bundled = Object.freeze([]);
    if (!this.busy) this.settlement = Promise.resolve({ status: 'settled' });
    this.lane.dispose();
  }
  /** Existing latest-request owner handles reentrancy/cancellation; this owner separately retains actual storage admission. */
  private async request(command?: AutocompleteListCommand): Promise<AutocompleteListOutcome> {
    if (this.disposed) return { status: 'disposed' };
    if (this.busy) return { status: 'busy' };
    const before = this.current;
    const revision = before?.revision;
    const storage = requireValue(this.storage, 'session');
    const generation = ++this.generation;
    this.busy = true;
    this.fresh = false;
    const outcome = await this.lane.run((signal) => {
      const operation = (async (): Promise<unknown> => {
        try {
          return await (command
            ? storage.mutate(command, requireValue(revision, 'session'), signal)
            : storage.read(signal));
        } finally {
          this.busy = false;
        }
      })();
      const settlement = operation.then(
        () => ({ status: 'settled' as const }),
        (cause: unknown) => ({ status: 'failed' as const, cause }),
      );
      this.settlement = settlement;
      void settlement.then(() => {
        if (this.disposed && this.settlement === settlement)
          this.settlement = Promise.resolve({ status: 'settled' });
      });
      return operation;
    });
    if (outcome.status !== 'ready') return outcome;
    if (this.disposed) return { status: 'disposed' };
    if (generation !== this.generation) return { status: 'cancelled' };
    const result = readListResult(outcome.value);
    if (!command && (this.disposed || generation !== this.generation))
      return { status: this.disposed ? 'disposed' : 'cancelled' };
    if (result.status !== 'ready') {
      if (!command && result.status !== 'unavailable' && result.status !== 'invalid')
        throw new TypeError('Invalid autocomplete read rejection');
      return result;
    }
    if (command)
      assertListMutation(
        requireValue(before, 'session'),
        result.snapshot,
        command,
        result.affectedId,
      );
    else if (result.affectedId !== null) throw new TypeError('Invalid autocomplete read identity');
    if (command && (this.disposed || generation !== this.generation))
      return {
        status: 'committed',
        value: result.snapshot,
        retirement: this.disposed ? 'disposed' : 'cancelled',
      };
    if (
      !command &&
      before?.revision === result.snapshot.revision &&
      !sameListSnapshot(before, result.snapshot)
    )
      throw new TypeError('Autocomplete list revision reused for changed content');
    let prepared: PreparedAutocompleteLists | undefined;
    try {
      prepared =
        !command && before && sameListSnapshot(before, result.snapshot)
          ? before
          : prepareLists(result.snapshot, this.bundled);
      if (this.disposed || generation !== this.generation) {
        if (command) return { status: 'ready', value: prepared };
        return { status: this.disposed ? 'disposed' : 'cancelled' };
      }
      this.current = prepared;
      this.fresh = true;
      this.observe?.(prepared);
    } catch (cause) {
      if (command) {
        const value = prepared ?? result.snapshot;
        const failure = new AutocompleteListCommittedError(
          value,
          requireValue(revision, 'session'),
          cause,
        );
        this.commits.set(failure, {
          value,
          priorRevision: requireValue(revision, 'session'),
          command,
        });
        throw failure;
      }
      throw cause;
    }
    // A validated commit receipt survives observer retirement; freshness and future admission remain retired.
    if (command) return { status: 'ready', value: prepared };
    if (this.disposed) return { status: 'disposed' };
    if (generation !== this.generation) return { status: 'cancelled' };
    return { status: 'ready', value: prepared };
  }
}
