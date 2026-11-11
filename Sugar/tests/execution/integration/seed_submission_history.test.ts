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
/** Prove seed history observes effective accepted prompts rather than next armed widget values. */
import { jest } from '@jest/globals';
import { WorkflowSeedHistory } from '../../../frontend/comfyui/ui/execution/application/WorkflowSeedHistory.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { WorkflowSubmissionContexts } from '../../../frontend/comfyui/ui/execution/application/WorkflowSubmissionContexts.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { ComfyPromptSubmissionObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptSubmissionObserver.js';
import {
  ComfyPromptQueueBridge,
  type ComfyPromptQueueApi,
} from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';

/** Bind a real queue observer and keep the seed history inspectable after each queue outcome. */
function fixture() {
  const sessions = new WorkflowSessionStore();
  const binding = {
    sessionId: workflowSessionId('seed-workflow'),
    title: 'Seed workflow',
    path: 'seed.json',
  };
  sessions.synchronize([binding], binding.sessionId);
  const context = sessions.read().active;
  if (!context) throw new Error('Missing workflow');
  const history = new WorkflowSeedHistory();
  history.synchronize(sessions.read());
  sessions.subscribe((snapshot) => history.synchronize(snapshot));
  const contexts = new WorkflowSubmissionContexts();
  let sequence = 0;
  const observer = new ComfyPromptSubmissionObserver({
    contexts,
    seeds: history,
    logger: { error: jest.fn(), warn: jest.fn() },
    submitting: (captured) => ({ ...captured, order: ++sequence }),
    accepted: jest.fn(),
  });
  const payload = { output: { 'cube:node': { inputs: { seed: 111 } } } };
  contexts.associate(payload, context, []);
  return { history, context, observer, payload };
}

test.each([false, true])(
  'captures effective host or compiled cube seed (direct=%s)',
  async (direct) => {
    const f = fixture();
    let armed = 111;
    const api: ComfyPromptQueueApi = {
      queuePrompt: async (position, payload) => {
        if (payload !== f.payload) throw new Error('Unexpected prompt');
        f.payload.output['cube:node'].inputs.seed = 222;
        armed = 333;
        return { prompt_id: 'accepted' };
      },
    };
    const bridge = new ComfyPromptQueueBridge({
      api,
      observer: f.observer,
      transform: async (payload) => ({ payload }),
      ...(direct
        ? {
            executeDirect: async () => ({
              prompt_id: 'cube',
              execution_prompt: { 'cube:node': { inputs: { seed: 444 } } },
            }),
          }
        : {}),
    });
    bridge.install();
    await api.queuePrompt?.(0, f.payload);
    expect(f.history.read(f.context, [{ path: 'cube:node', name: 'seed' }])).toBe(
      direct ? 444 : 222,
    );
    if (!direct) expect(armed).toBe(333);
    bridge.dispose();
  },
);

test.each(['rejected', 'invalid', 'cancelled'] as const)(
  'does not replace last seed on %s submission',
  async (outcome) => {
    const f = fixture();
    const observation = f.observer.capture(f.payload);
    observation?.prepared(f.payload);
    observation?.accepted({ prompt_id: 'first' });
    f.payload.output['cube:node'].inputs.seed = 999;
    const next = f.observer.capture(f.payload);
    next?.prepared(f.payload);
    if (outcome === 'rejected') next?.rejected(new Error('Validation failure'));
    else if (outcome === 'cancelled') next?.cancelled();
    else next?.accepted({ node_errors: ['invalid'] });
    next?.accepted({ prompt_id: 'late' });
    expect(f.history.read(f.context, [{ path: 'cube:node', name: 'seed' }])).toBe(111);
  },
);
