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
/** Freeze the document revision at prompt-construction entry, before awaited host widgets and later edits. */
import { createComfyPromptConstructionObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptConstructionObserver.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { WorkflowSubmissionContexts } from '../../../frontend/comfyui/ui/execution/application/WorkflowSubmissionContexts.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import { ComfyPromptSubmissionObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptSubmissionObserver.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowOrderedSubmission } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSubmission.js';

test('accepted pending submission keeps the construction graph revision after graph edits while awaiting widgets', async () => {
  const sessions = new WorkflowSessionStore();
  const context = { sessionId: workflowSessionId('A'), title: 'A', path: '' };
  sessions.synchronize([context], context.sessionId);
  const contexts = new WorkflowSubmissionContexts();
  let finish: (value: object) => void = () => {};
  const promise = new Promise<object>((resolve) => {
    finish = resolve;
  });
  const app = { graphToPrompt: async () => await promise };
  let revision = 3;
  const observer = createComfyPromptConstructionObserver(
    app,
    sessions,
    contexts,
    { error: () => {} },
    () => revision,
  );
  observer.install();
  const construction = app.graphToPrompt();
  revision = 8;
  const payload = { output: { '1': { class_type: 'PreviewImage' } }, workflow: { nodes: [] } };
  finish(payload);
  await construction;
  expect(contexts.read(payload)?.graphRevision).toBe(3);
  const runs = new WorkflowRunRegistry();
  let accepted: WorkflowOrderedSubmission | null = null;
  const submissions = new ComfyPromptSubmissionObserver({
    contexts,
    logger: { error: () => {}, warn: () => {} },
    submitting: (context) => runs.reserve(context),
    accepted: (context) => {
      accepted = context;
    },
  });
  submissions.capture(payload)?.accepted({ prompt_id: 'pending' });
  expect(accepted).toMatchObject({ graphRevision: 3, order: 1 });
  observer.dispose();
  contexts.dispose();
  sessions.dispose();
});
