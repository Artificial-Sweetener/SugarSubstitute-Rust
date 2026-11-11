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
/** Supply explicit nonvisual host boundaries to native controller contracts; browser tests own actual surfaces. */
import type { PromptMenuFactory } from '../../../frontend/sugarprompt/editor/src/menu-presentation.js';
import type { TooltipTransportFactory } from '../../../frontend/sugarprompt/editor/src/tooltip-transport.js';
/** Headless controller proofs never request native popup rendering. */
export const editorPresentations: {
  menuPresentation: PromptMenuFactory;
  tooltipTransport: TooltipTransportFactory;
} = {
  menuPresentation: () => ({
    show: () => {
      throw new Error('Unexpected popup in controller proof');
    },
    hide: () => undefined,
    dispose: () => undefined,
  }),
  tooltipTransport: () => ({
    show: () => undefined,
    hide: () => undefined,
    dispose: () => undefined,
  }),
};
