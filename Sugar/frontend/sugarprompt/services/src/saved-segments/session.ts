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
/** Expected external availability and retirement have distinct finite outcomes. */
import { LatestRequest } from '../requests/latest.js';
import { strip } from '../../../core/src/prompt/text.js';
import { captureReply, captureText } from './capture.js';
import type {
  SavedSegmentAdapter,
  SavedSegmentCatalog,
  SavedSegmentContext,
  SavedSegmentScope,
} from './contracts.js';
import {
  captureSavedSegmentContext,
  prepareSavedSegmentCatalog,
  captureSavedSegmentAdapter,
} from './preparation.js';
export type SavedSegmentOutcome =
  | { readonly status: 'ready'; readonly catalog: SavedSegmentCatalog }
  | { readonly status: 'saved' | 'unavailable' | 'cancelled' | 'disposed' };
/** One mounted owner runs read/save work; retirement settles even cancellation-ignoring providers. */
export class SavedSegmentSession {
  private readonly adapter: SavedSegmentAdapter;
  private readonly lane = new LatestRequest();
  readonly context: SavedSegmentContext;
  /** Capture the host context before accepting any storage data. */
  constructor(adapter: SavedSegmentAdapter, context: SavedSegmentContext) {
    this.adapter = captureSavedSegmentAdapter(adapter);
    this.context = captureSavedSegmentContext(context);
  }
  /** A finite deadline aborts the provider and settles as unavailable without publishing late data. */
  private async run<T>(
    work: (signal: AbortSignal) => Promise<T>,
  ): Promise<
    | { readonly status: 'ready'; readonly value: T }
    | { readonly status: 'unavailable' | 'cancelled' | 'disposed' }
  > {
    let expired = false;
    const timer = setTimeout(() => {
      expired = true;
      this.lane.cancel();
    }, 10000);
    try {
      const outcome = await this.lane.run(work);
      return expired ? { status: 'unavailable' } : outcome;
    } finally {
      clearTimeout(timer);
    }
  }
  /** Listing traversal and scope matching happen once outside native menu input. */
  async read(): Promise<SavedSegmentOutcome> {
    const outcome = await this.run(async (signal): Promise<SavedSegmentOutcome> => {
      const raw: unknown = await this.adapter.read(this.context, signal);
      if (signal.aborted) return { status: 'cancelled' };
      const reply = captureReply(raw, ['ready', 'unavailable']);
      const result: SavedSegmentOutcome =
        reply.status === 'unavailable'
          ? Object.freeze({ status: 'unavailable' })
          : Object.freeze({
              status: 'ready',
              catalog: prepareSavedSegmentCatalog(this.context, reply.segments),
            });
      return signal.aborted ? { status: 'cancelled' } : result;
    });
    if (outcome.status !== 'ready') return outcome;
    return outcome.value;
  }
  /** Exact captured text and literal label reach only an admitted current scope. */
  async save(label: string, text: string, scope: SavedSegmentScope): Promise<SavedSegmentOutcome> {
    const capturedLabel = strip(captureText(label, 2048)),
      capturedText = captureText(text, 8_000_000);
    if (!this.context.scopes.includes(scope)) throw new TypeError('invalid-saved-segment-save');
    const outcome = await this.run(async (signal): Promise<SavedSegmentOutcome> => {
      const raw: unknown = await this.adapter.save(
        Object.freeze({ label: capturedLabel, text: capturedText, scope }),
        signal,
      );
      if (signal.aborted) return { status: 'cancelled' };
      const reply = captureReply(raw, ['saved', 'unavailable']);
      const status = reply.status === 'saved' ? 'saved' : 'unavailable';
      return signal.aborted ? { status: 'cancelled' } : Object.freeze({ status });
    });
    if (outcome.status !== 'ready') return outcome;
    return outcome.value;
  }
  /** Supersession/source retirement releases the current host request immediately. */
  cancel(): void {
    this.lane.cancel();
  }
  /** Teardown prevents subsequent provider invocation and releases held request data. */
  dispose(): void {
    this.lane.dispose();
  }
}
