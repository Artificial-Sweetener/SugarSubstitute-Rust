/** @jest-environment node */
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
/** Prove committed native edits remain the user's undo steps while panel reloads retain their mask association. */
import { jest } from '@jest/globals';
import { build } from 'esbuild';
import { runInNewContext } from 'node:vm';
import type {
  CanvasMaskCommand,
  CanvasMaskResult,
} from '../../../integrations/contracts/CanvasMasksProtocol.generated.js';
import type {
  PhotoshopMaskEditingState,
  PhotoshopMaskSessionPort,
} from '../../../integrations/photoshop/src/infrastructure/PhotoshopMaskSession.js';
import { BUILD_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BuildFixtureBudget.js';

interface NativeMasks {
  execute(command: CanvasMaskCommand): Promise<CanvasMaskResult>;
  dispose(): void;
}
let code = '';
beforeAll(async () => {
  const bundle = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopCanvasMasks.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
    external: ['photoshop'],
  });
  const file = bundle.outputFiles[0];
  if (!file) throw new Error('Missing native mask adapter bundle.');
  code = file.text;
}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);

test('observes committed selection edits without adding history and restores the same editing association', async () => {
  const channels = Object.assign([] as { name: string; typename: string }[], {
    add: () => {
      const channel = { name: '', typename: 'AlphaChannel' };
      channels.push(channel);
      return channel;
    },
    getByName: (name: string) => channels.find((channel) => channel.name === name),
  });
  const doc = {
    id: 1,
    width: 100,
    height: 100,
    mode: 'RGB',
    bitsPerChannel: 'bitDepth8',
    colorProfileName: 'sRGB',
    pixelAspectRatio: 1,
    quickMaskMode: false,
    artboards: [],
    activeHistoryState: { id: 1 },
    channels,
    activeChannels: [] as { name: string }[],
    componentChannels: [],
    selection: {
      bounds: { left: 10, top: 20, right: 30, bottom: 40 },
      saveTo: jest.fn(async () => {}),
    },
  };
  const history: string[] = [];
  let editing: PhotoshopMaskEditingState | null = null;
  const session: PhotoshopMaskSessionPort = {
    read: () => editing,
    write: (value) => {
      editing = value;
    },
  };
  let tick = () => {};
  const changed = jest.fn(),
    report = jest.fn();
  const module: { exports: Record<string, unknown> } = { exports: {} };
  runInNewContext(code, {
    module,
    crypto: { randomUUID: () => 'region-1' },
    setInterval: (callback: () => void) => {
      tick = callback;
      return 1;
    },
    clearInterval: () => {},
    require: () => ({
      app: { activeDocument: doc, documents: [doc] },
      core: {
        executeAsModal: async (action: (execution: unknown) => Promise<void>) => {
          await action({
            hostControl: {
              suspendHistory: async ({ name }: { name: string }) => name,
              resumeHistory: async (name: string, commit: boolean) => {
                if (commit) {
                  history.push(name);
                  doc.activeHistoryState = { id: doc.activeHistoryState.id + 1 };
                }
              },
            },
          });
        },
      },
    }),
  });
  // The compiled native boundary has a separate constructor identity from this test environment.
  const Constructor = module.exports.PhotoshopCanvasMasks as new (
    changed: (documentId: string, maskId: string, revision: string) => void,
    report: (error: unknown) => void,
    session: PhotoshopMaskSessionPort,
  ) => NativeMasks;
  const first = new Constructor(changed, report, session);
  const context = await first.execute({ type: 'context' });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing native context.');
  const created = await first.execute({
    type: 'create',
    source: 'selection',
    document: context.document,
    name: 'Region',
    pixels: null,
  });
  if (created.kind !== 'created') throw new Error('Missing created mask.');
  const before = [...history];
  doc.activeHistoryState = { id: 10 };
  tick();
  for (let n = 0; n < 16; n++) await Promise.resolve();
  expect(changed).toHaveBeenLastCalledWith('1', created.mask.id, '10');
  expect(history).toEqual(before);
  first.dispose();
  expect(editing).toEqual({ mask: created.mask, revision: '10' });
  const second = new Constructor(changed, report, session);
  doc.activeHistoryState = { id: 11 };
  tick();
  for (let n = 0; n < 16; n++) await Promise.resolve();
  expect(changed).toHaveBeenLastCalledWith('1', created.mask.id, '11');
  expect(history).toEqual(before);
  expect(report).not.toHaveBeenCalled();
  second.dispose();
  const notifications = changed.mock.calls.length;
  doc.activeHistoryState = { id: 20 };
  const stale = new Constructor(changed, report, session);
  doc.activeHistoryState = { id: 21 };
  tick();
  for (let n = 0; n < 16; n++) await Promise.resolve();
  expect(changed).toHaveBeenCalledTimes(notifications);
  stale.dispose();
});

test('validates retained editing references and retires released state', async () => {
  const bundle = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopMaskSession.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
  });
  const file = bundle.outputFiles[0];
  if (!file) throw new Error('Missing native association storage bundle.');
  const values = new Map<string, string>(),
    report = jest.fn();
  const module: { exports: Record<string, unknown> } = { exports: {} };
  runInNewContext(file.text, {
    module,
    localStorage: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
      removeItem: (key: string) => {
        values.delete(key);
      },
    },
  });
  // Compile the real native storage boundary with an isolated plugin store.
  const Constructor = module.exports.PhotoshopMaskSession as new (
    report: (error: unknown) => void,
  ) => PhotoshopMaskSessionPort;
  const session = new Constructor(report);
  const editing = {
    mask: {
      canvasId: 'photoshop',
      document: { id: '1', geometryRevision: 'g', width: 100, height: 100 },
      id: 'region-1',
    },
    revision: '10',
  };
  session.write(editing);
  expect(session.read()).toEqual(editing);
  expect(report).not.toHaveBeenCalled();
  const key = [...values.keys()][0]!;
  values.set(key, JSON.stringify({ ...editing, mask: { id: 'wrong' } }));
  expect(session.read()).toBeNull();
  values.set(key, '{broken');
  expect(session.read()).toBeNull();
  expect(report).toHaveBeenCalledTimes(2);
  session.write(null);
  expect(session.read()).toBeNull();
  expect(values.size).toBe(0);
});
