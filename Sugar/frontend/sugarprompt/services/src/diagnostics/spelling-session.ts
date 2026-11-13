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
/** One injected dictionary session owns word suppression and finite concurrent external operations. */
import { requireValue } from '../../../core/src/required.js';
import { caseFold } from '../../../core/src/prompt/text.js';
import { decodedRangeToSource } from '../../../core/src/document/value-mapping.js';
import { LatestRequest } from '../requests/latest.js';
import { isPreparedSpellingCandidates } from './spelling-preparation.js';
import type {
  SpellcheckGateway,
  SpellingAvailability,
  SpellingLookup,
  SpellingOutcome,
  SpellingUnavailableCode,
  PreparedSpellingCandidates,
  PreparedSpellingSnapshot,
  PreparedSpellingSuggestions,
  SpellingIssue,
  SpellingIgnoreResult,
} from './spelling-contracts.js';
const sessions = new WeakSet<SpellcheckSession>();
/** Native hosts admit only sessions initialized by this service owner, retaining their actual ignore authority. */
export function isSpellcheckSession(value: unknown): value is SpellcheckSession {
  return value instanceof SpellcheckSession && sessions.has(value);
}
/** Only static expected absence codes cross the adapter boundary. */
function unavailableCode(value: unknown): SpellingUnavailableCode {
  if (
    value !== 'backend-unavailable' &&
    value !== 'dictionary-unavailable' &&
    value !== 'unsupported'
  )
    throw new TypeError('spelling-unavailable-code');
  return value;
}
/** Snapshot adapter properties once; caller-owned status/value getters cannot rewrite admitted output later. */
function lookup(value: SpellingLookup): SpellingLookup {
  if (!value || typeof value !== 'object') throw new TypeError('spelling-lookup');
  const status = value.status;
  if (status === 'ready') return { status, value: value.value };
  if (status === 'unavailable') return { status, code: unavailableCode(value.code) };
  if (status === 'cancelled') return { status };
  throw new TypeError('spelling-lookup-status');
}
/** Read every dense cell once before validating the complete bounded payload. */
function cells(value: unknown, maximum: number): readonly unknown[] {
  if (!Array.isArray(value)) throw new TypeError('spelling-payload-array');
  const length: number = value.length;
  if (!Number.isSafeInteger(length) || length < 0 || length > maximum)
    throw new TypeError('spelling-payload-length');
  const result: unknown[] = [];
  for (let index = 0; index < length; index++) {
    if (!Object.hasOwn(value, index)) throw new TypeError('spelling-payload-hole');
    result.push(value[index]);
  }
  return Object.freeze(result);
}
/** Literal strings preserve punctuation and Unicode while refusing malformed or unbounded external text. */
function suggestion(value: unknown): string {
  if (typeof value !== 'string' || !value || value.length > 4096 || /[\uD800-\uDFFF]/u.test(value))
    throw new TypeError('spelling-suggestion');
  return value;
}
/** Shared host session survives editor remount; each operation has its own cancellation lane. */
export class SpellcheckSession {
  readonly identity = Symbol();
  readonly gatewayIdentity: symbol;
  readonly languageTag: string;
  private readonly gateway: SpellcheckGateway;
  private readonly ignored = new Set<string>();
  private readonly active = new Set<LatestRequest>();
  private readonly subscribers = new Set<() => void>();
  private readonly snapshots = new WeakSet<PreparedSpellingSnapshot>();
  private currentRevision = 0;
  private disposed = false;
  /** Capture adapter identity and methods before callers can mutate its public descriptor. */
  constructor(gateway: SpellcheckGateway) {
    const identity = gateway.identity,
      languageTag = gateway.languageTag,
      availability = gateway.availability,
      checkWords = gateway.checkWords,
      suggest = gateway.suggest,
      add = gateway.addToDictionary,
      ignore = gateway.ignoreForSession;
    if (
      typeof identity !== 'symbol' ||
      typeof languageTag !== 'string' ||
      !languageTag ||
      languageTag.length > 128 ||
      typeof availability !== 'function' ||
      typeof checkWords !== 'function' ||
      typeof suggest !== 'function' ||
      (add !== undefined && typeof add !== 'function') ||
      (ignore !== undefined && typeof ignore !== 'function')
    )
      throw new TypeError('spelling-gateway');
    this.gatewayIdentity = identity;
    this.languageTag = languageTag;
    this.gateway = Object.freeze({
      identity,
      languageTag,
      availability: availability.bind(gateway),
      checkWords: checkWords.bind(gateway),
      suggest: suggest.bind(gateway),
      ...(add ? { addToDictionary: add.bind(gateway) } : {}),
      ...(ignore ? { ignoreForSession: ignore.bind(gateway) } : {}),
    });
    sessions.add(this);
  }
  /** Monotonic dictionary/ignore revision is observed rather than mirrored by editor consumers. */
  get revision(): number {
    return this.currentRevision;
  }
  /** External property reads may retire or replace the session before any later adaptation or side effect. */
  private retirement(
    signal: AbortSignal,
    revision: number,
  ): {
    readonly status: 'cancelled' | 'disposed';
  } | null {
    if (this.disposed) return { status: 'disposed' };
    return signal.aborted || revision !== this.currentRevision ? { status: 'cancelled' } : null;
  }
  /** One mounted consumer owns the returned release function; disposed sessions retain no subscribers. */
  subscribe(listener: () => void): () => void {
    if (this.disposed) return () => {};
    this.subscribers.add(listener);
    return () => {
      this.subscribers.delete(listener);
    };
  }
  /** Capture all capability cells once and reject a promised operation without its actual method. */
  private availability(): SpellingAvailability {
    const value = this.gateway.availability(),
      status = value.status;
    if (status === 'unavailable') return { status, code: unavailableCode(value.code) };
    if (status !== 'available') throw new TypeError('spelling-availability');
    const dictionaryAdd = value.dictionaryAdd,
      sessionIgnore = value.sessionIgnore;
    if (
      typeof dictionaryAdd !== 'boolean' ||
      typeof sessionIgnore !== 'boolean' ||
      (dictionaryAdd && !this.gateway.addToDictionary) ||
      (sessionIgnore && !this.gateway.ignoreForSession)
    )
      throw new TypeError('spelling-capability');
    return Object.freeze({ status, dictionaryAdd, sessionIgnore });
  }
  /** Retire earlier work before notifying consumers; one failing observer does not prevent other retirement notifications. */
  private changed(): void {
    this.currentRevision++;
    for (const operation of [...this.active]) operation.cancel();
    const causes: unknown[] = [];
    for (const listener of [...this.subscribers]) {
      try {
        listener();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'spelling-session-observers');
  }
  /** Each concurrent request retains a separate finite lane; payload capture is followed by an abort/revision publication fence. */
  private async run<T>(
    signal: AbortSignal,
    revision: number,
    work: (signal: AbortSignal) => Promise<SpellingLookup>,
    adapt: (payload: unknown) => T,
  ): Promise<SpellingOutcome<T>> {
    if (this.disposed) return { status: 'disposed' };
    if (signal.aborted || revision !== this.currentRevision) return { status: 'cancelled' };
    const lane = new LatestRequest();
    this.active.add(lane);
    /** Caller abort settles this operation only, without superseding another mounted consumer. */
    const abort = (): void => lane.cancel();
    signal.addEventListener('abort', abort, { once: true });
    try {
      if (signal.aborted) lane.cancel();
      const outcome = await lane.run(async (ownedSignal) => {
        if (signal.aborted || revision !== this.currentRevision)
          return { status: 'cancelled' } as const;
        const captured = lookup(await work(ownedSignal));
        if (captured.status !== 'ready') return captured;
        const value = adapt(captured.value);
        if (
          ownedSignal.aborted ||
          signal.aborted ||
          this.disposed ||
          revision !== this.currentRevision
        )
          return { status: 'cancelled' } as const;
        return { status: 'ready', value } as const;
      });
      if (this.disposed) return { status: 'disposed' };
      if (outcome.status !== 'ready') return { status: 'cancelled' };
      if (signal.aborted || revision !== this.currentRevision) return { status: 'cancelled' };
      return outcome.value;
    } finally {
      signal.removeEventListener('abort', abort);
      this.active.delete(lane);
      lane.dispose();
    }
  }
  /** Project unique dictionary decisions onto actual canonical candidates; expected absence never becomes an empty ready snapshot. */
  async check(
    prepared: PreparedSpellingCandidates,
    signal: AbortSignal,
  ): Promise<SpellingOutcome<PreparedSpellingSnapshot>> {
    if (this.disposed) return { status: 'disposed' };
    if (!isPreparedSpellingCandidates(prepared)) return { status: 'rejected' };
    const revision = this.currentRevision,
      availability = this.availability();
    const retired = this.retirement(signal, revision);
    if (retired) return retired;
    if (availability.status === 'unavailable') return availability;
    const words = Object.freeze(
      [
        ...new Set(
          prepared.groups.flatMap((group) =>
            group.candidates.map((candidate) => caseFold(candidate.text)),
          ),
        ),
      ].filter((word) => !this.ignored.has(word)),
    );
    return this.run(
      signal,
      revision,
      (owned) =>
        words.length
          ? this.gateway.checkWords(words, owned)
          : Promise.resolve({ status: 'ready', value: [] }),
      (payload) => {
        const answers = cells(payload, words.length);
        if (answers.length !== words.length || answers.some((value) => typeof value !== 'boolean'))
          throw new TypeError('spelling-check-payload');
        const rejected = new Set(words.filter((_, index) => !answers[index]));
        const issues: SpellingIssue[] = [];
        for (const group of prepared.groups)
          for (const candidate of group.candidates) {
            if (!rejected.has(caseFold(candidate.text))) continue;
            const range = decodedRangeToSource(group.scope.mapping, candidate.range);
            if (!range) throw new Error('spelling-range-unmapped');
            issues.push(
              Object.freeze({
                kind: 'spelling',
                group,
                candidate,
                range: Object.freeze(range),
              }),
            );
          }
        const snapshot = Object.freeze({
          prepared,
          gateway: this.gatewayIdentity,
          languageTag: this.languageTag,
          revision,
          dictionaryAdd: availability.dictionaryAdd,
          issues: Object.freeze(issues),
        });
        this.snapshots.add(snapshot);
        return snapshot;
      },
    );
  }
  /** Only actual current issue membership can request suggestions or dictionary operations. */
  current(snapshot: PreparedSpellingSnapshot, issue: SpellingIssue): boolean {
    return (
      !this.disposed &&
      this.snapshots.has(snapshot) &&
      snapshot.revision === this.currentRevision &&
      snapshot.issues.includes(issue)
    );
  }
  /** Lazy suggestions retain literal provider order and the actual issue; no shared suggestion cache. */
  async suggestions(
    snapshot: PreparedSpellingSnapshot,
    issue: SpellingIssue,
    signal: AbortSignal,
  ): Promise<SpellingOutcome<PreparedSpellingSuggestions>> {
    if (!this.current(snapshot, issue)) return { status: this.disposed ? 'disposed' : 'rejected' };
    const availability = this.availability();
    const retired = this.retirement(signal, snapshot.revision);
    if (retired) return retired;
    if (availability.status === 'unavailable') return availability;
    return this.run(
      signal,
      snapshot.revision,
      (owned) => this.gateway.suggest(issue.candidate.text, 8, owned),
      (payload) =>
        Object.freeze({
          snapshot,
          issue,
          suggestions: Object.freeze(cells(payload, 8).map(suggestion)),
        }),
    );
  }
  /** Word-wide local suppression is accepted before optional external delegation; backend absence cannot undo it. */
  async ignore(
    snapshot: PreparedSpellingSnapshot,
    issue: SpellingIssue,
    signal: AbortSignal,
  ): Promise<SpellingOutcome<SpellingIgnoreResult>> {
    if (!this.current(snapshot, issue)) return { status: this.disposed ? 'disposed' : 'rejected' };
    const availability = this.availability();
    const retired = this.retirement(signal, snapshot.revision);
    if (retired) return retired;
    this.ignored.add(caseFold(issue.candidate.text));
    const revision = this.currentRevision + 1;
    this.changed();
    const changedRetirement = this.retirement(signal, revision);
    if (changedRetirement) return changedRetirement;
    if (availability.status === 'unavailable' || !availability.sessionIgnore)
      return {
        status: 'ready',
        value: Object.freeze({
          revision,
          delegation: availability.status === 'unavailable' ? 'unavailable' : 'unsupported',
        }),
      };
    const delegated = await this.run(
      signal,
      revision,
      (owned) =>
        requireValue(this.gateway.ignoreForSession, 'spelling-session')(
          issue.candidate.text,
          owned,
        ),
      (payload) => {
        if (payload !== true) throw new TypeError('spelling-ignore-payload');
        return Object.freeze({ revision, delegation: 'accepted' as const });
      },
    );
    const delegationRetirement = this.retirement(signal, revision);
    if (delegationRetirement) return delegationRetirement;
    return delegated.status === 'unavailable'
      ? {
          status: 'ready',
          value: Object.freeze({ revision, delegation: 'unavailable' }),
        }
      : delegated;
  }
  /** Only successful supported persistence suppresses the word; failed adds never change native source/history. */
  async add(
    snapshot: PreparedSpellingSnapshot,
    issue: SpellingIssue,
    signal: AbortSignal,
  ): Promise<SpellingOutcome<boolean>> {
    if (!this.current(snapshot, issue)) return { status: this.disposed ? 'disposed' : 'rejected' };
    const availability = this.availability();
    const retired = this.retirement(signal, snapshot.revision);
    if (retired) return retired;
    if (availability.status === 'unavailable') return availability;
    if (!availability.dictionaryAdd) return { status: 'unavailable', code: 'unsupported' };
    const outcome = await this.run(
      signal,
      snapshot.revision,
      (owned) =>
        requireValue(this.gateway.addToDictionary, 'spelling-session')(issue.candidate.text, owned),
      (payload) => {
        if (typeof payload !== 'boolean') throw new TypeError('spelling-add-payload');
        return payload;
      },
    );
    const publicationRetirement = this.retirement(signal, snapshot.revision);
    if (publicationRetirement) return publicationRetirement;
    if (!this.current(snapshot, issue)) return { status: 'rejected' };
    if (outcome.status === 'ready' && outcome.value) {
      this.ignored.add(caseFold(issue.candidate.text));
      this.changed();
    }
    return outcome;
  }
  /** Host session teardown retires every consumer operation and releases all retained ignore/subscriber state. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const lane of [...this.active]) lane.dispose();
    this.active.clear();
    this.subscribers.clear();
    this.ignored.clear();
  }
}
