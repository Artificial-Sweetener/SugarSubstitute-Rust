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
/** Invoke the execution-owned prepared asset projection for an ordinary Comfy prompt. */
import type { WorkflowPreparedInput } from '../../execution/domain/WorkflowPreparation.js';
import { isRecord } from '../../types/common.js';

/** Keep graph projection policy on the same backend owner used by direct Cube execution. */
export class PreparedInputPromptClient {
  /** Inject same-origin transport for bounded, deterministic failure behavior. */
  constructor(private readonly fetch: (input: string, init: RequestInit) => Promise<Response>) {}

  /** Return the canonical derived prompt while preserving authored workflow data and loader settings. */
  async prepare(prompt: unknown, assets: readonly WorkflowPreparedInput[]): Promise<unknown> {
    const response = await this.fetch('/sugarcubes/v2/canvas-inputs/prepare', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ prompt, assets }),
      signal: AbortSignal.timeout(30000),
    });
    const payload: unknown = await response.json();
    if (!response.ok || !isRecord(payload) || !isRecord(payload.prompt))
      throw new Error(
        isRecord(payload) && typeof payload.error === 'string'
          ? payload.error
          : 'Input preparation is unavailable.',
      );
    return payload.prompt;
  }
}
