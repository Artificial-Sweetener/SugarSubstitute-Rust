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
/** Prepared model-page intents cross one finite validated host boundary, with no internal browser or URL acquisition. */
import { LatestRequest } from '../requests/latest.js';
import type { LoraModelIdentity, LoraResource } from './contracts.js';
import { loraRecord, readLoraModelIdentity, readLoraModelPageUrl } from './metadata-validation.js';

/** Canonical model identity and validated page URL are independent of authored aliases and display labels. */
export interface LoraPageRequest {
  readonly model: LoraModelIdentity;
  readonly url: string;
  readonly label: 'Go to CivitAI page' | 'Open model page';
}

/** Expected host outcomes distinguish successful opening from cancellation or unavailable integration. */
export interface LoraPageResult {
  readonly status: 'opened' | 'cancelled' | 'unavailable';
}

/** The host opens its prepared external URL and respects abort; the editor never chooses a browser or navigates itself. */
export interface LoraPageAdapter {
  open(request: LoraPageRequest, signal: AbortSignal): Promise<LoraPageResult>;
}

/** Finite retirement additionally distinguishes a destroyed component lifetime. */
export type LoraPageOutcome = LoraPageResult | { readonly status: 'disposed' };

const preparedRequests = new WeakSet<LoraPageRequest>();

/** Prepare outside input/paint from validated metadata; absent or unresolved pages do not manufacture an action. */
export function prepareLoraPage(resource: LoraResource): LoraPageRequest | null {
  if (resource.resolution !== 'found' || resource.modelPageUrl === null) return null;
  const url = readLoraModelPageUrl(resource.modelPageUrl),
    host = new URL(url).hostname.toLowerCase();
  const request: LoraPageRequest = Object.freeze({
    model: readLoraModelIdentity(resource.model),
    url,
    label:
      host === 'civitai.com' || host.endsWith('.civitai.com')
        ? 'Go to CivitAI page'
        : 'Open model page',
  });
  preparedRequests.add(request);
  return request;
}

/** One host lane consumes only immutable prepared requests; no menu/source state or URL cache is retained. */
export class LoraPageSession {
  private readonly lane = new LatestRequest();
  private adapter: LoraPageAdapter | undefined;
  private disposed = false;

  /** Adapter replacement creates a new complete external capability lifetime. */
  constructor(adapter: LoraPageAdapter | undefined) {
    this.adapter = adapter;
  }

  /** New actions supersede old host work, and unknown replies cannot masquerade as success. */
  async open(request: LoraPageRequest): Promise<LoraPageOutcome> {
    if (this.disposed) return { status: 'disposed' };
    if (!preparedRequests.has(request)) throw new TypeError('Invalid prepared LoRA page request');
    const adapter = this.adapter;
    if (!adapter) return { status: 'unavailable' };
    const outcome = await this.lane.run((signal) => adapter.open(request, signal));
    if (outcome.status !== 'ready') return outcome;
    const raw = loraRecord(outcome.value, ['status']);
    if (raw.status !== 'opened' && raw.status !== 'cancelled' && raw.status !== 'unavailable')
      throw new TypeError('Invalid LoRA page outcome');
    return Object.freeze({ status: raw.status });
  }

  /** Source/context/resource retirement settles pending work even when the host ignores abort. */
  cancel(): void {
    this.lane.cancel();
  }

  /** Release host references and owned pending work before the native view disappears. */
  dispose(): void {
    this.disposed = true;
    this.adapter = undefined;
    this.lane.dispose();
  }
}
