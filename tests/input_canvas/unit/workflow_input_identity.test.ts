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
/** Prove stable per-workflow input identities and fresh identities for duplicated workflows. */
import { jest } from '@jest/globals';
import {
  ComfyWorkflowInputState,
  WORKFLOW_INPUT_STATE_KEY,
} from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyWorkflowInputState.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

/** Compose the store over one root graph and a list of other open workflows' states. */
function fixture(peers: unknown[] = []) {
  const root: ComfyGraph = { extra: { other: true } };
  const commit = jest.fn();
  let ids = 0;
  const state = new ComfyWorkflowInputState(
    () => root,
    () => 'id' + String(++ids),
    () => peers,
    commit,
  );
  return { root, state, commit, peers };
}

test('workflows without surfaces stay unmodified and keep one identity once created', () => {
  const f = fixture();
  expect(f.state.workflow(false)).toBe('workflow-none');
  expect(f.root.extra).toEqual({ other: true });
  expect(f.state.workflow(true)).toBe('workflow-id1');
  expect(f.state.workflow(true)).toBe('workflow-id1');
  expect(f.state.workflow(false)).toBe('workflow-id1');
  f.state.write('workflow-id1', { route: 'input', surface: 's', mask: 'm' });
  expect(f.state.read('workflow-id1')).toEqual({ route: 'input', surface: 's', mask: 'm' });
  expect(f.state.read('workflow-other')).toBeNull();
  expect(f.root.extra).toMatchObject({ other: true });
  expect(f.commit).not.toHaveBeenCalled();
});

test('a workflow sharing its identity with another open workflow is a copy and takes a fresh one', () => {
  const f = fixture();
  f.state.workflow(true);
  f.state.write('workflow-id1', { route: 'input', surface: 's', mask: null });
  f.peers.push({ extra: { [WORKFLOW_INPUT_STATE_KEY]: { id: 'workflow-id1' } } }, null);
  expect(f.state.workflow(false)).toBe('workflow-id2');
  expect(f.commit).toHaveBeenCalledTimes(1);
  expect(f.state.read('workflow-id2')).toEqual({ route: 'input', surface: 's', mask: null });
  expect(f.state.workflow(false)).toBe('workflow-id2');
  expect(f.commit).toHaveBeenCalledTimes(1);
});

test('the loaded identity is read without assigning, replacing or committing one', () => {
  const f = fixture();
  expect(f.state.loaded()).toBe('workflow-none');
  expect(f.root.extra).toEqual({ other: true });
  f.state.workflow(true);
  f.peers.push({ extra: { [WORKFLOW_INPUT_STATE_KEY]: { id: 'workflow-id1' } } });
  expect(f.state.loaded()).toBe('workflow-id1');
  expect(f.commit).not.toHaveBeenCalled();
});
