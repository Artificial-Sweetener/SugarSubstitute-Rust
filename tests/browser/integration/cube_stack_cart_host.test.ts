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
/** Prove a modal cart retires late work and retains actionable failed drafts through the shared dialog owner. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { nextTick } from 'vue';
import { CubeStackCartHost } from '../../../frontend/comfyui/ui/picker/CubeStackCartHost.js';
import { CubeStackCartSession } from '../../../frontend/comfyui/ui/picker/application/CubeStackCartSession.js';
import { FluentAppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceState.js';
import { createFallbackIconModel } from '../../../frontend/comfyui/ui/core/CubeFallbackIconRenderer.js';
import type { CapturedCubeStackCart } from '../../../frontend/comfyui/ui/picker/infrastructure/CubeStackCartFactory.js';
import type { CubeGraphStackEntry } from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphStackModels.js';
import type { CubeStackLibraryRow } from '../../../frontend/comfyui/ui/picker/CubeStackCartView.js';
import type { CubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphModels.js';

const hosts: CubeStackCartHost[] = [];
beforeEach(() => {
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
});
afterEach(() => {
  for (const host of hosts.splice(0)) host.close();
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
});

test('Cancel while opening disposes late capture without mounting or applying it', async () => {
  const f = fixture();
  let finish: ((capture: CapturedCubeStackCart) => void) | undefined;
  const opening = new Promise<CapturedCubeStackCart>((resolve) => {
    finish = resolve;
  });
  const host = f.host(() => opening);
  const result = host.open();
  expect(button('Apply').disabled).toBe(true);
  button('Cancel').click();
  finish?.(f.capture);
  await result;
  expect(document.querySelector('.sc-cube-stack-cart')).toBeNull();
  expect(() => f.capture.session.add('artifact')).toThrow('closed');
  expect(f.commit).not.toHaveBeenCalled();
});

test('Apply keeps failed edits visible, uses literal error text and permits one successful retry', async () => {
  const f = fixture();
  f.plan.mockRejectedValueOnce(new Error('<b>Fixed by wiring</b>'));
  const host = f.host(async () => f.capture);
  const result = host.open();
  await settle();
  document.querySelector<HTMLButtonElement>('[aria-label="Cube library"] button')!.click();
  button('Apply').click();
  await settle();
  expect(document.querySelector('.sugarcubes-modal__error')?.textContent).toBe(
    '<b>Fixed by wiring</b>',
  );
  expect(document.querySelector('.sugarcubes-modal__error b')).toBeNull();
  expect(f.capture.session.changed).toBe(true);
  expect(button('Apply').disabled).toBe(false);
  button('Apply').click();
  await result;
  await settle();
  expect(f.commit).toHaveBeenCalledTimes(1);
  expect(f.applied).toHaveBeenCalledTimes(1);
  expect(f.errors).toHaveBeenCalledTimes(1);
});

test('renderer/workflow teardown invalidates a pending plan before any native commit', async () => {
  const f = fixture();
  let finish: ((analysis: CubeGraphAnalysis) => void) | undefined;
  f.plan.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const host = f.host(async () => f.capture);
  const result = host.open();
  await settle();
  document.querySelector<HTMLButtonElement>('[aria-label="Cube library"] button')!.click();
  button('Apply').click();
  expect(button('Cancel').disabled).toBe(true);
  expect(button('Reset').disabled).toBe(true);
  host.close();
  finish?.(analysis(['new-1']));
  await result;
  await settle();
  expect(f.commit).not.toHaveBeenCalled();
  expect(f.applied).not.toHaveBeenCalled();
});

test('Reset restores the captured draft and appearance changes retain search and row controls', async () => {
  const f = fixture();
  const host = f.host(async () => f.capture);
  const result = host.open();
  await settle();
  document.querySelector<HTMLButtonElement>('[aria-label="Cube library"] button')!.click();
  await settle();
  const row = document.querySelector('[aria-label="Cube cart"] button');
  const input = document.querySelector<HTMLInputElement>('input.sc-cube-stack-cart__search')!;
  input.value = 'cube';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  f.appearance.update(false, false);
  await settle();
  expect(document.querySelector('[aria-label="Cube cart"] button')).toBe(row);
  expect(input.value).toBe('cube');
  button('Reset').click();
  await settle();
  expect(f.capture.session.entries).toHaveLength(0);
  expect(button('Apply').disabled).toBe(true);
  host.close();
  await result;
});

test('Refresh preserves draft, search and row identity while Cancel remains available', async () => {
  const f = fixture();
  let finish: (() => void) | undefined;
  f.capture.refreshLibrary = () =>
    new Promise<void>((resolve) => {
      finish = resolve;
    });
  const host = f.host(async () => f.capture);
  const result = host.open();
  await settle();
  document.querySelector<HTMLButtonElement>('[aria-label="Cube library"] button')!.click();
  await settle();
  const row = document.querySelector('[aria-label="Cube cart"] button');
  const input = document.querySelector<HTMLInputElement>('input.sc-cube-stack-cart__search')!;
  input.value = 'cube';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  button('Refresh').click();
  expect(button('Apply').disabled).toBe(true);
  expect(button('Reset').disabled).toBe(true);
  expect(button('Cancel').disabled).toBe(false);
  finish?.();
  await settle();
  expect(document.querySelector('[aria-label="Cube cart"] button')).toBe(row);
  expect(input.value).toBe('cube');
  expect(button('Apply').disabled).toBe(false);
  button('Cancel').click();
  await result;
  expect(f.commit).not.toHaveBeenCalled();
});

test('Refresh failure remains literal and retryable; closing ignores a late refresh', async () => {
  const f = fixture();
  const refresh = jest.fn<() => Promise<void>>();
  refresh.mockRejectedValueOnce(new Error('<b>Unavailable</b>'));
  let finish: (() => void) | undefined;
  refresh.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  f.capture.refreshLibrary = refresh;
  const host = f.host(async () => f.capture);
  const result = host.open();
  await settle();
  button('Refresh').click();
  await settle();
  expect(document.querySelector('.sugarcubes-modal__error')?.textContent).toBe(
    '<b>Unavailable</b>',
  );
  expect(document.querySelector('.sugarcubes-modal__error b')).toBeNull();
  expect(button('Refresh').disabled).toBe(false);
  button('Refresh').click();
  button('Cancel').click();
  await result;
  finish?.();
  await settle();
  expect(document.querySelector('.sugarcubes-modal-overlay')?.classList.contains('is-open')).toBe(
    false,
  );
  expect(f.commit).not.toHaveBeenCalled();
  expect(f.errors).toHaveBeenCalledTimes(1);
});

/** Query an actual shared shell control without depending on implementation fields. */
function button(label: string): HTMLButtonElement {
  const found = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
    (item) => item.textContent === label,
  );
  if (!found) throw new Error(`Missing control ${label}`);
  return found;
}
/** Settle asynchronous capture, Vue projection and application completion without real timers. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await nextTick();
  await Promise.resolve();
  await nextTick();
}
/** Bind a real application session while replacing only external planning and native commit. */
function fixture() {
  const snapshot = { revision: 'opening', serializedWorkflow: '{}' };
  const commit = jest.fn();
  const plan = jest.fn(
    async (_snapshot: typeof snapshot, entries: readonly CubeGraphStackEntry[]) =>
      analysis(entries.map((entry) => entry.instanceId)),
  );
  const appearance = new FluentAppearanceState();
  const artifactRows = new Map<string, CubeStackLibraryRow>();
  const capture: CapturedCubeStackCart = {
    session: new CubeStackCartSession({
      snapshot,
      analysis: analysis([]),
      artifacts: [
        {
          catalogKey: 'artifact',
          cubeId: 'cube',
          version: '1',
          alias: 'Cube',
          document: { cube_id: 'cube', version: '1' },
          size: [320, 180],
        },
      ],
      snapshots: { available: true, snapshot: () => snapshot },
      gateway: { applyStack: plan },
      commit: { commit },
      createId: () => 'new-1',
    }),
    existingRows: new Map(),
    artifactRows,
    refreshLibrary: async () => undefined,
    library: [
      {
        id: 'artifact',
        catalogKey: 'artifact',
        label: 'Cube',
        secondary: 'v1',
        tooltip: 'Cube',
        icon: createFallbackIconModel('Cube'),
        targetModel: 'SDXL',
        selected: false,
        bypassed: false,
        severity: 'none',
        models: ['Model'],
        role: 'Start cubes',
        searchText: 'cube',
      },
    ],
  };
  for (const row of capture.library) artifactRows.set(row.catalogKey, row);
  const errors = jest.fn();
  const applied = jest.fn(async () => undefined);
  return {
    capture,
    commit,
    plan,
    appearance,
    errors,
    applied,
    host: (open: () => Promise<CapturedCubeStackCart>) => {
      const host = new CubeStackCartHost({
        document,
        appearance,
        factory: { open },
        reportError: errors,
        onApplied: applied,
      });
      hosts.push(host);
      return host;
    },
  };
}
/** Return a complete canonical new-instance result for the cart's exact staged order. */
function analysis(ids: readonly string[]): CubeGraphAnalysis {
  return {
    semanticHash: 'hash',
    instances: ids.map((instanceId) => ({
      instanceId,
      nodeId: instanceId,
      definitionId: instanceId,
      cubeId: 'cube',
      version: '1',
      alias: 'Cube',
      executionMode: 0,
    })),
    segments: [{ instanceIds: ids, reorderable: true, boundaryNodeIds: [] }],
    placements: ids.map((instanceId, index) => ({
      instanceId,
      nodeId: instanceId,
      position: [index * 344, 0],
    })),
  };
}
