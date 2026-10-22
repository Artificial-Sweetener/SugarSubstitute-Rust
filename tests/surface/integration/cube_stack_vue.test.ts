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
/** Prove safe stack rendering, stable focused controls and canonical keyboard/drag intent. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { nextTick } from 'vue';
import { createFallbackIconModel } from '../../../frontend/comfyui/ui/core/CubeFallbackIconRenderer.js';
import { mountCubeStackView } from '../../../frontend/comfyui/ui/surface/stack/vue/CubeStackVueRuntime.js';
import { ensureCubeStackStyles } from '../../../frontend/comfyui/ui/surface/stack/CubeStackStyles.js';
import type {
  CubeStackNavigationRow,
  CubeStackViewState,
  CubeStackViewActions,
} from '../../../frontend/comfyui/ui/surface/stack/CubeStackView.js';

const mounted: (() => void)[] = [];
afterEach(() => {
  for (const dispose of mounted.splice(0)) dispose();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('keeps the normal arrow cursor over reorderable cards and throughout a pointer drag', async () => {
  ensureCubeStackStyles(document);
  const f = setup();
  expect(getComputedStyle(f.activate(0)).cursor).toBe('default');
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 90));
  await nextTick();
  expect(getComputedStyle(f.rail).cursor).toBe('default');
  expect(getComputedStyle(f.activate(0)).cursor).toBe('default');
});

test.each(['fluent', 'comfy'] as const)(
  'keeps the model capsule over the icon in compact and expanded %s rows',
  async (appearance) => {
    const f = setup();
    const button = f.activate(0);
    button.focus();
    const asset = button.querySelector('.sc-cube-stack-row__asset');
    for (const compact of [true, false]) {
      f.view.update({ ...f.state, appearance, compact });
      await nextTick();
      const icon = button.querySelector('.sc-cube-stack-row__icon');
      expect(icon?.querySelector('.sc-cube-stack-row__model')?.textContent).toBe('SDXL');
      expect(icon?.getAttribute('aria-hidden')).toBe('true');
      expect(button.getAttribute('aria-description')).toBe('Model: SDXL');
      expect(button.querySelector('.sc-cube-stack-row__asset')).toBe(asset);
      expect(document.activeElement).toBe(button);
    }
    const literal = '<img src=x onerror=alert(1)>';
    f.view.update({ ...f.state, rows: [{ ...f.state.rows[0]!, targetModel: literal }] });
    await nextTick();
    expect(button.querySelector('.sc-cube-stack-row__model')?.textContent).toBe(literal);
    expect(button.querySelector('img')).toBeNull();
    f.view.update({ ...f.state, rows: [{ ...f.state.rows[0]!, targetModel: '' }] });
    await nextTick();
    expect(button.querySelector('.sc-cube-stack-row__model')).toBeNull();
    expect(button.hasAttribute('aria-description')).toBe(false);
  },
);

test('renders markup-like cube metadata as literal text and routes navigation and removal by stable identity', async () => {
  const fixture = setup();
  const literal = '<img src=x onerror=alert(1)>';
  fixture.view.update({
    ...fixture.state,
    rows: [
      { ...fixture.state.rows[0]!, label: literal, secondary: '<b>version</b>', tooltip: literal },
    ],
  });
  await nextTick();
  expect(fixture.element.querySelector('img')).toBeNull();
  expect(fixture.element.textContent).toContain(literal);
  expect(fixture.element.textContent).toContain('<b>version</b>');
  fixture.activate(0).click();
  fixture.element.querySelector<HTMLButtonElement>('.sc-cube-stack-row__remove')!.click();
  expect(fixture.actions.navigate).toHaveBeenCalledWith('first');
  expect(fixture.actions.remove).toHaveBeenCalledWith('first');
});

test('retains the focused navigation button while order or busy presentation changes', async () => {
  const fixture = setup();
  const original = fixture.activate(0);
  original.focus();
  fixture.view.update({ ...fixture.state, rows: [...fixture.state.rows].reverse(), pending: true });
  await nextTick();
  expect(fixture.element.querySelectorAll('.sc-cube-stack-row__activate')[1]).toBe(original);
  expect(document.activeElement).toBe(original);
  expect(original.disabled).toBe(false);
});

test('provides keyboard movement and focus navigation without changing graph state locally', async () => {
  const fixture = setup();
  fixture.activate(0).dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'ArrowDown',
      altKey: true,
      bubbles: true,
      cancelable: true,
    }),
  );
  await nextTick();
  expect(fixture.actions.move).toHaveBeenCalledWith('first', 1);
  fixture
    .activate(0)
    .dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
    );
  expect(document.activeElement).toBe(fixture.activate(1));
  expect(fixture.state.rows.map((row) => row.id)).toEqual(['first', 'second']);
});

test('previews a drag once and submits only its complete Cube permutation', async () => {
  const fixture = setup();
  fixture.row(0).dispatchEvent(pointer('pointerdown', 23));
  fixture.rail.dispatchEvent(pointer('pointermove', 130));
  fixture.rail.dispatchEvent(pointer('pointermove', 130));
  await nextTick();
  expect(fixture.actions.previewOrder).toHaveBeenCalledTimes(1);
  expect(fixture.actions.previewOrder).toHaveBeenCalledWith(['second', 'first']);
  fixture.rail.dispatchEvent(pointer('pointerup', 130));
  await nextTick();
  expect(fixture.actions.reorder).toHaveBeenCalledWith('first', ['second', 'first']);
});

test('animates fixed row slots without measuring every row during a preview update', async () => {
  const fixture = setup();
  fixture.row(0).dispatchEvent(pointer('pointerdown', 23));
  fixture.rail.dispatchEvent(pointer('pointermove', 130));
  const measurements = [0, 1].map((index) =>
    jest.spyOn(fixture.row(index), 'getBoundingClientRect').mockClear(),
  );
  await nextTick();
  for (const measurement of measurements) expect(measurement).not.toHaveBeenCalled();
  expect(fixture.row(0).style.transform).toBe('translateY(0px)');
  expect(fixture.row(1).style.transform).toBe('translateY(50px)');
  fixture.rail.dispatchEvent(pointer('pointerup', 130));
  await nextTick();
});

test('cancels captured drag with Escape without mutating the workflow', async () => {
  const fixture = setup();
  fixture.row(0).dispatchEvent(pointer('pointerdown', 23));
  fixture.rail.dispatchEvent(pointer('pointermove', 130));
  await nextTick();
  expect(fixture.actions.previewOrder).toHaveBeenCalledWith(['second', 'first']);
  document.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
  );
  fixture.rail.dispatchEvent(pointer('pointerup', 130));
  await nextTick();
  expect(fixture.actions.reorder).not.toHaveBeenCalled();
  expect(fixture.actions.previewOrder).toHaveBeenLastCalledWith(null);
  expect(fixture.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
});

test('reverts temporary preview and reports an application rejection through SugarCubes feedback', async () => {
  const fixture = setup();
  fixture.actions.reorder.mockRejectedValue(new Error('Workflow changed'));
  fixture.row(0).dispatchEvent(pointer('pointerdown', 23));
  fixture.rail.dispatchEvent(pointer('pointermove', 130));
  await nextTick();
  fixture.rail.dispatchEvent(pointer('pointerup', 130));
  await nextTick();
  await nextTick();
  expect(fixture.actions.reportError).toHaveBeenCalledWith(
    expect.objectContaining({ message: 'Workflow changed' }),
  );
  expect(fixture.actions.previewOrder).toHaveBeenLastCalledWith(null);
  expect(fixture.element.querySelector('.sc-cube-stack-row')?.getAttribute('data-row-id')).toBe(
    'first',
  );
});

test('supports two accepted drops followed by navigation, compact toggles and another drag', async () => {
  const f = setup();
  let accepted = f.state;
  f.actions.reorder.mockImplementation(async (_id, order) => {
    accepted = {
      ...accepted,
      rows: [...accepted.rows].sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id)),
    };
    f.view.update(accepted);
    await nextTick();
  });
  const marker = f.element.querySelector('.sc-cube-stack__selection');
  const firstButton = f.activate(0);
  for (let drop = 0; drop < 2; drop += 1) {
    f.row(0).dispatchEvent(pointer('pointerdown', 23));
    f.rail.dispatchEvent(pointer('pointermove', 130));
    await nextTick();
    f.rail.dispatchEvent(pointer('pointerup', 130));
    for (let tick = 0; tick < 6; tick += 1) await nextTick();
    f.rail.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
    expect(f.row(0).hasAttribute('data-reorderable')).toBe(true);
  }
  expect(f.actions.reorder).toHaveBeenCalledTimes(2);
  expect(f.activate(0)).toBe(firstButton);
  f.activate(1).click();
  expect(f.actions.navigate).toHaveBeenLastCalledWith('second');
  accepted = {
    ...accepted,
    compact: true,
    rows: accepted.rows.map((row) => ({ ...row, selected: row.id === 'second' })),
  };
  f.view.update(accepted);
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__selection')).toBe(marker);
  expect(marker?.getAttribute('data-position')).toBe('1');
  expect(marker?.getAttribute('style')).toContain('translateY(65px)');
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 130));
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).not.toBeNull();
});

test('cancels captured drag intent while the drawer leaves and permits dragging after re-entry', async () => {
  const f = setup();
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 73));
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).not.toBeNull();
  f.view.setInteractive(false);
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
  expect(f.actions.previewOrder).toHaveBeenLastCalledWith(null);
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 73));
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
  expect(f.actions.reorder).not.toHaveBeenCalled();
  f.view.setInteractive(true);
  await nextTick();
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 73));
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).not.toBeNull();
});

test('permutes three cubes repeatedly with a displaced shadow and stable retained controls', async () => {
  const f = setup(['start', 'middle', 'end']);
  let accepted = f.state;
  const controls = new Map(accepted.rows.map((row, index) => [row.id, f.activate(index)]));
  f.actions.reorder.mockImplementation(async (_id, order) => {
    accepted = {
      ...accepted,
      rows: order.map((id) => accepted.rows.find((row) => row.id === id)!),
    };
    f.view.update(accepted);
    await nextTick();
  });
  for (const [from, to, expected] of [
    [0, 2, ['middle', 'end', 'start']],
    [1, 0, ['end', 'middle', 'start']],
    [2, 0, ['start', 'end', 'middle']],
    [1, 2, ['start', 'middle', 'end']],
  ] as const) {
    const id = accepted.rows[from]!.id;
    f.row(from).dispatchEvent(pointer('pointerdown', from * 50 + 23));
    f.rail.dispatchEvent(pointer('pointermove', to * 50 + (to === 0 ? 1 : 73)));
    await nextTick();
    expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).not.toBeNull();
    expect(f.element.querySelector('.sc-cube-stack-row[data-dragging]')).not.toBeNull();
    f.rail.dispatchEvent(pointer('pointerup', to * 50 + (to === 0 ? 1 : 73)));
    for (let tick = 0; tick < 6; tick += 1) await nextTick();
    f.rail.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    expect(f.actions.reorder).toHaveBeenLastCalledWith(id, expected);
    expect(accepted.rows.map((row) => row.id)).toEqual(expected);
    expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
    for (const [index, row] of accepted.rows.entries())
      expect(f.activate(index)).toBe(controls.get(row.id));
  }
  expect(f.actions.reorder).toHaveBeenCalledTimes(4);
  expect(f.actions.reportError).not.toHaveBeenCalled();
});

test('resumes a held regrab at the drop point after the previous graph commit settles', async () => {
  const f = setup(['start', 'middle', 'end']);
  let accepted = f.state;
  let accept: (() => void) | undefined;
  f.actions.reorder.mockImplementation(async (_id, order) => {
    f.view.update({
      ...accepted,
      pending: true,
      rows: accepted.rows.map((row) => ({ ...row, reorderable: false })),
    });
    await new Promise<void>((resolve) => {
      accept = resolve;
    });
    accepted = {
      ...accepted,
      rows: order.map((id) => accepted.rows.find((row) => row.id === id)!),
    };
    f.view.update(accepted);
    await nextTick();
  });
  const start = f.activate(0);
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 123));
  await nextTick();
  f.rail.dispatchEvent(pointer('pointerup', 123));
  await nextTick();
  expect(f.row(2).dataset.rowId).toBe('start');
  f.row(2).dispatchEvent(pointer('pointerdown', 123));
  f.rail.dispatchEvent(pointer('pointermove', 10));
  await nextTick();
  expect(f.actions.reorder).toHaveBeenCalledTimes(1);
  accept!();
  for (let tick = 0; tick < 6; tick += 1) await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).not.toBeNull();
  expect(f.row(0).dataset.rowId).toBe('start');
  f.rail.dispatchEvent(pointer('pointerup', 10));
  await nextTick();
  expect(f.actions.reorder).toHaveBeenLastCalledWith('start', ['start', 'middle', 'end']);
  accept!();
  for (let tick = 0; tick < 6; tick += 1) await nextTick();
  expect(f.activate(0)).toBe(start);
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
  expect(f.actions.navigate).not.toHaveBeenCalled();
});

test.each(['pointer', 'keyboard'] as const)(
  'preserves the first %s toolbar activation after a drop without a generated click',
  async (input) => {
    const f = setup();
    f.row(0).dispatchEvent(pointer('pointerdown', 23));
    f.rail.dispatchEvent(pointer('pointermove', 73));
    await nextTick();
    f.rail.dispatchEvent(pointer('pointerup', 73));
    for (let tick = 0; tick < 6; tick += 1) await nextTick();
    const tool = f.element.querySelector<HTMLElement>('.sc-cube-stack__tool')!;
    if (input === 'pointer') tool.dispatchEvent(pointer('pointerdown', 0));
    tool.dispatchEvent(
      new MouseEvent('click', {
        bubbles: true,
        cancelable: true,
        detail: input === 'pointer' ? 1 : 0,
      }),
    );
    expect(f.actions.setCompact).toHaveBeenCalledWith(true);
  },
);

test.each(['release', 'cancel', 'escape'] as const)(
  'discards a pending regrab on %s and permits another drag without leaving the card',
  async (ending) => {
    const f = setup();
    let accept: (() => void) | undefined;
    f.actions.reorder.mockImplementationOnce(async (_id, order) => {
      f.view.update({ ...f.state, pending: true });
      await new Promise<void>((resolve) => {
        accept = resolve;
      });
      f.view.update({
        ...f.state,
        rows: order.map((id) => f.state.rows.find((row) => row.id === id)!),
      });
      await nextTick();
    });
    f.row(0).dispatchEvent(pointer('pointerdown', 23));
    f.rail.dispatchEvent(pointer('pointermove', 73));
    await nextTick();
    f.rail.dispatchEvent(pointer('pointerup', 73));
    await nextTick();
    f.row(1).dispatchEvent(pointer('pointerdown', 73));
    f.rail.dispatchEvent(pointer('pointermove', 10));
    if (ending === 'escape')
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    else f.rail.dispatchEvent(pointer(ending === 'release' ? 'pointerup' : 'pointercancel', 10));
    await nextTick();
    expect([f.row(0).dataset.rowId, f.row(1).dataset.rowId]).toEqual(['second', 'first']);
    expect(f.actions.previewOrder).toHaveBeenLastCalledWith(['second', 'first']);
    accept!();
    for (let tick = 0; tick < 6; tick += 1) await nextTick();
    expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
    expect(f.actions.reorder).toHaveBeenCalledTimes(1);
    f.row(0).dispatchEvent(pointer('pointerdown', 10));
    f.rail.dispatchEvent(pointer('pointermove', 73));
    await nextTick();
    expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).not.toBeNull();
  },
);

test('suppresses only the drop click and retains the next ordinary cube navigation', async () => {
  const f = setup();
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 73));
  await nextTick();
  f.rail.dispatchEvent(pointer('pointerup', 73));
  for (let tick = 0; tick < 6; tick += 1) await nextTick();
  const click = () => new MouseEvent('click', { bubbles: true, cancelable: true, detail: 1 });
  f.activate(0).dispatchEvent(click());
  expect(f.actions.navigate).not.toHaveBeenCalled();
  f.activate(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointerup', 23));
  f.activate(0).dispatchEvent(click());
  expect(f.actions.navigate).toHaveBeenCalledWith('first');
});

test('reverts a rejected submitted order after a quick regrab release and permits retry', async () => {
  const f = setup();
  let reject: (() => void) | undefined;
  f.actions.reorder.mockImplementationOnce(async () => {
    f.view.update({ ...f.state, pending: true });
    await new Promise<void>((resolve) => {
      reject = resolve;
    });
    f.view.update(f.state);
    throw new Error('Workflow changed');
  });
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 73));
  await nextTick();
  f.rail.dispatchEvent(pointer('pointerup', 73));
  await nextTick();
  f.row(1).dispatchEvent(pointer('pointerdown', 73));
  f.rail.dispatchEvent(pointer('pointerup', 73));
  await nextTick();
  expect(f.row(1).dataset.rowId).toBe('first');
  reject!();
  for (let tick = 0; tick < 6; tick += 1) await nextTick();
  expect(f.actions.reportError).toHaveBeenCalledWith(
    expect.objectContaining({ message: 'Workflow changed' }),
  );
  expect(f.actions.previewOrder).toHaveBeenLastCalledWith(null);
  expect(f.row(0).dataset.rowId).toBe('first');
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
  f.row(1).dispatchEvent(pointer('pointerdown', 73));
  f.rail.dispatchEvent(pointer('pointermove', 10));
  await nextTick();
  f.rail.dispatchEvent(pointer('pointerup', 10));
  expect(f.actions.reorder).toHaveBeenLastCalledWith('second', ['second', 'first']);
});

test('discards held intent when refreshed graph availability still excludes the cube', async () => {
  const f = setup();
  const unavailable = f.state.rows.map((row) => ({ ...row, reorderable: false }));
  f.view.update({ ...f.state, rows: unavailable, pending: true });
  await nextTick();
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 73));
  f.view.update({ ...f.state, rows: unavailable });
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
  f.rail.dispatchEvent(pointer('pointerup', 73));
  expect(f.actions.reorder).not.toHaveBeenCalled();
  f.view.update(f.state);
  await nextTick();
  f.row(1).dispatchEvent(pointer('pointerdown', 73));
  f.rail.dispatchEvent(pointer('pointermove', 10));
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).not.toBeNull();
});

/** Bind real Vue state and immutable row identities while isolating graph and canvas boundaries. */
function setup(ids: readonly string[] = ['first', 'second']) {
  // Synthetic events need a later turn than Vue listener registration, including capture-to-target delivery.
  let eventTime = Date.now();
  jest.spyOn(Date, 'now').mockImplementation(() => ++eventTime);
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  const rows: CubeStackNavigationRow[] = ids.map((id) => ({
    id,
    label: id,
    secondary: 'v1.0.0',
    tooltip: id,
    icon: createFallbackIconModel('CU'),
    targetModel: 'SDXL',
    selected: id === 'first',
    bypassed: false,
    severity: 'none',
    reorderable: true,
  }));
  const state: CubeStackViewState = { rows, compact: false, pending: false, appearance: 'comfy' };
  const actions = {
    navigate: jest.fn<(id: string) => void>(),
    previewOrder: jest.fn<(order: readonly string[] | null) => void>(),
    reorder: jest.fn<CubeStackViewActions['reorder']>(async () => undefined),
    move: jest.fn<CubeStackViewActions['move']>(async () => undefined),
    openCart: jest.fn(),
    setCompact: jest.fn(),
    hide: jest.fn(),
    remove: jest.fn(),
    contextMenu: jest.fn(),
    reportError: jest.fn(),
  };
  const element = document.createElement('div');
  document.body.append(element);
  const view = mountCubeStackView(element, state, actions);
  const rail = element.querySelector<HTMLElement>('.sc-cube-stack')!;
  rail.setPointerCapture = jest.fn();
  rail.hasPointerCapture = jest.fn(() => true);
  rail.releasePointerCapture = jest.fn();
  const rect = (top: number, height: number) => ({
    x: 0,
    y: top,
    left: 0,
    right: 224,
    top,
    bottom: top + height,
    width: 224,
    height,
    toJSON: () => ({}),
  });
  jest.spyOn(rail, 'getBoundingClientRect').mockImplementation(() => rect(0, 200));
  jest.spyOn(element, 'getBoundingClientRect').mockImplementation(() => rect(0, 200));
  const list = rail.querySelector<HTMLElement>('.sc-cube-stack__rows')!;
  jest.spyOn(list, 'getBoundingClientRect').mockImplementation(() => rect(0, 100));
  for (const row of list.querySelectorAll<HTMLElement>('.sc-cube-stack-row')) {
    Object.defineProperty(row, 'offsetTop', {
      get: () => [...list.querySelectorAll('.sc-cube-stack-row')].indexOf(row) * 50,
    });
    jest.spyOn(row, 'getBoundingClientRect').mockImplementation(() => rect(row.offsetTop, 46));
  }
  mounted.push(view.dispose);
  return {
    view,
    rail,
    state,
    actions,
    element,
    activate: (index: number) =>
      element.querySelectorAll<HTMLButtonElement>('.sc-cube-stack-row__activate')[index]!,
    row: (index: number) =>
      element.querySelectorAll<HTMLElement>('.sc-cube-stack__rows > .sc-cube-stack-row')[index]!,
  };
}

