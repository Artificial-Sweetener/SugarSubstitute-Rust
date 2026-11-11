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
/** One finite native lifetime prepares queue hit testing after source semantics settle outside browser input. */
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import {
  sceneQueuePublication,
  sceneQueueTargets,
  prepareSceneQueueTargets,
  installSceneQueueTargets,
} from './scene-queue.js';
import { semanticStructures } from './structure.js';

/** Coalesced publication has no parser, editable source, timer or independent request authority. */
class SceneQueuePreparation {
  private queued = false;
  private disposed = false;
  private readonly view: EditorView;
  /** Prepare initial authority only after the native semantic owner publishes its source. */
  constructor(view: EditorView) {
    this.view = view;
    this.schedule();
  }
  /** Source/authority/semantic changes invalidate derived lookup and schedule one current preparation. */
  update(update: ViewUpdate): void {
    if (
      update.startState.field(sceneQueuePublication) !==
        update.state.field(sceneQueuePublication) ||
      update.startState.field(semanticStructures).native !==
        update.state.field(semanticStructures).native
    )
      this.schedule();
  }
  /** Preparation never runs inside native transaction update, input handling or measurement. */
  private schedule(): void {
    if (this.queued || this.disposed) return;
    this.queued = true;
    queueMicrotask(() => {
      this.queued = false;
      if (this.disposed || this.view.state.field(sceneQueueTargets)) return;
      const prepared = prepareSceneQueueTargets(this.view.state);
      if (prepared) this.view.dispatch({ effects: installSceneQueueTargets.of(prepared) });
    });
  }
  /** Pending publication cannot reach a destroyed editor. */
  destroy(): void {
    this.disposed = true;
  }
}
/** Mounted queue lookup is assembled by the native editor, not consumers. */
export const sceneQueuePreparation = ViewPlugin.fromClass(SceneQueuePreparation);
