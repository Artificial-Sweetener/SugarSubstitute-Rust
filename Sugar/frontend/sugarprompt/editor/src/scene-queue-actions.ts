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
/** Accepted scene intents delegate to one finite service lane independently of native menu geometry. */
import type { EditorView, ViewUpdate } from '@codemirror/view';
import { SceneQueueSession } from '../../services/src/scenes/queue.js';
import { editorFreshness } from './freshness.js';
import {
  currentSceneQueueTarget,
  sceneQueuePublication,
  type PreparedSceneQueueTarget,
} from './scene-queue.js';

/** Native lifetime cancellation settles held host work and suppresses late results after teardown. */
export class SceneQueueActions {
  private readonly view: EditorView;
  private session: SceneQueueSession;
  private operation = 0;
  private disposed = false;
  /** Capture this mounted source-bound publication without retaining prompt bodies. */
  constructor(view: EditorView) {
    this.view = view;
    this.session = new SceneQueueSession(
      view.state.field(sceneQueuePublication)?.capability ?? null,
    );
  }
  /** Publication replacement replaces the complete lane; composition/permission changes cancel current work. */
  update(update: ViewUpdate): void {
    if (
      update.startState.field(sceneQueuePublication) !== update.state.field(sceneQueuePublication)
    ) {
      this.session.dispose();
      this.session = new SceneQueueSession(
        update.state.field(sceneQueuePublication)?.capability ?? null,
      );
    } else if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      this.view.composing ||
      this.view.compositionStarted
    )
      this.session.cancel();
  }
  /** A current accepted menu target invokes the captured host once; unexpected current failures remain observable. */
  async execute(target: PreparedSceneQueueTarget): Promise<void> {
    if (
      this.disposed ||
      this.view.composing ||
      this.view.compositionStarted ||
      !currentSceneQueueTarget(this.view.state, target)
    )
      return;
    const operation = ++this.operation;
    try {
      const outcome = await this.session.queue(target.request);
      if (!this.disposed && operation === this.operation)
        target.host.observe(
          currentSceneQueueTarget(this.view.state, target) ? outcome : { status: 'cancelled' },
        );
    } catch (cause) {
      if (
        !this.disposed &&
        operation === this.operation &&
        currentSceneQueueTarget(this.view.state, target)
      )
        target.host.report(cause);
    }
  }
  /** Retire all owned work and host callbacks before the view disappears. */
  dispose(): void {
    this.disposed = true;
    this.operation++;
    this.session.dispose();
  }
}
