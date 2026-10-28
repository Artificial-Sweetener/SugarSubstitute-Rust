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
/** Exercise surface handoff against the real attachment registry. */

import { jest } from '@jest/globals';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import type { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import {
  CanvasPrimarySurface,
  type PrimaryCanvasAttachment,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

function fixture() {
  const catalog = canvasCatalogFixture();
  let key = 0;
  const views = new CanvasViewRegistry(() => String(++key));
  const view = views.create(catalog.sessionId).viewId;
  let active: typeof view | null = view;
  const coordinator = {
    active: () => active,
    catalog: () => catalog,
    subscribe: () => () => undefined,
  } as unknown as CanvasSessionCoordinator;
  const element = document.createElement('div');
  document.body.append(element);
  let renderer = 'browser';
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  let resolveMount!: () => void;
  const ready = new Promise<void>((done, failed) => {
    resolve = done;
    reject = failed;
  });
  void ready.catch(() => {});
  const attachments: {
    address: Parameters<CanvasViewRegistry['accepts']>[0];
    element: HTMLElement;
    accepts: () => boolean;
    acceptsPreparation: () => boolean;
    dispose: ReturnType<typeof jest.fn<() => void | Promise<void>>>;
  }[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const primary = new CanvasPrimarySurface({
    document,
    window: windowId('primary'),
    views,
    coordinator,
    element: () => element,
    renderer: () => renderer,
    report,
    mount: (root, address, _catalog, accepts, acceptsPreparation) => {
      const dispose = jest.fn<() => void | Promise<void>>();
      attachments.push({ address, element: root, accepts, acceptsPreparation, dispose });
      const attachment: PrimaryCanvasAttachment & { ready?: Promise<void> } = {
        ...(renderer === 'native' ? { ready } : {}),
        diagnostics: () => ({
          displayed: 0,
          pendingDecodes: 0,
          queuedInputs: 0,
          previewDrops: 0,
          estimatedBytes: 0,
          unknownLoads: 0,
          activeDeadlines: 0,
          renderFrames: 0,
        }),
        refresh: () => {},
        wants: () => false,
        receive: () => {},
        dispose,
      };
      if (renderer === 'deferred')
        return new Promise<PrimaryCanvasAttachment>((done) => {
          resolveMount = () => done(attachment);
        });
      return attachment;
    },
  });
  primary.setup();
  return {
    primary,
    views,
    attachments,
    report,
    resolve,
    reject,
    resolveMount: () => resolveMount(),
    deactivate: () => {
      active = null;
      primary.reconcile();
    },
    renderer: (value: string) => {
      renderer = value;
      primary.reconcile();
    },
    dispose: () => {
      primary.dispose();
      element.remove();
    },
  };
}

test('keeps the current surface until a deferred presenter is constructed', async () => {
  const f = fixture();
  try {
    const old = f.attachments[0]!;
    f.renderer('deferred');
    const next = f.attachments[1]!;
    expect(f.primary.diagnostics().address).toEqual(old.address);
    expect(old.dispose).not.toHaveBeenCalled();
    f.resolveMount();
    await Promise.resolve();
    expect(f.primary.diagnostics().address).toEqual(next.address);
    expect(old.dispose).toHaveBeenCalledTimes(1);
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('retires a presenter that finishes loading after its attachment was superseded', async () => {
  const f = fixture();
  try {
    const old = f.attachments[0]!;
    f.renderer('deferred');
    const next = f.attachments[1]!;
    f.renderer('browser');
    expect(f.primary.diagnostics().address).toEqual(old.address);
    f.resolveMount();
    await Promise.resolve();
    expect(next.dispose).toHaveBeenCalledTimes(1);
    expect(f.primary.diagnostics().address).toEqual(old.address);
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('keeps the current surface until a replacement is ready and commits it once', async () => {
  const f = fixture();
  try {
    const old = f.attachments[0]!;
    f.renderer('native');
    const next = f.attachments[1]!;
    expect(f.primary.diagnostics().address).toEqual(old.address);
    expect(f.views.accepts(old.address)).toBe(true);
    expect(f.views.accepts(next.address)).toBe(false);
    expect(old.accepts()).toBe(true);
    expect(next.accepts()).toBe(false);
    expect(old.acceptsPreparation()).toBe(false);
    expect(next.acceptsPreparation()).toBe(true);
    expect(old.dispose).not.toHaveBeenCalled();
    expect(next.element.style.visibility).toBe('hidden');

    f.resolve();
    await Promise.resolve();
    expect(f.primary.diagnostics().address).toEqual(next.address);
    expect(f.views.accepts(old.address)).toBe(false);
    expect(f.views.accepts(next.address)).toBe(true);
    expect(old.accepts()).toBe(false);
    expect(next.accepts()).toBe(true);
    expect(next.acceptsPreparation()).toBe(false);
    expect(old.dispose).toHaveBeenCalledTimes(1);
    expect(next.element.style.visibility).toBe('');
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('revokes a failed or superseded pending surface without disturbing its replacement', async () => {
  const f = fixture();
  try {
    const old = f.attachments[0]!;
    f.renderer('native');
    const pending = f.attachments[1]!;
    f.renderer('browser');
    expect(pending.dispose).toHaveBeenCalledTimes(1);
    expect(pending.acceptsPreparation()).toBe(false);
    expect(f.primary.diagnostics().address).toEqual(old.address);
    expect(f.views.accepts(old.address)).toBe(true);
    expect(old.accepts()).toBe(true);
    f.reject(new Error('retired native surface'));
    await Promise.resolve();
    expect(f.primary.diagnostics().address).toEqual(old.address);
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('retiring the active workflow cancels both surfaces and ignores late readiness', async () => {
  const f = fixture();
  try {
    const old = f.attachments[0]!;
    f.renderer('native');
    const pending = f.attachments[1]!;
    f.deactivate();
    expect(old.dispose).toHaveBeenCalledTimes(1);
    expect(pending.dispose).toHaveBeenCalledTimes(1);
    expect(f.views.accepts(old.address)).toBe(false);
    expect(f.views.accepts(pending.address)).toBe(false);
    expect(pending.acceptsPreparation()).toBe(false);
    f.resolve();
    await Promise.resolve();
    expect(f.primary.diagnostics().address).toBeNull();
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('retains the current surface when replacement readiness fails', async () => {
  const f = fixture();
  try {
    const old = f.attachments[0]!;
    f.renderer('native');
    const pending = f.attachments[1]!;
    const failure = new Error('native surface unavailable');
    pending.dispose.mockReturnValue(Promise.resolve());
    f.reject(failure);
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.attachments).toHaveLength(2);
    expect(f.primary.diagnostics().address).toEqual(old.address);
    expect(f.views.accepts(old.address)).toBe(true);
    expect(f.views.accepts(pending.address)).toBe(false);
    expect(pending.acceptsPreparation()).toBe(false);
    expect(pending.dispose).toHaveBeenCalledTimes(1);
    expect(f.report).toHaveBeenCalledWith(failure);
  } finally {
    f.dispose();
  }
});

test('waits for canceled candidate release before mounting its replacement', async () => {
  const f = fixture();
  try {
    f.renderer('native');
    const candidate = f.attachments[1]!;
    let released!: () => void;
    candidate.dispose.mockReturnValue(
      new Promise<void>((resolve) => {
        released = resolve;
      }),
    );
    f.renderer('next');
    expect(candidate.dispose).toHaveBeenCalledTimes(1);
    expect(f.attachments).toHaveLength(2);
    for (let index = 0; index < 100; index++) f.renderer(`replacement-${String(index)}`);
    expect(f.attachments).toHaveLength(2);
    expect(document.querySelectorAll('div[style*="pointer-events: none"]')).toHaveLength(1);
    released();
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.attachments).toHaveLength(3);
    expect(f.primary.diagnostics().address).toEqual(f.attachments[2]!.address);
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('does not mount a canceled replacement after its release barrier settles', async () => {
  const f = fixture();
  try {
    f.renderer('native');
    let released!: () => void;
    f.attachments[1]!.dispose.mockReturnValue(
      new Promise<void>((resolve) => {
        released = resolve;
      }),
    );
    f.renderer('next');
    f.deactivate();
    released();
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.attachments).toHaveLength(2);
    expect(f.primary.diagnostics().address).toBeNull();
    expect(f.report).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});

test('reports failed release and does not mount over uncertain native custody', async () => {
  const f = fixture();
  try {
    f.renderer('native');
    let failed!: (error: Error) => void;
    f.attachments[1]!.dispose.mockReturnValue(
      new Promise<void>((_resolve, reject) => {
        failed = reject;
      }),
    );
    f.renderer('next');
    const error = new Error('native rollback failed');
    failed(error);
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.attachments).toHaveLength(2);
    expect(f.primary.diagnostics().address).toEqual(f.attachments[0]!.address);
    expect(f.report).toHaveBeenCalledWith(error);
    f.renderer('another');
    for (let turn = 0; turn < 8; turn++) await Promise.resolve();
    expect(f.attachments).toHaveLength(2);
  } finally {
    f.dispose();
  }
});
