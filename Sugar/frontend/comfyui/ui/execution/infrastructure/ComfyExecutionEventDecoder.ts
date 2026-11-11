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
/** Validate Comfy event details and authoritative execution identifiers at the host boundary. */

import { isRecord, type UnknownRecord } from '../../types/common.js';
import { isCubeOutputExecutionId } from '../../cube/execution/CubeOutputExecutionIdentity.js';
import { promptId, type PromptId } from '../domain/ExecutionIdentity.js';
import type { WorkflowFinalEvent } from '../domain/WorkflowExecutionEvent.js';
import { decodeComfyMedia } from './ComfyMediaDecoder.js';

/** Extract structured event details without accepting arrays or opaque host instances as media records. */
export function comfyEventDetail(event: unknown): UnknownRecord | null {
  return isRecord(event) && isRecord(event.detail) && !Array.isArray(event.detail)
    ? event.detail
    : null;
}

/** Validate a backend job ID while retaining its transport spelling. */
export function decodeComfyPromptId(value: unknown): PromptId | null {
  return typeof value === 'string' && value.trim() && value.length <= 4096 ? promptId(value) : null;
}

/** Prefer existing authoritative cube sink identities, then exact full execution paths over display fallbacks. */
export function decodeComfyExecutionPaths(values: readonly unknown[]): readonly string[] {
  const paths = values.flatMap((value) =>
    typeof value === 'string' && value.length > 0 && value.length <= 4096
      ? [value]
      : typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
        ? [String(value)]
        : [],
  );
  return Object.freeze([...new Set([...paths.filter(isCubeOutputExecutionId), ...paths])]);
}

/** Decode all workflow finals rather than limiting ingestion to exported cube markers. */
export function decodeComfyFinalEvent(event: unknown): WorkflowFinalEvent | null {
  const detail = comfyEventDetail(event);
  if (!detail) return null;
  const id = decodeComfyPromptId(detail.prompt_id);
  const media = decodeComfyMedia(detail.output);
  const paths = decodeComfyExecutionPaths([detail.node, detail.display_node]);
  if (
    !id ||
    !media ||
    !paths.length ||
    (detail.merge !== undefined && typeof detail.merge !== 'boolean')
  )
    return null;
  return Object.freeze({
    promptId: id,
    executionPaths: paths,
    ...media,
    merge: detail.merge === true,
  });
}
