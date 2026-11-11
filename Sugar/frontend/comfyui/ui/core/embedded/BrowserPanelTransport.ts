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
/** Define browser delivery for the existing portable editor protocols. */
import type { PanelMessage } from './PanelRuntime.js';
import type { CanvasMessage, CanvasMaskMessage } from './CanvasRuntime.js';
import type { MemoryClientMessage } from '../memory/MemoryHostProtocol.js';

/** Keep editor transport mechanics outside the embedded application lifecycle. */
export interface BrowserPanelTransport {
  readonly session: string;
  /** Choose whether host presentation temporarily overrides Comfy's saved preferences. */
  readonly preferencePolicy?: 'host' | 'saved';
  /** Prefer an embedded host surface before capability negotiation; grants no native authority. */
  readonly outputSurfacePreference?: 'host';
  /** Whether the host joins this page to its application memory budget. */
  readonly memoryBudget?: true;
  send(message: PanelMessage | CanvasMessage | CanvasMaskMessage | MemoryClientMessage): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/** Identify one navigation without accepting duplicate or partially matched identities. */
export function panelNavigation(search: string): { host: string; session: string } | null {
  const query = new URLSearchParams(search);
  const host = query.get('sc-panel-host');
  const session = query.get('sc-panel-session');
  if (
    !host ||
    !session ||
    !/^[a-zA-Z0-9._-]{1,128}(?![\s\S])/.test(host) ||
    !/^[a-zA-Z0-9-]{16,80}(?![\s\S])/.test(session) ||
    query.getAll('sc-panel-host').length !== 1 ||
    query.getAll('sc-panel-session').length !== 1
  )
    return null;
  return { host, session };
}
