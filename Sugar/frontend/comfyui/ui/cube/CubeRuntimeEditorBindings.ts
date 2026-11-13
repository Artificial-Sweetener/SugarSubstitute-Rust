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
/** Compose Cube editor-entry policy at the Comfy runtime boundary. */

import type { CubeEditorMetadataHudActions } from '../surface/CubeEditorMetadataHud.js';
import type { CubeSurfacePresenterOptions } from '../surface/CubeSurfacePresenter.js';

interface RuntimeEditorOptions {
  editorMetadata?: CubeEditorMetadataHudActions;
  feedback?: { push?(severity: string, summary: string, detail?: string): unknown } | null;
}

type SurfaceEditorBindings = Pick<
  CubeSurfacePresenterOptions,
  'openEditor' | 'canEdit' | 'onEditDenied'
>;

/** Bind navigation, edit authorization, and denial feedback as one surface contract. */
export function buildCubeRuntimeEditorBindings(
  options: RuntimeEditorOptions,
  openEditor: CubeSurfacePresenterOptions['openEditor'],
): SurfaceEditorBindings {
  return {
    openEditor,
    ...(options.editorMetadata ? { canEdit: options.editorMetadata.canEdit } : {}),
    onEditDenied: () =>
      options.feedback?.push?.(
        'info',
        'Cube definition is read-only',
        'Fork this Cube before editing its definition.',
      ),
  };
}
