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
/** Generated from host.schema.json by scripts/integrations/generate-host-contract.ts. */
/** Describe the schema-defined HostCapability wire value. */
export type HostCapability = { readonly name: string };
/** Describe the schema-defined HostProtocol wire value. */
export type HostProtocol = { readonly name: string; readonly version: number };
/** Describe the schema-defined HostDescriptionMessage wire value. */
export type HostDescriptionMessage = {
  readonly protocol: 'sugarcubes.host';
  readonly version: 1;
  readonly session: string;
  readonly type: 'description';
  readonly protocols: readonly HostProtocol[];
  readonly capabilities: readonly HostCapability[];
};
/** Describe every version-one host description message. */
export type HostMessage = HostDescriptionMessage;
