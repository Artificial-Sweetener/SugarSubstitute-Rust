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
import { jest } from '@jest/globals';
import { SugarPromptMenu } from '../../../frontend/comfyui/ui/prompt_editor/runtime/SugarPromptMenu.js';
import type { ContextMenu as PromptMenu } from '../../../frontend/sugarprompt/editor/src/context-menu-state.js';

afterEach(() => document.body.replaceChildren());

/** A prepared prompt menu with one row of its own, opened on a LoRA banner or plain text. */
function promptMenu(inline: boolean): PromptMenu {
  return {
    items: [{ kind: 'text', label: 'Copy', action: 'copy', shortcut: 'Control+C' }],
    actions: inline
      ? {
          kind: 'inline',
          resource: { resolution: 'found', model: { backendValue: 'styles/ink.safetensors' } },
        }
      : null,
    anchor: { left: 10, top: 10, right: 20, bottom: 20, width: 10, height: 10 },
  } as unknown as PromptMenu;
}

test('a LoRA banner menu adds the model rows every model surface shows, after its own rows', () => {
  const root = document.createElement('div');
  document.body.append(root);
  const loraMenu = jest.fn((_value: string) => [
    { label: 'Go to CivitAI page', select: jest.fn() },
  ]);
  const menu = new SugarPromptMenu(root, jest.fn(), loraMenu);
  menu.show(
    promptMenu(true),
    async () => true,
    () => undefined,
  );
  expect(loraMenu).toHaveBeenCalledWith('styles/ink.safetensors');
  const text = document.querySelector('[data-sc-prompt-menu]')?.textContent ?? '';
  expect(text.indexOf('Copy')).toBeGreaterThanOrEqual(0);
  expect(text.indexOf('Go to CivitAI page')).toBeGreaterThan(text.indexOf('Copy'));
  menu.dispose();
});

test('a menu opened away from a LoRA asks for no model rows', () => {
  const root = document.createElement('div');
  document.body.append(root);
  const loraMenu = jest.fn((_value: string) => []);
  const menu = new SugarPromptMenu(root, jest.fn(), loraMenu);
  menu.show(
    promptMenu(false),
    async () => true,
    () => undefined,
  );
  expect(loraMenu).not.toHaveBeenCalled();
  menu.dispose();
});
