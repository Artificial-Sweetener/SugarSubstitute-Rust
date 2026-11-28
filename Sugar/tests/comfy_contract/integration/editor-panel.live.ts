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
/** Exercise both external editor messaging boundaries against a real ComfyUI page. */
import { expect, expectSugarLoaded, test } from '../support/LiveComfy.js';

for (const transport of ['native', 'uxp'] as const) {
  test(`${transport} editor panel negotiates Plugin Mode and reserved geometry through real extension setup`, async ({
    page,
  }) => {
    const host = 'fixture.editor.panel';
    const session = 'compatibility-editor-session';
    await page.addInitScript(
      ({ host, session, transport }) => {
        let deliver: ((message: unknown) => void) | undefined;
        const send = (message: { type?: string }): void => {
          if (message.type === 'applied')
            document.documentElement.dataset.compatibilityReceipt = 'applied';
          if (message.type !== 'ready') return;
          queueMicrotask(() => {
            deliver?.({
              protocol: 'sugarcubes.host',
              version: 1,
              session,
              type: 'description',
              protocols: [{ name: 'sugarcubes.panel', version: 3 }],
              capabilities: [{ name: 'presentation.host-styles' }],
            });
            deliver?.({
              protocol: 'sugarcubes.panel',
              version: 3,
              session,
              type: 'configure',
              presentation: {
                pluginMode: true,
                visualStyle: 'photopro',
                photoProPalette: 'dark-gray',
                accentColor: '#d6d6d6',
                resizeCornerSize: 18,
              },
            });
          });
        };
        if (transport === 'native') {
          Reflect.set(window, 'sugarcubesPanelHost', {
            host,
            session,
            send,
            subscribe: (receive: (message: unknown) => void) => {
              deliver = receive;
              return () => {
                deliver = undefined;
              };
            },
          });
        } else {
          const bridge = { postMessage: send };
          Reflect.set(window, 'uxpHost', bridge);
          deliver = (data) => {
            const event = new MessageEvent('message', { origin: host, data });
            Object.defineProperty(event, 'source', { value: bridge });
            window.dispatchEvent(event);
          };
        }
      },
      { host, session, transport },
    );
    const query = new URLSearchParams({ 'sc-panel-host': host, 'sc-panel-session': session });
    if (transport === 'native') query.set('sc-panel-transport', 'native');
    await page.goto('/?' + query.toString(), { waitUntil: 'domcontentloaded' });
    const root = page.locator('html');
    await expectSugarLoaded(page);
    await expect(root).toHaveAttribute('data-compatibility-receipt', 'applied');
    await expect(root).toHaveAttribute('data-sc-plugin-mode', '');
    await expect(root).toHaveAttribute('data-sc-visual-style', 'photopro');
    await expect(root).toHaveCSS('--sc-panel-resize-corner', '18px');
  });
}
