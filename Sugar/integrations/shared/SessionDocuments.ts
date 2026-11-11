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
/** Admit one Comfy-owned current/baseline snapshot without interpreting graph semantics. */
import type { SessionFailure } from '../contracts/SessionProtocol.generated.js';
import { isSessionDocument } from './SessionSnapshotValidation.generated.js';
import {
  SESSION_DOCUMENT_BYTES,
  sessionGraph,
  sessionJsonScalars,
  utf8Bytes,
} from './SessionPrimitives.js';

/** Bound combined UTF-8 graph bytes before either graph is parsed, then bound the full envelope. */
export function sessionDocumentError(value: unknown): SessionFailure['code'] | null {
  if (!isSessionDocument(value) || !sessionJsonScalars(value)) return 'invalid';
  if (
    utf8Bytes(value.graphJson) + utf8Bytes(value.baselineGraphJson ?? '') >
    SESSION_DOCUMENT_BYTES
  )
    return 'too-large';
  if (
    !value.name ||
    (value.path !== null && !value.path) ||
    (value.temporary && value.path !== null) ||
    !sessionGraph(value.graphJson) ||
    (value.baselineGraphJson !== null && !sessionGraph(value.baselineGraphJson))
  )
    return 'invalid';
  return utf8Bytes(JSON.stringify(value)) > SESSION_DOCUMENT_BYTES ? 'too-large' : null;
}
