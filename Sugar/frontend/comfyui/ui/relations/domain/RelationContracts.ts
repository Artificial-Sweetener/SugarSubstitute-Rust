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
/** Define renderer-independent editable surfaces and durable workflow relations. */
export type FieldValue = string | number | boolean;
export type FieldDomain =
  | { kind: 'choice'; values: readonly string[] }
  | { kind: 'number'; minimum: number; maximum: number; step: number; integer: boolean }
  | { kind: 'string' }
  | { kind: 'boolean' };

export interface FieldContract {
  readonly id: string;
  readonly nodeId: string;
  readonly cubeId: string | null;
  readonly label: string;
  readonly name: string;
  readonly semantic: string;
  readonly domain: FieldDomain;
  readonly value: FieldValue;
  readonly seedMode?: RelationSeedMode;
  readonly linkable?: boolean;
}

export interface NodeContract {
  readonly id: string;
  readonly cubeId: string;
  readonly cubeLabel: string;
  readonly label: string;
  readonly classType: string;
  readonly configurationIdentity: string;
  readonly promptRole?: 'positive' | 'negative' | null;
  readonly fields: readonly FieldContract[];
  readonly linkable: boolean;
  readonly mode?: number;
}

export type RelationSeedMode = 'fixed' | 'randomize' | 'increment' | 'decrement';
export interface GlobalOverrideState {
  enabled: boolean;
  value: FieldValue;
  seedMode: RelationSeedMode;
}

export interface RelationState {
  schema: 1;
  overrides: Record<string, GlobalOverrideState>;
  fieldLinks: Record<string, string | null>;
  nodeLinks: Record<string, string | null>;
  locals: Record<string, FieldValue>;
  localSeedModes: Record<string, RelationSeedMode>;
  localNodeModes: Record<string, number>;
}

export interface OverrideGroup {
  readonly key: string;
  readonly label: string;
  readonly members: readonly FieldContract[];
  readonly domain: FieldDomain;
  readonly enabled: boolean;
  readonly value: FieldValue;
  readonly seedMode: RelationSeedMode;
}

export interface ResolvedField {
  readonly field: FieldContract;
  readonly value: FieldValue;
  readonly source: string | null;
  readonly override: string | null;
}

/** Create isolated workflow state for unconfigured relation metadata. */
export function emptyRelationState(): RelationState {
  return {
    schema: 1,
    overrides: {},
    fieldLinks: {},
    nodeLinks: {},
    locals: {},
    localSeedModes: {},
    localNodeModes: {},
  };
}