/** Supply pointer fields omitted by jsdom while exercising the actual Vue event handlers. */
function pointer(type: string, y: number, id = 7, x = 20): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    button: { value: 0 },
    pointerId: { value: id },
    clientX: { value: x },
    clientY: { value: y },
  });
  return event;
}

test.each([
  { x: 180, y: 8 },
  { x: 12, y: 40 },
])('retains the exact grab point during drag and scaling at %j', async ({ x, y }) => {
  const f = setup();
  f.row(0).dispatchEvent(pointer('pointerdown', y, 7, x));
  f.rail.dispatchEvent(pointer('pointermove', y + 60, 7, x + 10));
  await nextTick();
  const overlay = f.element.querySelector<HTMLElement>('.sc-cube-stack__drag-overlay');
  expect(overlay?.style.transform).toBe('translate3d(10px,60px,0)');
  expect(overlay?.querySelector<HTMLElement>('.sc-cube-stack-row')?.style.transformOrigin).toBe(
    `${x}px ${y}px`,
  );
  f.rail.dispatchEvent(pointer('pointermove', y + 70, 7, x + 20));
  await nextTick();
  expect(overlay?.style.transform).toBe('translate3d(20px,70px,0)');
});

test('reserves a moving shadow slot, keeps a lifted preview, and never starts a native MIME drag', async () => {
  const f = setup();
  const first = f.row(0);
  const button = f.activate(0);
  button.focus();
  const text = button.querySelector('.sc-cube-stack-row__text');
  first.dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 25));
  await nextTick();
  expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
  f.rail.dispatchEvent(pointer('pointermove', 130));
  await nextTick();
  expect(f.row(1)).toBe(first);
  expect(first.hasAttribute('data-dragging')).toBe(true);
  expect(
    f.element.querySelector('.sc-cube-stack__drag-overlay')?.getAttribute('inert'),
  ).not.toBeNull();
  expect(f.rail.setPointerCapture).toHaveBeenCalledWith(7);
  const native = new Event('dragstart', { bubbles: true, cancelable: true });
  const host = jest.fn();
  document.addEventListener('dragstart', host);
  first.dispatchEvent(native);
  expect(native.defaultPrevented).toBe(true);
  expect(host).not.toHaveBeenCalled();
  document.removeEventListener('dragstart', host);
  expect(first.draggable).toBe(false);
  expect(button.querySelector('.sc-cube-stack-row__text')).toBe(text);
  document.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
  );
  await nextTick();
  expect(f.row(0)).toBe(first);
  expect(f.actions.reorder).not.toHaveBeenCalled();
  expect(f.actions.previewOrder).toHaveBeenLastCalledWith(null);
});

