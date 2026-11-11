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
/** Own the current scene run and its latest output per source independently of live job routing. */
import type { OutputSourceId } from '../domain/ExecutionIdentity.js';
import type { WorkflowRun } from '../domain/WorkflowRun.js';
import type { WorkflowOutputSet } from '../domain/WorkflowOutput.js';
import type { WorkflowSceneRun } from '../domain/WorkflowScene.js';
import type { WorkflowSceneResults } from '../domain/WorkflowResultSession.js';

interface SceneOutputs {
  readonly run: WorkflowSceneRun;
  readonly outputs: Map<string, Map<OutputSourceId, WorkflowOutputSet>>;
  running: string | null;
  order: number;
}

/** Preserve accepted-scene handoff while a newer ordinary run still awaits its first final. */
export class WorkflowSceneOutputs {
  #scenes: SceneOutputs | null = null;

  /** Follow only a current accepted scene run; a pending ordinary run retains existing scene pixels. */
  follow(run: WorkflowRun, frontier: number): void {
    const member = run.scene;
    if (!member || run.order < frontier) return;
    if (this.#scenes?.run.id !== member.run.id)
      this.#scenes = { run: member.run, outputs: new Map(), running: member.key, order: run.order };
    else {
      this.#scenes.running = member.key;
      this.#scenes.order = Math.max(this.#scenes.order, run.order);
    }
  }

  /** Retain every scene's latest source output, retiring its group only on an ordinary final handoff. */
  retain(run: WorkflowRun | null, output: WorkflowOutputSet, frontier: number): void {
    if (run?.scene) {
      this.follow(run, frontier);
      if (this.#scenes?.run.id === run.scene.run.id) {
        const outputs = this.#scenes.outputs.get(run.scene.key) ?? new Map();
        outputs.set(output.source.id, output);
        this.#scenes.outputs.set(run.scene.key, outputs);
      }
    } else if (run && this.#scenes && run.order > this.#scenes.order) this.#scenes = null;
  }

  /** Import already validated scene membership without marking any scene as executing or replaying a job. */
  restore(snapshot: WorkflowSceneResults | null, frontier: number): void {
    this.#scenes = snapshot
      ? {
          run: Object.freeze({
            id: snapshot.id,
            scenes: Object.freeze(
              snapshot.scenes.map(({ key, title, order }) => Object.freeze({ key, title, order })),
            ),
          }),
          outputs: new Map(
            snapshot.scenes.map((scene) => [
              scene.key,
              new Map(scene.outputs.map((output) => [output.source.id, output])),
            ]),
          ),
          running: null,
          order: frontier,
        }
      : null;
  }

  /** Publish authored scene order while preserving reference identity for unchanged subscriptions. */
  snapshot(previous: WorkflowSceneResults | null): WorkflowSceneResults | null {
    const scenes = this.#scenes;
    if (!scenes) return null;
    const next = scenes.run.scenes.map((scene) =>
      Object.freeze({
        ...scene,
        outputs: Object.freeze(
          [...(scenes.outputs.get(scene.key)?.values() ?? [])].sort(
            (a, b) => a.source.order - b.source.order,
          ),
        ),
      }),
    );
    if (
      previous?.id === scenes.run.id &&
      previous.running === scenes.running &&
      previous.scenes.length === next.length &&
      previous.scenes.every((scene, index) => {
        const current = next[index];
        return (
          !!current &&
          scene.key === current.key &&
          scene.outputs.length === current.outputs.length &&
          scene.outputs.every((output, at) => output === current.outputs[at])
        );
      })
    )
      return previous;
    return Object.freeze({
      id: scenes.run.id,
      running: scenes.running,
      scenes: Object.freeze(next),
    });
  }
}
