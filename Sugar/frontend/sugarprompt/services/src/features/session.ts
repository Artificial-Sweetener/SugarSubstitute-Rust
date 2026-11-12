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
/** A finite host preference lifetime serializes persistence and owns immutable published snapshots. */
import { requireValue } from '../../../core/src/required.js';
import type {
  FeaturePreferences,
  PreferenceAdapter,
  PreferenceOutcome,
  PreferencePatch,
} from './contracts.js';
import {
  applyPreferencePatch,
  defaultPreferences,
  readPreferences,
  validatePreferencePatch,
} from './preferences.js';
/** Storage state is observable independently of current preference defaults. */
export type PreferencePersistence = 'loading' | 'ready' | 'unavailable' | 'invalid' | 'failed';
/** Queue validated preference changes after load; failed writes preserve the last accepted snapshot. */
export class PreferenceSession {
  private readonly cancellation = new AbortController();
  private current: FeaturePreferences = defaultPreferences;
  private state: PreferencePersistence = 'loading';
  private queue: Promise<void> = Promise.resolve();
  private initial: Promise<PreferenceOutcome> | undefined;
  private loaded = false;
  private readonly adapter: PreferenceAdapter | undefined;
  private readonly observe: (
    preferences: FeaturePreferences,
    persistence: PreferencePersistence,
  ) => void;
  /** Store one injected host boundary and optional observer for this finite component lifetime. */
  constructor(
    adapter: PreferenceAdapter | undefined,
    observe: (
      preferences: FeaturePreferences,
      persistence: PreferencePersistence,
    ) => void = () => {},
  ) {
    this.adapter = adapter;
    this.observe = observe;
  }
  /** Read accepted state; callers receive frozen data, never the request queue. */
  snapshot(): {
    readonly preferences: FeaturePreferences;
    readonly persistence: PreferencePersistence;
  } {
    return { preferences: this.current, persistence: this.state };
  }
  /** Load at most once; coalesced callers share a request and decoded data is validated before use. */
  load(): Promise<PreferenceOutcome> {
    if (this.initial) return this.initial;
    this.initial = this.enqueue(async () => {
      if (!this.adapter) return this.unavailable();
      const result = await this.request((signal) =>
        requireValue(this.adapter, 'session').read(signal),
      );
      if (!result || !['ready', 'unavailable', 'disposed'].includes(result.status))
        throw new TypeError('Invalid preference read outcome');
      if (result.status === 'disposed') return result;
      if (this.cancellation.signal.aborted) return { status: 'disposed' };
      if (result.status === 'unavailable') return this.unavailable();
      const preferences = readPreferences(result.value);
      if (!preferences) {
        this.state = 'invalid';
        this.publish();
        return { status: 'invalid' };
      }
      this.current = preferences;
      this.loaded = true;
      this.state = 'ready';
      this.publish();
      return { status: 'ready', preferences };
    });
    return this.initial;
  }
  /** Serialize read/modify/save so simultaneous independent settings edits cannot lose each other. */
  update(patch: PreferencePatch): Promise<PreferenceOutcome> {
    const valid = validatePreferencePatch(patch);
    if (!valid) return Promise.resolve({ status: 'invalid' });
    const captured = applyPreferencePatch(defaultPreferences, valid);
    const copy: PreferencePatch = {
      ...valid,
      ...(valid.allowed
        ? {
            allowed: Object.fromEntries(
              Object.keys(valid.allowed).map((id) => [
                id,
                captured.allowed[id as keyof typeof captured.allowed],
              ]),
            ),
          }
        : {}),
    };
    return this.load().then(() =>
      this.enqueue(async () => {
        if (!this.loaded || !this.adapter)
          return {
            status: this.state === 'invalid' ? 'invalid' : 'unavailable',
          };
        const preferences = applyPreferencePatch(this.current, copy);
        const result = await this.request((signal) =>
          requireValue(this.adapter, 'session').write(preferences, signal),
        );
        if (!result || !['saved', 'unavailable', 'disposed'].includes(result.status))
          throw new TypeError('Invalid preference write outcome');
        if (result.status === 'disposed') return result;
        if (this.cancellation.signal.aborted) return { status: 'disposed' };
        if (result.status === 'unavailable') return this.unavailable();
        this.current = preferences;
        this.state = 'ready';
        this.publish();
        return { status: 'saved', preferences };
      }),
    );
  }
  /** Cancel running work and prevent queued operations or late publication after teardown. */
  dispose(): void {
    this.cancellation.abort();
  }
  /** Recover only expected cancellation; unexpected errors remain rejected and observable. */
  private enqueue(operation: () => Promise<PreferenceOutcome>): Promise<PreferenceOutcome> {
    const result = this.queue.then(async () => {
      if (this.cancellation.signal.aborted) return { status: 'disposed' } as const;
      try {
        return await operation();
      } catch (error) {
        if (this.cancellation.signal.aborted) return { status: 'disposed' } as const;
        this.state = 'failed';
        this.publish();
        throw error;
      }
    });
    // The queue continues after a failed operation; its returned promise still reports that failure.
    this.queue = result.then(
      () => {},
      () => {},
    );
    return result;
  }
  /** Settle service-owned requests on disposal even when a faulty host ignores cancellation. */
  private request<T>(invoke: (signal: AbortSignal) => Promise<T>): Promise<
    | T
    | {
        readonly status: 'disposed';
      }
  > {
    const signal = this.cancellation.signal;
    if (signal.aborted) return Promise.resolve({ status: 'disposed' });
    return new Promise((resolve, reject) => {
      /** Abort releases the listener and prevents late host output from entering the service queue. */
      const abort = (): void => {
        resolve({ status: 'disposed' });
      };
      signal.addEventListener('abort', abort, { once: true });
      /** Resolve or reject the host boundary and always release the owned cancellation listener. */
      const complete = (value: T): void => {
        signal.removeEventListener('abort', abort);
        resolve(value);
      };
      /** Host errors remain observable unless this lifetime has already been cancelled. */
      const failure = (error: unknown): void => {
        signal.removeEventListener('abort', abort);
        if (signal.aborted) resolve({ status: 'disposed' });
        // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- Preserve the external host's original thrown value across this asynchronous boundary.
        else reject(error);
      };
      try {
        void invoke(signal).then(complete, failure);
      } catch (error) {
        failure(error);
      }
    });
  }
  /** Publish storage unavailability without discarding accepted preferences. */
  private unavailable(): PreferenceOutcome {
    this.state = 'unavailable';
    this.publish();
    return { status: 'unavailable' };
  }
  /** Finite publication is owned here, including callbacks after awaited host requests. */
  private publish(): void {
    if (!this.cancellation.signal.aborted) this.observe(this.current, this.state);
  }
}
