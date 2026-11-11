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
/** Captured host scene authority and one finite queue lane retain no prompt bodies. */
import { normalizeSceneTitle } from '../../../core/src/index.js';
import { LatestRequest } from '../requests/latest.js';

/** A host-owned workflow publication is valid for one native exact-source revision. */
export interface SceneQueueAdapter {
  readonly revision: string;
  readonly sourceRevision: number;
  readonly queueableKeys: readonly string[];
  queue(request: SceneQueueRequest, signal: AbortSignal): Promise<SceneQueueResult>;
}

/** Source-free queue intent carries captured workflow authority and the clicked normalized scene identity. */
export interface SceneQueueRequest {
  readonly revision: string;
  readonly sourceRevision: number;
  readonly sceneKey: string;
}

/** Expected host admission outcomes remain distinct from finite component retirement. */
export interface SceneQueueResult {
  readonly status: 'queued' | 'cancelled' | 'unavailable';
}
/** Disposal prohibits subsequent invocation and publication. */
export type SceneQueueOutcome = SceneQueueResult | { readonly status: 'disposed' };

/** Immutable publication reuses actual prepared requests through bounded keyed lookup. */
export interface PreparedSceneQueue {
  readonly revision: string;
  readonly sourceRevision: number;
  readonly queueableKeys: readonly string[];
  request(key: string): SceneQueueRequest | null;
}

/** Private authority captures the adapter receiver/method and exact request provenance once. */
interface QueueAuthority {
  readonly requests: ReadonlySet<SceneQueueRequest>;
  readonly execute: (request: SceneQueueRequest, signal: AbortSignal) => Promise<SceneQueueResult>;
}
const authorities = new WeakMap<PreparedSceneQueue, QueueAuthority>();

/** Admit explicit absence or the exact service-prepared queue authority before any consumer commits state. */
export function validatePreparedSceneQueue(
  value: unknown,
): asserts value is PreparedSceneQueue | null {
  if (
    value !== null &&
    (typeof value !== 'object' || !value || !authorities.has(value as PreparedSceneQueue))
  )
    throw new TypeError('Invalid prepared scene queue capability');
}

/** Validate and capture normalized host keys outside native input; absence is represented by a null public prop. */
export function prepareSceneQueue(adapter: SceneQueueAdapter): PreparedSceneQueue {
  const { revision, sourceRevision, queueableKeys, queue } = adapter;
  if (
    typeof revision !== 'string' ||
    !revision ||
    revision.length > 256 ||
    !Number.isSafeInteger(sourceRevision) ||
    sourceRevision < 0 ||
    !Array.isArray(queueableKeys) ||
    queueableKeys.length > 10000 ||
    typeof queue !== 'function'
  )
    throw new TypeError('Invalid scene queue publication');
  let total = 0;
  const keys = Array.from(queueableKeys, (key: unknown) => {
    if (typeof key !== 'string' || !key || key.length > 1000000)
      throw new TypeError('Scene queue requires normalized nonempty keys');
    total += key.length;
    if (total > 8000000) throw new RangeError('Scene queue preparation limit exceeded');
    if (normalizeSceneTitle(key) !== key)
      throw new TypeError('Scene queue requires normalized nonempty keys');
    return key;
  });
  if (new Set(keys).size !== keys.length) throw new TypeError('Duplicate scene queue key');
  const requests = new Map(
    keys.map((sceneKey) => [sceneKey, Object.freeze({ revision, sourceRevision, sceneKey })]),
  );
  const prepared: PreparedSceneQueue = Object.freeze({
    revision,
    sourceRevision,
    queueableKeys: Object.freeze(keys),
    /** Lookup reuses the captured request without retaining source or traversing the host catalog. */
    request: (key: string) => requests.get(key) ?? null,
  });
  authorities.set(prepared, {
    requests: new Set(requests.values()),
    execute: (request, signal) => queue.call(adapter, request, signal),
  });
  return prepared;
}

/** Latest accepted scene owns one external request and settles even when its host ignores abort. */
export class SceneQueueSession {
  private readonly lane = new LatestRequest();
  private authority: QueueAuthority | undefined;
  private disposed = false;

  /** A capability replacement creates a complete new host lifetime; forged capabilities reject. */
  constructor(capability: PreparedSceneQueue | null) {
    validatePreparedSceneQueue(capability);
    this.authority = capability ? authorities.get(capability) : undefined;
  }

  /** Accept only requests prepared by this exact publication; current malformed replies and failures remain observable. */
  async queue(request: SceneQueueRequest): Promise<SceneQueueOutcome> {
    if (this.disposed) return { status: 'disposed' };
    const authority = this.authority;
    if (!authority) return { status: 'unavailable' };
    if (!authority.requests.has(request))
      throw new TypeError('Invalid prepared scene queue request');
    const outcome = await this.lane.run((signal) => authority.execute(request, signal));
    if (outcome.status !== 'ready') return outcome;
    const value: unknown = outcome.value;
    if (
      !value ||
      typeof value !== 'object' ||
      Array.isArray(value) ||
      Object.keys(value).join() !== 'status'
    )
      throw new TypeError('Invalid scene queue outcome');
    const status = (value as Record<string, unknown>).status;
    if (status !== 'queued' && status !== 'cancelled' && status !== 'unavailable')
      throw new TypeError('Invalid scene queue outcome');
    return Object.freeze({ status });
  }

  /** Source/context retirement releases pending work without revoking the immutable host metadata. */
  cancel(): void {
    this.lane.cancel();
  }
  /** Release the captured host receiver and settle owned work before native teardown. */
  dispose(): void {
    this.disposed = true;
    this.authority = undefined;
    this.lane.dispose();
  }
}
