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
/** Prove native Run-strip docking preserves controls, geometry, and reversible disclosure. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeOverrideToolbarHost } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeOverrideToolbarHost.js';
import { OverrideToolbarPlacement } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarPlacement.js';
import type { OverrideToolbarPlacementState } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarPlacementState.js';
import { OverrideToolbarDrag } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarDrag.js';
import { OverrideToolbarDisclosure } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarDisclosure.js';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
  document.getElementById('sc-relation-styles')?.remove();
});

/** Supply layout bounds at the browser boundary instead of relying on jsdom geometry. */
function bounds(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    left: x,
    top: y,
    width,
    height,
    right: x + width,
    bottom: y + height,
    toJSON: () => ({}),
  };
}

/** Model Comfy's header, fixed native controls, and one width-constrained override toolbar. */
function fixture(saved: OverrideToolbarPlacementState = { docked: false, floating: null }) {
  ensureRelationStyles(document);
  let width = 1000;
  const canvas = document.createElement('div');
  canvas.id = 'graph-canvas-container';
  const panel = document.createElement('div');
  panel.className = 'p-splitterpanel';
  const line = document.createElement('div');
  const workflow = document.createElement('button');
  workflow.setAttribute('aria-label', 'Workflow actions');
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  const run = document.createElement('button');
  run.textContent = 'Run';
  bar.append(run);
  line.append(workflow, bar);
  const graph = document.createElement('div');
  graph.className = 'graph-canvas-panel';
  panel.append(line, graph);
  canvas.append(panel);
  document.body.append(canvas);
  const root = document.createElement('div'),
    fields = document.createElement('div');
  const input = document.createElement('input');
  input.value = '123456789';
  const grip = document.createElement('button'),
    pin = document.createElement('button'),
    button = document.createElement('button');
  root.className = 'sc-overrides-toolbar';
  grip.className = 'sc-overrides-grip';
  pin.className = 'sc-overrides-menu';
  fields.className = 'sc-overrides-fields';
  button.className = 'sc-overrides-disclosure';
  fields.append(input);
  root.append(grip, pin, fields, button);
  panel.getBoundingClientRect = () => bounds(0, 0, width, 600);
  line.getBoundingClientRect = () => bounds(0, 0, width, 48);
  workflow.getBoundingClientRect = () => bounds(4, 0, 100, 48);
  root.getBoundingClientRect = () => {
    const docked = root.dataset.placement === 'docked';
    const preferred = fields.hidden ? 88 : 500;
    const size = docked
      ? Math.min(preferred, Number.parseFloat(root.style.maxWidth) || preferred)
      : preferred;
    return bounds(
      docked ? width - 350 - size - 8 : Number.parseFloat(root.style.left) || 4,
      docked ? 8 : Number.parseFloat(root.style.top) || 52,
      size,
      docked ? 32 : 42,
    );
  };
  grip.getBoundingClientRect = () => {
    const r = root.getBoundingClientRect();
    return bounds(r.x, r.y, 20, 32);
  };
  bar.getBoundingClientRect = () => {
    const slot =
      root.parentElement?.className === 'sc-overrides-dock-slot'
        ? root.getBoundingClientRect().width + 8
        : 0;
    const target = bar.querySelector<HTMLElement>('.sc-overrides-dock-target');
    const targetWidth = target ? Number.parseFloat(target.style.width) + 8 : 0;
    return bounds(width - 350 - slot - targetWidth, 0, 350 + slot + targetWidth, 48);
  };
  run.getBoundingClientRect = () => bounds(width - 350, 0, 350, 48);
  const host = new NativeOverrideToolbarHost(document);
  const changed = jest.fn((docked: boolean) =>
    disclosure.setExpansionDirection(docked ? 'left' : 'right'),
  );
  const write = jest.fn<(state: OverrideToolbarPlacementState) => void>();
  const placement = new OverrideToolbarPlacement(root, grip, host, changed, () => undefined, {
    read: () => saved,
    write,
  });
  const disclosure = new OverrideToolbarDisclosure(
    root,
    fields,
    button,
    (w, h) => placement.resize(w, h),
    jest.fn(),
  );
  const drag = new OverrideToolbarDrag(root, grip, placement, () => disclosure.finish());
  placement.attached();
  placement.resize(500, 42);
  return {
    panel,
    line,
    bar,
    run,
    root,
    fields,
    input,
    grip,
    button,
    host,
    placement,
    disclosure,
    drag,
    changed,
    write,
    resize: (next: number) => {
      width = next;
      placement.attached();
    },
    dispose: () => {
      drag.dispose();
      disclosure.dispose();
      placement.dispose();
    },
  };
}

