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
/** Prove submitted seeds stay isolated from armed widgets, history and other workflows. */
import { WorkflowSeedHistory } from '../../../frontend/comfyui/ui/execution/application/WorkflowSeedHistory.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { decodeComfySubmittedSeeds } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfySubmittedSeeds.js';

/** Open two independent workflow bindings with observable generation changes. */
function fixture() {
  const sessions = new WorkflowSessionStore();
  const a = { sessionId: workflowSessionId('a'), path: 'a.json', title: 'A' };
  const b = { sessionId: workflowSessionId('b'), path: 'b.json', title: 'B' };
  sessions.synchronize([a, b], a.sessionId);
  const history = new WorkflowSeedHistory();
  sessions.subscribe((snapshot) => history.synchronize(snapshot));
  history.synchronize(sessions.read());
  const context = sessions.read().active;
  if (!context) throw new Error('Workflow missing');
  return { sessions, history, context, a, b };
}

test('records submitted values including zero and renamed seeds, rejecting connections and unsafe values', () => {
  const f = fixture();
  const prompt = {
    'cube:1': { inputs: { seed: 0, custom_seed: 123, linked: ['4', 0], unsafe: 1e20, text: '42' } },
  };
  const values = decodeComfySubmittedSeeds(prompt);
  f.history.record(f.context, 1, values);
  prompt['cube:1'].inputs.custom_seed = 456;
  expect(f.history.read(f.context, [{ path: 'cube:1', name: 'seed' }])).toBe(0);
  expect(f.history.read(f.context, [{ path: 'cube:1', name: 'custom_seed' }])).toBe(123);
  expect(values).toHaveLength(2);
  expect(decodeComfySubmittedSeeds(null)).toEqual([]);
});

test('preserves the newest accepted submission and refuses conflicting shared seeds', () => {
  const f = fixture();
  const inputs = [
    { path: '1', name: 'seed' },
    { path: '2', name: 'seed' },
  ];
  expect(f.history.read(f.context, inputs)).toBeNull();
  f.history.record(
    f.context,
    2,
    inputs.map((input) => ({ ...input, value: 222 })),
  );
  f.history.record(
    f.context,
    1,
    inputs.map((input) => ({ ...input, value: 111 })),
  );
  expect(f.history.read(f.context, inputs)).toBe(222);
  f.history.record(f.context, 3, [{ ...inputs[0]!, value: 333 }]);
  expect(f.history.read(f.context, inputs)).toBe(333);
  f.history.record(
    f.context,
    4,
    inputs.map((input, index) => ({ ...input, value: index })),
  );
  expect(f.history.read(f.context, inputs)).toBeNull();
});

test('retains values through undo and tab switching and rejects replaced or closed generations', () => {
  const f = fixture();
  const input = { path: '1', name: 'seed' };
  f.history.record(f.context, 1, [{ ...input, value: 42 }]);
  f.sessions.synchronize([f.a, f.b], f.b.sessionId);
  const b = f.sessions.read().active;
  if (!b) throw new Error('Workflow B missing');
  expect(f.history.read(b, [input])).toBeNull();
  f.sessions.synchronize([f.a, f.b], f.a.sessionId);
  const undo = f.sessions.beginLoad('history');
  f.sessions.completeLoad(undo, true);
  expect(f.history.read(f.context, [input])).toBe(42);
  const replace = f.sessions.beginLoad('replace');
  f.sessions.completeLoad(replace, true);
  f.history.record(f.context, 2, [{ ...input, value: 99 }]);
  expect(f.history.read(f.context, [input])).toBeNull();
  f.sessions.synchronize([f.b], f.b.sessionId);
  f.history.dispose();
});
