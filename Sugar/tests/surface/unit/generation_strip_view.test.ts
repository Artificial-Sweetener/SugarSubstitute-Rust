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
/** Prove stable generation-strip controls, batch edits and action feedback. */
import { jest } from '@jest/globals';
import { GenerationStripView } from '../../../frontend/comfyui/ui/surface/frame/GenerationStripView.js';

afterEach(() => document.body.replaceChildren());

/** Build real strip DOM around a deterministic external generation boundary. */
function fixture() {
  const actions = {
    setBatch: jest.fn<(value: number) => void>(),
    generate: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    cancel: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    skip: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    report: jest.fn<(error: unknown) => void>(),
  };
  const view = new GenerationStripView(document, actions);
  document.body.append(view.element);
  const state = { batch: 3, generateEnabled: true, skipEnabled: true, cancelEnabled: true };
  view.update(state);
  const input = view.element.querySelector('input')!;
  return { actions, view, state, input };
}

test('source order and control identity survive native execution state changes', () => {
  const { view, state } = fixture();
  const controls = [...view.element.querySelectorAll('.sc-generation-action')];
  expect(controls.map((control) => control.getAttribute('data-action'))).toEqual([
    'cancel',
    'generate',
    'skip',
  ]);
  view.update({ ...state, skipEnabled: false, cancelEnabled: false });
  expect([...view.element.querySelectorAll('.sc-generation-action')]).toEqual(controls);
  expect(view.element.querySelector<HTMLButtonElement>('[data-action="skip"]')!.disabled).toBe(
    true,
  );
  view.dispose();
  expect(document.querySelector('.sc-generation-strip')).toBeNull();
});

test('direct entry, steps and Escape edit cancellation use the host batch setting', () => {
  const { actions, view, input, state } = fixture();
  input.value = '5';
  input.dispatchEvent(new Event('change'));
  expect(actions.setBatch).toHaveBeenLastCalledWith(5);
  view.update({ ...state, batch: 5 });
  view.element.querySelector<HTMLButtonElement>('[data-direction="up"]')!.click();
  expect(actions.setBatch).toHaveBeenLastCalledWith(6);
  input.focus();
  input.value = '77';
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
  expect(actions.setBatch).not.toHaveBeenCalledWith(77);
  input.value = '<invalid>';
  input.dispatchEvent(new Event('change'));
  expect(input.value).toBe('5');
});

test('preview updates preserve an in-progress batch edit and unrelated focus', () => {
  const { view, input, state } = fixture();
  input.focus();
  input.value = '12';
  view.update({ ...state, skipEnabled: false });
  expect(input.value).toBe('12');
  expect(document.activeElement).toBe(input);
  view.update({ ...state, batch: 7 });
  expect(input.value).toBe('7');
});

test('each action delegates once and failure reaches the existing feedback boundary', async () => {
  const { actions, view } = fixture();
  for (const id of ['generate', 'cancel', 'skip'])
    view.element.querySelector<HTMLButtonElement>(`[data-action="${id}"]`)!.click();
  expect(actions.generate).toHaveBeenCalledTimes(1);
  expect(actions.cancel).toHaveBeenCalledTimes(1);
  expect(actions.skip).toHaveBeenCalledTimes(1);
  const error = new Error('host rejected generation');
  actions.generate.mockRejectedValue(error);
  view.element.querySelector<HTMLButtonElement>('[data-action="generate"]')!.click();
  await Promise.resolve();
  expect(actions.report).toHaveBeenCalledWith(error);
});