test('captures only activated drags and preserves ordinary pointer navigation', () => {
  const f = setup();
  f.activate(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 25));
  f.rail.dispatchEvent(pointer('pointerup', 25));
  expect(f.rail.setPointerCapture).not.toHaveBeenCalled();
  f.activate(0).click();
  expect(f.actions.navigate).toHaveBeenCalledWith('first');
  expect(f.actions.previewOrder).not.toHaveBeenCalled();
});

test('keeps the shadow stable while the pointer jitters at a crossed insertion boundary', async () => {
  const f = setup();
  f.row(0).dispatchEvent(pointer('pointerdown', 23));
  f.rail.dispatchEvent(pointer('pointermove', 94));
  await nextTick();
  expect(f.row(1).dataset.rowId).toBe('first');
  for (const y of [92, 95, 91, 96, 93]) {
    f.rail.dispatchEvent(pointer('pointermove', y));
    await nextTick();
    expect(f.row(1).dataset.rowId).toBe('first');
  }
  expect(f.actions.previewOrder).toHaveBeenCalledTimes(1);
});

test.each(['pointercancel', 'lostpointercapture'])(
  'cancels %s and discards foreign pointer movement',
  async (type) => {
    const f = setup();
    f.row(0).dispatchEvent(pointer('pointerdown', 23));
    f.rail.dispatchEvent(pointer('pointermove', 130, 8));
    expect(f.actions.previewOrder).not.toHaveBeenCalled();
    f.rail.dispatchEvent(pointer('pointermove', 130));
    f.rail.dispatchEvent(pointer(type, 130));
    await nextTick();
    expect(f.actions.reorder).not.toHaveBeenCalled();
    expect(f.actions.previewOrder).toHaveBeenLastCalledWith(null);
    expect(f.element.querySelector('.sc-cube-stack__drag-overlay')).toBeNull();
  },
);

