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
/** Authorize versioned transport messages before decoding their payloads. */

import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';

export interface CanvasMessageAuthority {
  readonly origin: string;
  readonly address: CanvasAttachmentAddress;
}

/** Match a configured origin and every attachment coordinate without trusting injected objects. */
export function authorizeCanvasMessage(
  value: unknown,
  origin: unknown,
  authority: CanvasMessageAuthority,
): value is UnknownRecord {
  if (!authority.origin || authority.origin === 'null' || origin !== authority.origin) return false;
  if (!isRecord(value) || value.schema !== 1 || !isRecord(value.address)) return false;
  const address = value.address;
  const expected = authority.address;
  return (
    address.sessionId === expected.sessionId &&
    address.viewId === expected.viewId &&
    address.windowId === expected.windowId &&
    address.attachmentId === expected.attachmentId &&
    address.generation === expected.generation
  );
}
