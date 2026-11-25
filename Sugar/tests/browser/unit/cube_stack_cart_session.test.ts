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
/** Prove the Add cubes draft stays isolated and reaches the workflow once, through the Cube chain. */
import { expect, jest, test } from '@jest/globals';
import {
  CubeStackCartSession,
  type CubeStackRequest,
} from '../../../frontend/comfyui/ui/picker/application/CubeStackCartSession.js';

test('Reset restores the full opening stack and an unchanged Apply changes nothing', () => {
  const f = fixture();
  f.session.remove('existing:first');
  f.session.add('artifact');
  expect(f.session.entries).toHaveLength(2);
  f.session.reset();
  expect(f.session.entries.map((entry) => entry.draftId)).toEqual([
    'existing:first',
    'existing:second',
  ]);
  expect(f.session.apply()).toBe(false);
  expect(f.apply).not.toHaveBeenCalled();
});

test('repeated staged artifacts become independent new Cubes in the requested order', () => {
  const f = fixture();
  f.session.add('artifact');
  f.session.add('artifact');
  expect(f.session.apply()).toBe(true);
  const requested = f.apply.mock.calls[0]![0];
  expect(requested.map((entry) => entry.instanceId)).toEqual(['first', 'second', 'new-1', 'new-2']);
  expect(requested.slice(2)).toEqual(
    ['new-1', 'new-2'].map((instanceId) => ({
      source: 'new',
      instanceId,
      alias: 'Cube',
      payload: { prepared: 'artifact' },
      size: [920, 600],
    })),
  );
  expect(() => f.session.apply()).toThrow('closed');
});

test('a cart whose workflow gained or lost Cubes meanwhile changes nothing', () => {
  const f = fixture();
  f.session.add('artifact');
  f.workflow.push('pasted');
  expect(() => f.session.apply()).toThrow('Workflow changed');
  expect(f.apply).not.toHaveBeenCalled();
  expect(f.session.changed).toBe(true);
});

test('a refused stack keeps the edited cart for another try', () => {
  const f = fixture();
  f.session.add('artifact');
  f.apply.mockImplementationOnce(() => {
    throw new Error('Upscale is wired to the Cube before it, so it cannot be first.');
  });
  expect(() => f.session.apply()).toThrow('cannot be first');
  expect(f.session.changed).toBe(true);
  expect(f.session.apply()).toBe(true);
});

/** Provide captured artifacts and a chain that records what the cart asks of it. */
function fixture() {
  let id = 0;
  const workflow = ['first', 'second'];
  const apply = jest.fn((_requested: readonly CubeStackRequest<{ prepared: string }>[]) => []);
  const session = new CubeStackCartSession<{ prepared: string }>({
    order: [...workflow],
    artifacts: [
      {
        catalogKey: 'artifact',
        cubeId: 'cube',
        version: '1',
        alias: 'Cube',
        document: { cube_id: 'cube', version: '1' },
        size: [920, 600],
      },
    ],
    chain: { order: () => workflow, apply },
    prepare: (catalogKey) => ({ prepared: catalogKey }),
    createId: () => `new-${++id}`,
  });
  return { session, apply, workflow };
}
