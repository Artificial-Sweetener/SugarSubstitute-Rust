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
/** Declare what the Photoshop plugin serves to the SugarCubes page it hosts. */
import {
  HOST_CAPABILITIES,
  HOST_PROTOCOLS,
  type HostDescriptionContent,
} from '../../../contracts/HostDescription.js';

/** The plugin serves the panel, canvas and mask protocols, and Photoshop's presentation replaces saved styles. */
export const PHOTOSHOP_HOST_DESCRIPTION: HostDescriptionContent = Object.freeze({
  protocols: [HOST_PROTOCOLS.panel, HOST_PROTOCOLS.canvas, HOST_PROTOCOLS.canvasMasks],
  capabilities: [{ name: HOST_CAPABILITIES.hostStyles }],
});