/** Deliver pointer gestures through the grip's actual listeners. */
function pointer(grip: HTMLElement, type: string, x: number, y: number): void {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    pointerId: { value: 1 },
    button: { value: 0 },
    clientX: { value: x },
    clientY: { value: y },
  });
  grip.dispatchEvent(event);
}

test('starts in the preferred dock and retains that preference through temporary width fallback', () => {
  const f = fixture({ docked: true, floating: null });
  try {
    expect(f.root.dataset.placement).toBe('docked');
    expect(f.root.parentElement?.parentElement).toBe(f.bar);
    expect(f.write).not.toHaveBeenCalled();
    f.resize(550);
    expect(f.root.dataset.placement).toBe('floating');
    f.resize(1000);
    expect(f.root.dataset.placement).toBe('docked');
    expect(f.write).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('retries the preferred dock after pane resize even when floating legacy placeholders consume space', async () => {
  const originalObserver = window.ResizeObserver;
  let resized: ResizeObserverCallback | null = null;
  let observer: ResizeObserver | null = null;
  /** Deliver settled pane resizing through the native browser geometry boundary. */
  class PaneObserver implements ResizeObserver {
    /** Capture the pane notification without observing disclosure frames. */
    constructor(callback: ResizeObserverCallback) {
      resized = callback;
      observer = this;
    }
    /** Accept the pane selected by the placement adapter. */
    observe(): void {}
    /** Release native geometry ownership at toolbar disposal. */
    disconnect(): void {}
    /** Implement the browser observer surface. */
    unobserve(): void {}
  }
  Object.defineProperty(window, 'ResizeObserver', { configurable: true, value: PaneObserver });
  const f = fixture({ docked: true, floating: null });
  try {
    const empty = document.createElement('div');
    empty.dataset.testid = 'legacy-topbar-container';
    f.bar.append(empty);
    f.bar.getBoundingClientRect = () => {
      const legacyWidth = empty.hasAttribute('data-sc-empty-run-content') ? 0 : 100;
      const slot = f.bar.contains(f.root) ? f.root.getBoundingClientRect().width + 8 : 0;
      return bounds(0, 0, 350 + legacyWidth + slot, 48);
    };
    f.resize(550);
    expect(f.root.dataset.placement).toBe('floating');
    f.line.getBoundingClientRect = () => bounds(0, 0, 650, 48);
    f.panel.getBoundingClientRect = () => bounds(0, 0, 650, 600);
    if (!resized || !observer) throw new Error('Pane resize observation was not mounted');
    const callback: ResizeObserverCallback = resized;
    callback([], observer);
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(f.root.dataset.placement).toBe('docked');
    expect(f.root.style.maxWidth).toBe('176px');
    expect(f.write).not.toHaveBeenCalled();
  } finally {
    f.dispose();
    Object.defineProperty(window, 'ResizeObserver', {
      configurable: true,
      value: originalObserver,
    });
  }
});

test('remembers completed floating movement and docking when the toolbar is recreated', () => {
  const f = fixture();
  let saved: OverrideToolbarPlacementState | undefined;
  try {
    f.placement.shift(140, 100);
    expect(f.write).toHaveBeenLastCalledWith({ docked: false, floating: { x: 144, y: 152 } });
    saved = f.write.mock.calls.at(-1)?.[0];
  } finally {
    f.dispose();
    f.panel.parentElement?.remove();
  }
  if (!saved) throw new Error('Floating placement was not saved');
  const restored = fixture(saved);
  try {
    expect(restored.root.style.left).toBe('144px');
    expect(restored.root.style.top).toBe('152px');
    restored.placement.toggleDock();
    saved = restored.write.mock.calls.at(-1)?.[0];
  } finally {
    restored.dispose();
    restored.panel.parentElement?.remove();
  }
  if (!saved) throw new Error('Docked placement was not saved');
  const docked = fixture(saved);
  try {
    expect(docked.root.dataset.placement).toBe('docked');
    expect(docked.write).not.toHaveBeenCalled();
    docked.placement.reset();
    expect(docked.write).toHaveBeenLastCalledWith({ docked: true, floating: null });
  } finally {
    docked.dispose();
  }
});

test('clamps restored offscreen coordinates without overwriting the saved position', () => {
  const f = fixture({ docked: false, floating: { x: 9000, y: -400 } });
  try {
    expect(f.root.style.left).toBe('496px');
    expect(f.root.style.top).toBe('0px');
    expect(f.write).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('docking reserves the existing Run strip and keeps native controls, values, and focus intact', () => {
  const f = fixture();
  try {
    const initialRun = f.run.getBoundingClientRect();
    f.input.focus();
    f.placement.toggleDock();
    expect(f.root.dataset.placement).toBe('docked');
    expect(f.root.parentElement?.className).toBe('sc-overrides-dock-slot');
    expect(f.bar.contains(f.root)).toBe(true);
    expect(f.run.getBoundingClientRect()).toMatchObject({
      left: initialRun.left,
      top: initialRun.top,
      width: initialRun.width,
      height: initialRun.height,
    });
    expect(f.root.querySelector('input')).toBe(f.input);
    expect(f.input.value).toBe('123456789');
    expect(document.activeElement).toBe(f.input);
    expect(f.root.style.maxWidth).toBe('526px');
    f.placement.attached();
    f.placement.attached();
    expect(f.root.style.maxWidth).toBe('526px');
    expect(f.bar.querySelectorAll('.sc-overrides-dock-slot')).toHaveLength(1);
    f.resize(700);
    expect(f.root.style.maxWidth).toBe('226px');
    expect(f.run.getBoundingClientRect().left).toBe(350);
    f.resize(550);
    expect(f.root.dataset.placement).toBe('floating');
    expect(f.root.parentElement).toBe(f.panel);
    expect(f.root.style.top).toBe('52px');
    expect(f.bar.contains(f.run)).toBe(true);
  } finally {
    f.dispose();
  }
});

test('keeps the top-strip dock independent of the movable native Run panel', () => {
  const f = fixture();
  try {
    const legacy = document.createElement('div');
    legacy.dataset.testid = 'legacy-topbar-container';
    const native = document.createElement('div');
    native.className = 'actionbar';
    const row = document.createElement('div');
    const nativeGrip = document.createElement('span');
    nativeGrip.className = 'drag-handle';
    const overrideGrip = document.createElement('span');
    overrideGrip.className = 'drag-handle';
    f.grip.append(overrideGrip);
    row.append(nativeGrip, f.run);
    native.append(row);
    f.bar.append(legacy, native);
    f.placement.toggleDock();
    expect(f.root.parentElement?.parentElement).toBe(f.bar);
    expect(getComputedStyle(legacy).display).toBe('none');
    expect([...row.children]).toEqual([nativeGrip, f.run]);
    native.style.position = 'absolute';
    native.style.top = '200px';
    f.placement.attached();
    expect(f.root.parentElement?.parentElement).toBe(f.bar);
    expect(native.contains(f.root)).toBe(false);
    expect(legacy.parentElement).toBe(f.bar);
    f.placement.toggleDock();
    f.host.dragging(true);
    expect(f.bar.firstElementChild?.className).toBe('sc-overrides-dock-target');
    f.dispose();
    expect([...row.children]).toEqual([nativeGrip, f.run]);
  } finally {
    f.dispose();
  }
});

test('reclaims native Run space when its panel undocks without resizing the workflow pane', async () => {
  const f = fixture();
  const originalObserver = window.ResizeObserver;
  const callbacks: ResizeObserverCallback[] = [];
  const observed: Element[] = [];
  let layoutObserver: ResizeObserver | null = null;
  /** Observe only native Run sizing, leaving override disclosure geometry to its own owner. */
  class LayoutObserver implements ResizeObserver {
    /** Capture the native browser callback at the external geometry boundary. */
    constructor(callback: ResizeObserverCallback) {
      callbacks.push(callback);
      layoutObserver = this;
    }
    /** Record the surface whose independent size drives the dock budget. */
    observe(target: Element): void {
      observed.push(target);
    }
    /** Release the fake browser observation at host teardown. */
    disconnect(): void {}
    /** Match the native observer contract for an individual surface. */
    unobserve(): void {}
  }
  Object.defineProperty(window, 'ResizeObserver', { configurable: true, value: LayoutObserver });
  try {
    const native = document.createElement('div');
    native.className = 'actionbar';
    native.append(f.run);
    const flow = document.createElement('div');
    flow.append(native);
    f.bar.append(flow);
    let floating = false;
    let targetWidth = 0;
    f.bar.getBoundingClientRect = () => {
      const nativeWidth = floating ? 90 + targetWidth : 350;
      const slot = f.bar.contains(f.root) ? f.root.getBoundingClientRect().width + 8 : 0;
      return bounds(1000 - nativeWidth - slot, 0, nativeWidth + slot, 48);
    };
    f.placement.toggleDock();
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(observed).toEqual([native, flow]);
    expect(callbacks).toHaveLength(1);
    expect(f.root.style.maxWidth).toBe('526px');
    floating = true;
    native.style.position = 'fixed';
    native.classList.add('fixed');
    await Promise.resolve();
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(f.root.style.maxWidth).toBe('786px');
    if (!layoutObserver) throw new Error('Native Run geometry observer was not mounted');
    targetWidth = 200;
    callbacks[0]?.([], layoutObserver);
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(f.root.style.maxWidth).toBe('586px');
    targetWidth = 0;
    callbacks[0]?.([], layoutObserver);
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(f.root.style.maxWidth).toBe('786px');
    floating = false;
    native.style.position = 'static';
    native.classList.remove('fixed');
    await Promise.resolve();
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    expect(f.root.style.maxWidth).toBe('526px');
  } finally {
    f.dispose();
    Object.defineProperty(window, 'ResizeObserver', {
      configurable: true,
      value: originalObserver,
    });
  }
});

test('completed drops dock, cancellation does not, and dragging out retains the same input', () => {
  const f = fixture();
  try {
    pointer(f.grip, 'pointerdown', 14, 62);
    pointer(f.grip, 'pointermove', 500, 250);
    expect(f.root.getBoundingClientRect().left).toBe(490);
    expect(f.root.getBoundingClientRect().top).toBe(240);
    const target = f.bar.querySelector<HTMLElement>('.sc-overrides-dock-target');
    if (!target) throw Error('Missing dock placeholder');
    target.getBoundingClientRect = () => bounds(442, 8, 200, 32);
    pointer(f.grip, 'pointermove', 500, 24);
    expect(target.hasAttribute('data-sc-override-drop-target')).toBe(true);
    expect(f.bar.hasAttribute('data-sc-override-drop-target')).toBe(false);
    expect(f.root.style.top).toBe('14px');
    expect(getComputedStyle(f.root).position).toBe('fixed');
    expect(f.host.canDock()).toBe(true);
    f.placement.attached();
    expect(target.style.width).toBe('200px');
    pointer(f.grip, 'pointercancel', 500, 24);
    expect(f.write).not.toHaveBeenCalled();
    expect(f.root.dataset.placement).toBe('floating');
    expect(f.bar.querySelector('.sc-overrides-dock-target')).toBeNull();
    const before = f.grip.getBoundingClientRect();
    pointer(f.grip, 'pointerdown', before.left + 10, before.top + 10);
    pointer(f.grip, 'pointermove', 500, 250);
    const second = f.bar.querySelector<HTMLElement>('.sc-overrides-dock-target');
    if (!second) throw Error('Missing second placeholder');
    second.getBoundingClientRect = () => bounds(442, 8, 200, 32);
    pointer(f.grip, 'pointermove', 500, 24);
    pointer(f.grip, 'pointerup', 500, 24);
    expect(f.root.dataset.placement).toBe('docked');
    expect(f.write).toHaveBeenLastCalledWith(expect.objectContaining({ docked: true }));
    const docked = f.grip.getBoundingClientRect();
    pointer(f.grip, 'pointerdown', docked.left + 10, docked.top + 10);
    pointer(f.grip, 'pointermove', 500, 250);
    pointer(f.grip, 'pointerup', 500, 250);
    expect(f.root.dataset.placement).toBe('floating');
    expect(f.write).toHaveBeenLastCalledWith(expect.objectContaining({ docked: false }));
    expect(f.root.querySelector('input')).toBe(f.input);
    expect(f.root.style.top).not.toBe('0px');
    expect(f.root.style.transform).toBe('');
    expect(document.querySelector('.sc-overrides-dock-slot')).toBeNull();
  } finally {
    f.dispose();
  }
});

test('drops over native Run controls or outside the dedicated placeholder remain floating', () => {
  const f = fixture();
  try {
    pointer(f.grip, 'pointerdown', 14, 62);
    pointer(f.grip, 'pointermove', 750, 24);
    const target = f.bar.querySelector<HTMLElement>('.sc-overrides-dock-target');
    if (!target) throw Error('Missing dock placeholder');
    target.getBoundingClientRect = () => bounds(442, 8, 200, 32);
    pointer(f.grip, 'pointerup', 750, 24);
    expect(f.root.dataset.placement).toBe('floating');
    expect(f.bar.querySelector('.sc-overrides-dock-target')).toBeNull();
    expect(f.run.getBoundingClientRect().left).toBe(650);
    expect(f.root.querySelector('input')).toBe(f.input);
  } finally {
    f.dispose();
  }
});

test('a grip click creates no target, insufficient room rejects docking, and teardown clears an active target', () => {
  const f = fixture();
  try {
    pointer(f.grip, 'pointerdown', 14, 62);
    pointer(f.grip, 'pointerup', 14, 62);
    expect(f.bar.querySelector('.sc-overrides-dock-target')).toBeNull();
    f.resize(550);
    pointer(f.grip, 'pointerdown', 14, 62);
    pointer(f.grip, 'pointermove', 200, 250);
    expect(f.bar.querySelector('.sc-overrides-dock-target')).toBeNull();
    pointer(f.grip, 'pointerup', 200, 250);
    expect(f.root.dataset.placement).toBe('floating');
    f.resize(1000);
    const grip = f.grip.getBoundingClientRect();
    pointer(f.grip, 'pointerdown', grip.left + 10, grip.top + 10);
    pointer(f.grip, 'pointermove', 300, 350);
    expect(f.bar.querySelector('.sc-overrides-dock-target')).not.toBeNull();
    f.dispose();
    expect(f.bar.querySelector('.sc-overrides-dock-target')).toBeNull();
    expect(f.bar.contains(f.run)).toBe(true);
  } finally {
    f.dispose();
  }
});

test('keyboard docking mirrors both chevrons and retains the collapsed state on undocking', () => {
  const f = fixture();
  try {
    f.button.click();
    expect(f.root.dataset.collapsed).toBe('true');
    expect(getComputedStyle(f.grip).display).toBe('none');
    f.placement.toggleDock();
    expect(f.root.dataset.placement).toBe('docked');
    expect(f.button.firstElementChild?.className).toBe('icon-[lucide--chevron-left]');
    expect(getComputedStyle(f.button).order).toBe('-1');
    expect(getComputedStyle(f.grip).order).not.toBe('-1');
    expect(getComputedStyle(f.fields).order).not.toBe('-1');
    f.button.click();
    expect(getComputedStyle(f.grip).display).toBe('flex');
    expect(f.button.firstElementChild?.className).toBe('icon-[lucide--chevron-right]');
    expect(getComputedStyle(f.button).order).toBe('-1');
    expect([...f.root.children]).toEqual([f.grip, f.grip.nextElementSibling, f.fields, f.button]);
    f.button.click();
    expect(getComputedStyle(f.grip).display).toBe('none');
    f.placement.toggleDock();
    expect(f.root.dataset.placement).toBe('floating');
    expect(f.fields.hidden).toBe(true);
    expect(getComputedStyle(f.grip).display).toBe('none');
    expect(f.button.firstElementChild?.className).toBe('icon-[lucide--chevron-right]');
    f.button.click();
    expect(getComputedStyle(f.grip).display).toBe('flex');
    f.grip.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', cancelable: true }));
    expect(f.root.dataset.placement).toBe('docked');
  } finally {
    f.dispose();
  }
});

test('replaced native chrome accepts the same docked toolbar and teardown removes only owned chrome', () => {
  const f = fixture();
  try {
    f.placement.toggleDock();
    const replacement = f.bar.cloneNode(false);
    if (!(replacement instanceof HTMLElement)) throw Error('Missing header');
    replacement.getBoundingClientRect = () => {
      const slot = replacement.contains(f.root) ? f.root.getBoundingClientRect().width + 8 : 0;
      return bounds(650 - slot, 0, 350 + slot, 48);
    };
    f.bar.replaceWith(replacement);
    replacement.append(f.run);
    f.placement.attached();
    expect(replacement.contains(f.root)).toBe(true);
    expect(f.root.querySelector('input')).toBe(f.input);
    f.placement.dispose();
    expect(replacement.contains(f.run)).toBe(true);
    expect(replacement.querySelector('.sc-overrides-dock-slot')).toBeNull();
  } finally {
    f.dispose();
  }
});

test('replacement Run chrome cannot borrow space from a detached dock slot', () => {
  const f = fixture();
  try {
    f.placement.toggleDock();
    const replacement = document.createElement('div');
    replacement.className = 'actionbar-container';
    replacement.getBoundingClientRect = () => bounds(50, 0, 950, 48);
    f.bar.replaceWith(replacement);
    replacement.append(f.run);
    f.placement.attached();
    expect(f.root.dataset.placement).toBe('floating');
    expect(f.root.parentElement).toBe(f.panel);
    expect(replacement.contains(f.run)).toBe(true);
    expect(replacement.querySelector('.sc-overrides-dock-slot')).toBeNull();
  } finally {
    f.dispose();
  }
});

test('floating and docked grips follow the native Run handle font instead of toolbar theme typography', () => {
  const f = fixture();
  try {
    const native = document.createElement('div');
    native.className = 'actionbar';
    const grip = document.createElement('span');
    grip.className = 'drag-handle';
    grip.style.fontFamily = 'Comfy Handle Font';
    native.append(grip);
    f.bar.append(native);
    f.placement.attached();
    expect(f.root.style.getPropertyValue('--sc-native-grip-font-family')).toBe('Comfy Handle Font');
    f.placement.toggleDock();
    expect(f.root.style.getPropertyValue('--sc-native-grip-font-family')).toBe('Comfy Handle Font');
    grip.style.fontFamily = 'Updated Comfy Font';
    f.placement.attached();
    expect(f.root.style.getPropertyValue('--sc-native-grip-font-family')).toBe(
      'Updated Comfy Font',
    );
  } finally {
    f.dispose();
  }
});
