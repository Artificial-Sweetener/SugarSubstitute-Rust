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
/** Retain accepted prompt ownership even before a submission produces final artifacts. */
import type { SessionEntry, SessionScope } from '../contracts/SessionProtocol.generated.js';
import { sessionUuid } from './SessionPrimitives.js';

/** Match receipts to frozen submission sources rather than a reused live node number. */
export function sessionSubmissions(entry: SessionEntry, scope: SessionScope): boolean {
  const prompts = new Set<string>();
  const runs = new Set<string>();
  for (const submission of entry.submissions) {
    const key = `${submission.runId}/${submission.scene?.sceneId ?? ''}`;
    if (
      submission.entryId !== entry.id ||
      submission.backend !== scope.backend ||
      !sessionUuid(submission.runId) ||
      !submission.promptId ||
      prompts.has(submission.promptId) ||
      runs.has(key) ||
      submission.graphRevision > entry.document.graphRevision ||
      (submission.scene !== null &&
        (!sessionUuid(submission.scene.groupId) ||
          !sessionUuid(submission.scene.sceneId) ||
          submission.scene.order >= submission.scene.count))
    )
      return false;
    prompts.add(submission.promptId);
    runs.add(key);
    const sources = new Set<string>();
    for (const source of submission.sources) {
      if (
        !sessionUuid(source.id) ||
        sources.has(source.id) ||
        !source.nodeId ||
        !source.executionPath ||
        source.graphRevision !== submission.graphRevision
      )
        return false;
      sources.add(source.id);
    }
  }
  return entry.outputs.every(
    (output) =>
      output.promptId === null ||
      entry.submissions.some(
        (submission) =>
          submission.promptId === output.promptId &&
          submission.runId === output.runId &&
          submission.runOrder === output.runOrder &&
          (submission.scene === null
            ? output.scene === null
            : output.scene !== null &&
              submission.scene.groupId === output.scene.groupId &&
              submission.scene.sceneId === output.scene.sceneId &&
              submission.scene.order === output.scene.order &&
              submission.scene.count === output.scene.count &&
              submission.scene.title === output.scene.title) &&
          submission.sources.some((source) => source.id === output.sourceId),
      ),
  );
}
