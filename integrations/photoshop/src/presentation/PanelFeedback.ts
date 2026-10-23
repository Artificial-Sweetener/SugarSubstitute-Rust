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
/** Translate connection states into concise native panel feedback. */
import type { PanelState } from '../../../shared/PanelSession.js';

/** Describe native status text independently of UXP's DOM declarations. */
export interface PanelFeedback {
  readonly title: string;
  readonly message: string;
  readonly retry: boolean;
  readonly visible: boolean;
}

/** Explain loading and recovery while the acknowledged frontend owns connected presentation. */
export function panelFeedback(state: PanelState): PanelFeedback {
  if (state === 'connected') return { title: '', message: '', retry: false, visible: false };
  if (state === 'loading')
    return {
      title: 'Loading SugarCubes',
      message: 'Connecting to ComfyUI and preparing the panel.',
      retry: false,
      visible: true,
    };
  if (state === 'disconnected')
    return {
      title: 'ComfyUI disconnected',
      message: 'Waiting for ComfyUI to reconnect. Retry reloads the panel.',
      retry: true,
      visible: true,
    };
  return {
    title: 'SugarCubes could not connect',
    message: 'Start ComfyUI at http://127.0.0.1:8188 with SugarCubes enabled, then retry.',
    retry: true,
    visible: true,
  };
}
