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
/** Prove modal semantics survive Comfy's property-based element helper. */
import { expect, jest, test } from '@jest/globals';
import { $el as createElement } from '../../frontend_core/support/mocks/ui.js';

/** Model hyphenated host properties that do not become DOM attributes. */
const hostElement: typeof createElement = (tag, props = {}, children = []) => {
  if (props && typeof props === 'object' && !Array.isArray(props) && !(props instanceof Node)) {
    const properties = Object.entries(props);
    const element = createElement(
      tag,
      Object.fromEntries(properties.filter(([key]) => !key.includes('-'))),
      children,
    );
    for (const [key, value] of properties.filter(([key]) => key.includes('-'))) {
      Reflect.set(element, key, value);
    }
    return element;
  }
  return createElement(tag, props, children);
};

jest.unstable_mockModule('/scripts/ui.js', () => ({ $el: hostElement }));
const { ModalShell } = await import('../../../frontend/comfyui/ui/dialogs/ModalShell.js');

test('explicit attributes identify the frontmost modal and its close action in the real host', async () => {
  const shell = new ModalShell({
    adapter: { getDocument: () => document, getWindow: () => window },
  });
  try {
    const result = shell.open({ title: 'Host dialog', cancelResult: false });
    expect(shell.elements.dialog?.getAttribute('aria-modal')).toBe('true');
    expect(shell.elements.dialog?.getAttribute('aria-label')).toBe('Host dialog');
    expect(shell.elements.closeButton?.getAttribute('aria-label')).toBe('Close dialog');
    expect(shell.elements.errorEl?.getAttribute('aria-live')).toBe('polite');
    shell.elements.closeButton?.click();
    await expect(result).resolves.toBe(false);
  } finally {
    shell.close(false);
    document.body.replaceChildren();
  }
});
