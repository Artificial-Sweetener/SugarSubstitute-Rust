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
/** Validate and capture only prepared scheduling authority; arbitrary host graphs remain outside services and the editor. */
import type { LoraCatalogItem } from './contracts.js';
import type {
  LoraSchedulingContextInput,
  PreparedLoraSchedulingContext,
} from './scheduled-contracts.js';
import { loraItemKeys, loraRecord, loraText, readLoraItemFields } from './metadata-validation.js';

const preparedContexts = new WeakSet<object>();

/** Copy bounded ordered model groups through the same metadata boundary as catalogs and resource replies. */
function modelGroup(value: unknown): readonly LoraCatalogItem[] {
  if (!Array.isArray(value) || value.length > 1000)
    throw new TypeError('Invalid LoRA scheduling models');
  return Object.freeze(
    value.map((item: unknown) => readLoraItemFields(loraRecord(item, loraItemKeys))),
  );
}

/** Capture one finite host snapshot without requests, graph traversal or a second catalog owner. */
export function prepareLoraSchedulingContext(
  input: LoraSchedulingContextInput,
): PreparedLoraSchedulingContext {
  const raw = loraRecord(input, [
    'revision',
    'catalogRevision',
    'mode',
    'supported',
    'upstream',
    'fields',
  ]);
  if (!['library', 'workflow'].includes(raw.mode as string) || typeof raw.supported !== 'boolean')
    throw new TypeError('Invalid LoRA scheduling authority');
  const upstream = modelGroup(raw.upstream),
    fields = modelGroup(raw.fields);
  if (raw.mode === 'library' && (upstream.length || fields.length))
    throw new TypeError('Invalid library scheduling context');
  const result: PreparedLoraSchedulingContext = Object.freeze({
    revision: loraText(raw.revision, 256),
    catalogRevision: loraText(raw.catalogRevision, 256),
    mode: raw.mode as 'library' | 'workflow',
    supported: raw.supported,
    upstream,
    fields,
  });
  preparedContexts.add(result);
  return result;
}

/** Preparation identity cannot be forged by a structurally similar mutable object. */
export function assertPreparedLoraSchedulingContext(context: PreparedLoraSchedulingContext): void {
  if (!preparedContexts.has(context)) throw new TypeError('Unprepared LoRA scheduling context');
}
