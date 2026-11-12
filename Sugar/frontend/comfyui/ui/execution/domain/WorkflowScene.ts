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
/** Describe the prompt scenes one generation runs, independently of prompt text and host graphs. */

/** One authored prompt scene: normalized identity, literal title and authored order. */
export interface WorkflowScene {
  readonly key: string;
  readonly title: string;
  readonly order: number;
}

/** Every scene one generation request queued, as separate prompts that share this identity. */
export interface WorkflowSceneRun {
  readonly id: string;
  readonly scenes: readonly WorkflowScene[];
}

/** The scene one queued prompt renders within its scene run. */
export interface WorkflowSceneMember {
  readonly run: WorkflowSceneRun;
  readonly key: string;
}

/** Copy a scene membership so neither callers nor later runs can change accepted routing. */
export function freezeWorkflowSceneMember(member: WorkflowSceneMember): WorkflowSceneMember {
  return Object.freeze({
    key: member.key,
    run: Object.freeze({
      id: member.run.id,
      scenes: Object.freeze(member.run.scenes.map((scene) => Object.freeze({ ...scene }))),
    }),
  });
}
