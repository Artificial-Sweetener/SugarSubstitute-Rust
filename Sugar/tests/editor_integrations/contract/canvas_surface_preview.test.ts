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
/** Exercise preview custody and presentation messages through the shared authenticated session. */
import type {
  CanvasSurfaceCommand,
  CanvasSurfacePresentationChange,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';
import type {
  CanvasObservationStatus,
  CanvasChangeSubscription,
} from '../../../integrations/contracts/CanvasChangeObservation.js';
import { selectedPngCapabilities } from '../support/canvas_surface.js';
import {
  fixture,
  change,
  session,
  address,
  preview,
  viewport,
  begin,
} from '../support/canvas_preview.js';

test('does not replay an old notification into a subscription reopened by its own listener', async () => {
  const f = await fixture();
  const seen: number[] = [],
    reopened: CanvasChangeSubscription[] = [];
  try {
    const first = f.host.presentation!.subscribe((value) => {
      seen.push(value.sequence);
      first.stop();
      reopened.push(f.host.presentation!.subscribe((next) => seen.push(next.sequence)));
    });
    await first.ready;
    for (const listener of [...f.listeners]) listener(change(1));
    expect(reopened).toHaveLength(1);
    await reopened[0]!.ready;
    expect(seen).toEqual([1]);
    for (const listener of [...f.listeners]) listener(change(2));
    expect(seen).toEqual([1, 2]);
    reopened[0]!.stop();
  } finally {
    f.close();
  }
});

test('an asynchronous presentation observation cannot complete the outstanding ordinary camera request', async () => {
  let finish: (value: CanvasSurfaceResult) => void = () => {};
  const f = await fixture({
    execute: () =>
      new Promise<CanvasSurfaceResult>((resolve) => {
        finish = resolve;
      }),
  });
  const seen: CanvasSurfacePresentationChange[] = [];
  try {
    const subscription = f.host.presentation!.subscribe((value) => seen.push(value));
    await subscription.ready;
    let settled = false;
    const request = f.host.execute({
      type: 'surface.viewport.request',
      address,
      revision: 4,
      command: { type: 'native' },
    });
    void request.then(() => {
      settled = true;
    });
    for (const listener of [...f.listeners]) listener(change());
    await Promise.resolve();
    expect(seen).toEqual([change()]);
    expect(settled).toBe(false);
    finish({ status: 'applied', message: '', viewport });
    expect((await request).status).toBe('applied');
    subscription.stop();
  } finally {
    f.close();
  }
});

test('releases a failed native watch and retains the original failure without admitting preview data', async () => {
  let rejectReady: (cause: Error) => void = () => {};
  const ready = new Promise<CanvasObservationStatus>((_resolve, reject) => {
    rejectReady = reject;
  });
  const f = await fixture({ ready });
  try {
    const subscription = f.host.presentation!.subscribe(() => {});
    const failure = new Error('Native observation failed');
    rejectReady(failure);
    expect(await subscription.ready).toBe('unavailable');
    expect(f.stops()).toBe(1);
    expect(f.errors).toContain(failure);
    expect((await f.host.execute(begin)).status).toBe('unavailable');
    subscription.stop();
  } finally {
    f.close();
  }
});

test('rejects individually bounded metadata whose serialized request exceeds native admission', async () => {
  const f = await fixture();
  try {
    const subscription = f.host.presentation!.subscribe(() => {});
    await subscription.ready;
    const oversized: CanvasSurfaceCommand = {
      ...begin,
      preview: { ...preview, sourceId: 'source:' + '😀'.repeat(4089) },
    };
    await expect(f.host.execute(oversized)).rejects.toThrow('transport bounds');
    expect(f.commands).toEqual([]);
    subscription.stop();
  } finally {
    f.close();
  }
});

test('keeps older public capability declarations unchanged and refuses unobserved preview bytes', async () => {
  const old = await fixture({ capabilities: selectedPngCapabilities });
  try {
    expect(old.host.capabilities).toEqual(selectedPngCapabilities);
    expect(old.host.presentation).toBeUndefined();
    expect((await old.host.execute(begin)).status).toBe('unavailable');
    expect(old.sent.map(({ type }) => type)).toEqual([
      'context.request',
      'surface.capabilities.request',
    ]);
  } finally {
    old.close();
  }
  const limited = await fixture({ observation: false });
  try {
    const subscription = limited.host.presentation!.subscribe(() => {});
    expect(await subscription.ready).toBe('unsupported');
    expect((await limited.host.execute(begin)).status).toBe('unavailable');
    expect(limited.commands).toEqual([]);
    subscription.stop();
  } finally {
    limited.close();
  }
});

test('commits an empty attachment and releases every custody request before first-preview acceptance', async () => {
  const f = await fixture();
  const seen: CanvasSurfacePresentationChange[] = [];
  try {
    const subscription = f.host.presentation!.subscribe((value) => seen.push(value));
    expect(await subscription.ready).toBe('observing');
    for (const command of [
      { type: 'surface.prepare', address, revision: 1 },
      { type: 'surface.commit', address, revision: 2 },
      begin,
      {
        type: 'surface.preview.chunk',
        address,
        revision: 4,
        token: preview.token,
        leaseRevision: 3,
        offset: 0,
        data: 'AQID',
      },
      { type: 'surface.preview.end', address, revision: 5, token: preview.token, leaseRevision: 3 },
    ] satisfies CanvasSurfaceCommand[])
      expect((await f.host.execute(command)).status).toBe(
        command.type === 'surface.prepare' ? 'ready' : 'applied',
      );
    expect(seen).toEqual([]);
    expect(f.commands.some(({ type }) => type === 'surface.select')).toBe(false);
    const camera = await f.host.execute({
      type: 'surface.viewport.request',
      address,
      revision: 6,
      command: { type: 'native' },
    });
    expect(camera.status).toBe('applied');
    expect(f.sent.map(({ requestId }) => requestId)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    for (const listener of f.listeners) listener(change());
    expect(seen).toEqual([change()]);
    expect(Object.isFrozen(seen[0]?.address)).toBe(true);
    expect(Object.isFrozen(seen[0]?.displayed)).toBe(true);
    subscription.stop();
  } finally {
    f.close();
  }
});

test('keeps lease and final selection identities stable across later camera revisions and successive previews', async () => {
  const f = await fixture();
  const seen: CanvasSurfacePresentationChange[] = [];
  try {
    const subscription = f.host.presentation!.subscribe((value) => seen.push(value));
    await subscription.ready;
    for (const listener of f.listeners) listener(change());
    const next = {
      ...change(2),
      token: 'preview:2',
      leaseRevision: 7,
      displayed: { type: 'preview', token: 'preview:2', leaseRevision: 7, viewport },
    } as const;
    for (const listener of f.listeners) listener(next);
    const restored = {
      ...change(3),
      token: 'preview:2',
      leaseRevision: 7,
      status: 'cancelled',
      displayed: { type: 'final', selectionRevision: 2, viewport: { ...viewport, mode: 'custom' } },
    } as const;
    for (const listener of f.listeners) listener(restored);
    expect(seen).toEqual([change(), next, restored]);
    expect(seen.at(-1)?.displayed).toMatchObject({ type: 'final', selectionRevision: 2 });
    subscription.stop();
  } finally {
    f.close();
  }
});

test('stops native observation, fences duplicate events across watcher reopen and ignores late notifications', async () => {
  const f = await fixture();
  const seen: number[] = [];
  try {
    const first = f.host.presentation!.subscribe((value) => seen.push(value.sequence));
    await first.ready;
    for (const listener of f.listeners) listener(change(1));
    first.stop();
    expect(f.stops()).toBe(1);
    expect(
      f.client.receive({
        protocol: 'sugarcubes.canvas',
        version: 3,
        session,
        requestId: 0,
        type: 'surface.presentation.changed',
        schema: 1,
        change: change(2),
      }),
    ).toBe(false);
    const second = f.host.presentation!.subscribe((value) => seen.push(value.sequence));
    await second.ready;
    for (const listener of f.listeners) {
      listener(change(1));
      listener(change(2));
      listener(change(3));
    }
    expect(seen).toEqual([1, 3]);
    expect(f.starts()).toBe(2);
    expect((await f.host.execute(begin)).status).toBe('applied');
    second.stop();
    expect((await f.host.execute({ ...begin, revision: 4 })).status).toBe('unavailable');
  } finally {
    f.close();
  }
});

test('a late start acknowledgement cannot admit preview data after its subscription stopped', async () => {
  let accept: (status: CanvasObservationStatus) => void = () => {};
  const ready = new Promise<CanvasObservationStatus>((resolve) => {
    accept = resolve;
  });
  const f = await fixture({ ready });
  try {
    const subscription = f.host.presentation!.subscribe(() => {});
    subscription.stop();
    accept('observing');
    expect(await subscription.ready).toBe('unavailable');
    expect((await f.host.execute(begin)).status).toBe('unavailable');
    expect(f.commands).toEqual([]);
    expect(f.stops()).toBe(1);
  } finally {
    f.close();
  }
});

test('rejects noncanonical, foreign-session and false native acceptance observations without altering pending state', async () => {
  const f = await fixture();
  const seen: CanvasSurfacePresentationChange[] = [];
  try {
    const subscription = f.host.presentation!.subscribe((value) => seen.push(value));
    await subscription.ready;
    const event = {
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 0,
      type: 'surface.presentation.changed',
      schema: 1,
      change: change(),
    } as const;
    expect(f.client.receive({ ...event, session: 'foreign-contract-session' })).toBe(false);
    expect(
      f.client.receive({
        ...event,
        change: { ...change(), displayed: { type: 'final', selectionRevision: 2, viewport } },
      }),
    ).toBe(false);
    expect(
      f.client.receive({
        ...event,
        change: { ...change(), displayed: { ...change().displayed, token: 'preview:other' } },
      }),
    ).toBe(false);
    expect(seen).toEqual([]);
    expect(f.client.receive(event)).toBe(true);
    expect(seen).toEqual([change()]);
    subscription.stop();
  } finally {
    f.close();
  }
});
