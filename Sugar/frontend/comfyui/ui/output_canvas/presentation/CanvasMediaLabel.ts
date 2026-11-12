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
/** Name output variants consistently across detail announcements and grid accessibility. */
import type { WorkflowMediaKind } from '../../execution/domain/WorkflowOutput.js';

/** Preserve still-image terminology while identifying the selected media's actual type. */
export function canvasMediaLabel(kind: WorkflowMediaKind): string {
  switch (kind) {
    case 'model':
      return '3D model';
    case 'video':
      return 'video';
    case 'animated':
      return 'animation';
    case 'image':
      return 'image';
    case 'unsupported':
      return 'output';
  }
}
