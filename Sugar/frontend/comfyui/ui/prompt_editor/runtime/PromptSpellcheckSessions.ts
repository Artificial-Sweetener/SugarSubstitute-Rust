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
 * One spelling session per page spellchecker, made here, beside the editor engine. The editor
 * admits only sessions its own services made, and the deployed page holds the engine in its own
 * bundle, so the page hands over its spellchecker's gateway and the session is made on this side.
 */
import {
  SpellcheckSession,
  type SpellcheckGateway,
} from '../../../../sugarprompt/services/src/index.js';

const sessions = new WeakMap<SpellcheckGateway, SpellcheckSession>();

/** The session every editor shares for `gateway`, or null without a spellchecker. */
export function spellcheckSessionFor(gateway: SpellcheckGateway | null): SpellcheckSession | null {
  if (!gateway) return null;
  let session = sessions.get(gateway);
  if (!session) {
    session = new SpellcheckSession(gateway);
    sessions.set(gateway, session);
  }
  return session;
}