test('centers one 16px marker on the active card through selection and compact toggles', async () => {
  ensureCubeStackStyles(document);
  const f = setup();
  const marker = f.element.querySelector<HTMLElement>('.sc-cube-stack__selection')!;
  const text = f.activate(0).querySelector('.sc-cube-stack-row__text');
  f.view.update({
    ...f.state,
    rows: f.state.rows.map((row, index) => ({ ...row, selected: index === 0 })),
    compact: true,
  });
  await nextTick();
  expect(marker.style.transform).toBe('translateY(15px)');
  expect(getComputedStyle(marker).height).toBe('16px');
  expect(getComputedStyle(marker).width).toBe('3px');
  expect(getComputedStyle(marker).transition).toContain('transform 220ms');
  expect(f.element.querySelector('.sc-cube-stack__selection')).toBe(marker);
  expect(f.activate(0).querySelector('.sc-cube-stack-row__text')).toBe(text);
  f.view.update({
    ...f.state,
    rows: f.state.rows.map((row, index) => ({ ...row, selected: index === 1 })),
    compact: false,
  });
  await nextTick();
  expect(marker.style.transform).toBe('translateY(65px)');
  expect(f.activate(0).querySelector('.sc-cube-stack-row__text')).toBe(text);
});

test('opens cube context actions using Shift+F10 and ContextMenu while suppressing pending intent', async () => {
  const f = setup();
  const button = f.activate(0);
  for (const input of [{ key: 'F10', shiftKey: true }, { key: 'ContextMenu' }])
    button.dispatchEvent(
      new KeyboardEvent('keydown', { ...input, bubbles: true, cancelable: true }),
    );
  expect(f.actions.contextMenu).toHaveBeenCalledTimes(2);
  expect(f.actions.contextMenu).toHaveBeenCalledWith('first', expect.any(MouseEvent));
  f.view.update({ ...f.state, pending: true });
  await nextTick();
  button.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  button.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ContextMenu', bubbles: true, cancelable: true }),
  );
  expect(f.actions.contextMenu).toHaveBeenCalledTimes(2);
});
