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
/** Prove editing use cases issue the contract commands and render only from host state. */
import { describe, expect, test } from '@jest/globals';
import { CanvasEditController } from '../../../frontend/comfyui/ui/canvas_editing/application/CanvasEditController.js';
import { CanvasEditHostFixture, editState, settle } from '../support/CanvasEditHostFixture.js';

const selection = { bounds: { left: 10, top: 20, width: 100, height: 80 } };

async function started(host = new CanvasEditHostFixture()) {
  const errors: unknown[] = [];
  const controller = new CanvasEditController({ host, report: (error) => errors.push(error) });
  controller.start();
  await settle();
  return { host, controller, errors };
}

describe('canvas edit controller', () => {
  test('reads the current state on start and adopts only newer revisions', async () => {
    const { host, controller } = await started();
    expect(host.commands).toEqual([{ type: 'state' }]);
    expect(host.observers).toBe(1);
    expect(controller.read()?.state.revision).toBe(1);
    host.push({ tool: 'brush' });
    expect(controller.read()?.state.tool).toBe('brush');
    host.state = { ...host.state, revision: 1, tool: 'move' };
    controller.execute({ type: 'commit' });
    await settle();
    expect(controller.read()?.state.tool).toBe('brush');
    controller.dispose();
    expect(host.observers).toBe(0);
  });

  test('shows nothing until the host presents an editable document', async () => {
    const { host, controller } = await started(
      new CanvasEditHostFixture(editState({ document: null })),
    );
    expect(controller.read()).toBeNull();
    host.push({ document: 'document-2' });
    expect(controller.read()?.state.document).toBe('document-2');
  });

  test('activates tools, remembers flyout members and refuses unavailable tools', async () => {
    const { host, controller } = await started(
      new CanvasEditHostFixture(
        editState({
          tools: [
            ...editState().tools.filter((t) => t.id !== 'move'),
            { id: 'move', available: false },
          ],
        }),
      ),
    );
    controller.selectTool('select-lasso');
    controller.selectSlot('move');
    controller.selectSlot('brush');
    await settle();
    expect(host.sent()).toEqual([
      { type: 'tool', tool: 'select-lasso' },
      { type: 'tool', tool: 'brush' },
    ]);
    expect(controller.read()?.slots.find((slot) => slot.id === 'selection')?.current.id).toBe(
      'select-lasso',
    );
  });

  test('previews a selection modification live and settles it once', async () => {
    const { host, controller } = await started(new CanvasEditHostFixture(editState({ selection })));
    controller.beginModify();
    expect(controller.read()?.surface).toEqual({ kind: 'modify' });
    controller.updateModify({ operation: 'feather', pixels: 12 });
    await settle();
    host.push({ session: { kind: 'modify', undo: false, redo: false }, tool: 'pan-zoom' });
    await controller.applyModify();
    expect(host.sent()).toEqual([
      { type: 'modify', target: 'selection', operation: 'expand', pixels: 4, phase: 'preview' },
      { type: 'modify', target: 'selection', operation: 'feather', pixels: 12, phase: 'preview' },
      { type: 'modify', target: 'selection', operation: 'feather', pixels: 12, phase: 'apply' },
    ]);
    host.push({ session: null });
    expect(controller.read()?.surface).toEqual({ kind: 'selection' });
  });

  test('cancels a selection modification and closes its page at once', async () => {
    const { host, controller } = await started(new CanvasEditHostFixture(editState({ selection })));
    controller.beginModify();
    controller.cancelModify();
    await settle();
    expect(host.sent().at(-1)).toEqual({
      type: 'modify',
      target: 'selection',
      operation: 'expand',
      pixels: 4,
      phase: 'cancel',
    });
    expect(controller.read()?.surface).toEqual({ kind: 'selection' });
  });

  test('opens the coverage editor, previews the whole mask and leaves it after apply', async () => {
    const host = new CanvasEditHostFixture();
    host.reply = (command) => {
      if (command.type === 'mode')
        host.state = { ...host.state, mode: command.mode, revision: host.state.revision + 1 };
      return { kind: 'state', state: host.state };
    };
    const { controller } = await started(host);
    await controller.openCoverage();
    await settle();
    expect(controller.read()?.surface).toEqual({ kind: 'coverage' });
    controller.updateCoverage({ operation: 'contract', pixels: 9 });
    await controller.applyCoverage();
    await settle();
    expect(host.sent()).toEqual([
      { type: 'mode', mode: 'coverage' },
      { type: 'modify', target: 'mask', operation: 'expand', pixels: 4, phase: 'preview' },
      { type: 'modify', target: 'mask', operation: 'contract', pixels: 9, phase: 'preview' },
      { type: 'modify', target: 'mask', operation: 'contract', pixels: 9, phase: 'apply' },
      { type: 'mode', mode: 'none' },
    ]);
    expect(controller.read()?.surface).toEqual({ kind: 'none' });
  });

  test('offers coverage editing only with an active mask and closes it with Esc', async () => {
    const empty = await started(new CanvasEditHostFixture(editState({ activeMask: null })));
    await empty.controller.openCoverage();
    expect(empty.host.sent()).toEqual([]);
    const { host, controller } = await started(
      new CanvasEditHostFixture(editState({ mode: 'coverage' })),
    );
    const escape = {
      phase: 'down' as const,
      key: 'Escape',
      ctrl: false,
      meta: false,
      shift: false,
      alt: false,
    };
    expect(controller.key(escape)).toBe(true);
    await settle();
    expect(host.sent()).toEqual([
      { type: 'modify', target: 'mask', operation: 'expand', pixels: 4, phase: 'cancel' },
      { type: 'mode', mode: 'none' },
    ]);
  });

  test('sends each navigation hold change once and releases it on blur', async () => {
    const { host, controller } = await started();
    const space = {
      phase: 'down' as const,
      key: ' ',
      ctrl: false,
      meta: false,
      shift: false,
      alt: false,
    };
    controller.key(space);
    controller.key(space);
    controller.releaseNavigation();
    controller.key({ ...space, phase: 'up' });
    await settle();
    expect(host.sent()).toEqual([
      { type: 'navigation', held: true },
      { type: 'navigation', held: false },
    ]);
  });
});
