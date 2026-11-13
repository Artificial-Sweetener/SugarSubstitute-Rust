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
/** Provide a manually settled public history transport for race and cancellation proofs. */

export interface WorkflowHistoryRequest {
  readonly path: string;
  readonly signal: AbortSignal;
  respond(payload: unknown, ok?: boolean): void;
  fail(error: unknown): void;
}

/** Leave request cancellation observable while allowing deliberately late responses. */
export class WorkflowHistoryApi {
  readonly requests: WorkflowHistoryRequest[] = [];
  /** Capture only the exact requested route and forwarded abort signal. */
  fetchApi(path: string, options: { readonly signal: AbortSignal }): Promise<unknown> {
    return new Promise((resolve, reject) => {
      this.requests.push({
        path,
        signal: options.signal,
        respond: (payload, ok = true) => {
          resolve({ ok, json: async (): Promise<unknown> => payload });
        },
        fail: reject,
      });
    });
  }
  /** Select a captured request with a useful failure when the boundary was never called. */
  request(index: number): WorkflowHistoryRequest {
    const request = this.requests[index];
    if (!request) throw new Error(`Missing history request ${index}`);
    return request;
  }
}

/** Drain promise continuations without advancing wall-clock timers. */
export async function settleWorkflowHistory(): Promise<void> {
  for (let step = 0; step < 16; step++) await Promise.resolve();
}
