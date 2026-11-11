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
/**
 * Name the protocols and capabilities a host declares in its description.
 *
 * Each protocol is identified by its schema's message name and fixed envelope version. Each
 * capability names one behaviour a host offers; absence means the host does not offer it, and a
 * changed behaviour receives a new name instead of reusing an old one.
 */
import type { HostProtocol } from './HostProtocol.generated.js';

export type {
  HostCapability,
  HostDescriptionMessage,
  HostProtocol,
} from './HostProtocol.generated.js';

/** The panel-channel protocols SugarCubes reads, at the version each schema fixes. */
export const HOST_PROTOCOLS = Object.freeze({
  panel: Object.freeze({ name: 'sugarcubes.panel', version: 3 }),
  canvas: Object.freeze({ name: 'sugarcubes.canvas', version: 3 }),
  canvasMasks: Object.freeze({ name: 'sugarcubes.canvas.masks', version: 2 }),
  memory: Object.freeze({ name: 'sugarcubes.memory', version: 1 }),
} as const satisfies Readonly<Record<string, HostProtocol>>);

/** The capabilities SugarCubes reads from a host description. */
export const HOST_CAPABILITIES = Object.freeze({
  /** The host's configured presentation replaces the user's saved appearance while embedded. */
  hostStyles: 'presentation.host-styles',
  /** The host's configured accent is the system accent, offered as a choice beside a custom one. */
  systemAccent: 'presentation.system-accent',
  /** The host presents output images itself; the page never falls back to browser images. */
  hostOutputSurface: 'output.host-surface',
} as const);

/** One capability name SugarCubes reads. */
export type HostCapabilityName = (typeof HOST_CAPABILITIES)[keyof typeof HOST_CAPABILITIES];

/** The content of a host description, without its session envelope. */
export interface HostDescriptionContent {
  readonly protocols: readonly HostProtocol[];
  readonly capabilities: readonly { readonly name: string }[];
}
