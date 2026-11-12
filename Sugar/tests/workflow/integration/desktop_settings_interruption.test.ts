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
/** Verify settings route interruption through the real settings lifecycle owner and production desktop runtime. */
import { SettingsWorkspaceHost } from '../../../frontend/comfyui/ui/settings/SettingsWorkspaceHost.js';
import type { SettingsWorkspaceRoute } from '../../../frontend/comfyui/ui/settings/SettingsWorkspaceRoute.js';
import { desktopRuntimeFixture } from '../support/desktop_runtime.js';
import { DesktopSessionCancellation } from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';

/** Wait on the actual owner's event, never on DOM polling, arbitrary sleeps or private dialog stores. */
function routeChanged(
  route: SettingsWorkspaceRoute,
  expected: 'workflow' | 'settings',
): Promise<void> {
  if (route.read() === expected) return Promise.resolve();
  return new Promise<void>((resolve) => {
    const stop = route.subscribe(() => {
      if (route.read() === expected) {
        stop();
        resolve();
      }
    });
  });
}

/** Model only Comfy's existing command and settings registration at the browser boundary. */
function settingsFixture() {
  const errors: unknown[] = [];
  const commands: string[] = [];
  const dialog = document.createElement('div');
  dialog.dataset.testid = 'settings-dialog';
  const settings = new SettingsWorkspaceHost({
    document,
    app: {
      ui: { settings: { addSetting: () => {}, settingsParamLookup: {} } },
      extensionManager: {
        command: {
          execute: async (id: string) => {
            commands.push(id);
            document.body.append(dialog);
          },
        },
      },
    },
    mode: { enabled: false, subscribe: () => () => {} },
    palette: { theme: { dark: true, color: '#479ef5' }, subscribe: () => () => {} },
    report: (error) => errors.push(error),
  });
  settings.setup();
  return { settings, dialog, commands, errors };
}

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

test('settings-only startup interruption preserves protected restore and refuses saving the default workspace after close', async () => {
  const owner = settingsFixture();
  const f = await desktopRuntimeFixture(owner.settings.sessionRoute);
  const opened = routeChanged(owner.settings.sessionRoute, 'settings');
  document.body.append(owner.dialog);
  await opened;
  f.host.committed();
  expect(f.runtime.readiness().committed).toBe(false);
  const closed = routeChanged(owner.settings.sessionRoute, 'workflow');
  owner.dialog.remove();
  await closed;
  expect(f.runtime.readiness().committed).toBe(false);
  await expect(f.runtime.capture(f.nextId(), 20, new DesktopSessionCancellation())).rejects.toThrow(
    'committed',
  );
  await expect(
    f.runtime.materialize(f.checkpoint, new DesktopSessionCancellation()),
  ).rejects.toThrow('committed');
  const original = f.host.workflow.activeWorkflow;
  expect(original?.filename).toBe('Initial');
  // A later actual authored graph transaction establishes a fresh user-owned frontier.
  f.host.setGraph({ nodes: [{ id: 7, type: 'SaveImage', title: 'User authored' }] });
  f.host.app.rootGraph.onAfterChange(f.host.app.rootGraph);
  await Promise.resolve();
  expect(f.runtime.readiness().committed).toBe(true);
  f.dispose();
  owner.settings.dispose();
  expect(f.errors).toEqual([]);
  expect(owner.errors).toEqual([]);
});

test('restored settings route uses the public command and acknowledges only after the actual settings owner mounts', async () => {
  const owner = settingsFixture();
  const f = await desktopRuntimeFixture(owner.settings.sessionRoute);
  f.host.committed();
  await f.runtime.materialize(
    { ...f.checkpoint, route: 'settings' },
    new DesktopSessionCancellation(),
  );
  expect(owner.commands).toEqual(['Comfy.ShowSettingsDialog']);
  expect(owner.settings.sessionRoute.read()).toBe('settings');
  expect(f.runtime.readiness().committed).toBe(true);
  const captured = await f.runtime.capture(f.nextId(), 30, new DesktopSessionCancellation());
  expect(captured.route).toBe('settings');
  f.dispose();
  owner.settings.dispose();
  expect(f.errors).toEqual([]);
  expect(owner.errors).toEqual([]);
});

test('closing the page cancels a pending actual settings mount without a late route acknowledgement', async () => {
  const owner = settingsFixture();
  const controller = new AbortController();
  controller.abort();
  await expect(owner.settings.sessionRoute.show(controller.signal)).rejects.toThrow('cancelled');
  expect(owner.commands).toEqual([]);
  owner.settings.dispose();
});
