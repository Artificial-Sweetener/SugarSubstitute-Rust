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
/** Generated from memory.schema.json by scripts/integrations/generate-memory-contract.ts. */
/** Describe the schema-defined MemoryLevel wire value. */
export type MemoryLevel = 'normal' | 'elevated' | 'high' | 'critical';
/** Describe the schema-defined MemoryPreference wire value. */
export type MemoryPreference =
  | { readonly mode: 'share'; readonly percent: number }
  | { readonly mode: 'fixed'; readonly bytes: number };
/** Describe the schema-defined MemoryConsumer wire value. */
export type MemoryConsumer = { readonly id: string; readonly bytes: number };
/** Describe the schema-defined MemoryUsageMessage wire value. */
export type MemoryUsageMessage = {
  readonly protocol: 'sugarcubes.memory';
  readonly version: 1;
  readonly session: string;
  readonly type: 'usage';
  readonly usedBytes: number;
  readonly demandBytes: number;
  readonly consumers: readonly MemoryConsumer[];
};
/** Describe the schema-defined MemoryPreferenceMessage wire value. */
export type MemoryPreferenceMessage = {
  readonly protocol: 'sugarcubes.memory';
  readonly version: 1;
  readonly session: string;
  readonly type: 'preference';
  readonly preference: MemoryPreference;
};
/** Describe the schema-defined MemoryAllowanceMessage wire value. */
export type MemoryAllowanceMessage = {
  readonly protocol: 'sugarcubes.memory';
  readonly version: 1;
  readonly session: string;
  readonly type: 'allowance';
  readonly generation: number;
  readonly level: MemoryLevel;
  readonly allowanceBytes: number;
  readonly ceilingBytes: number;
  readonly totalBytes: number;
  readonly preference: MemoryPreference;
};
/** Describe the schema-defined MemoryClientMessage wire value. */
export type MemoryClientMessage = MemoryUsageMessage | MemoryPreferenceMessage;
/** Describe every version-one memory budget message. */
export type MemoryMessage = MemoryUsageMessage | MemoryPreferenceMessage | MemoryAllowanceMessage;
