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
/** Prove when What's new opens and what it shows for SugarCubes and a host. */
import { expect, jest, test } from '@jest/globals';
import {
  WhatsNewPresenter,
  type SeenReleaseStore,
  type WhatsNewSection,
  type WhatsNewView,
  type WhatsNewViewActions,
} from '../../../frontend/comfyui/ui/core/release_notes/WhatsNewPresenter.js';
import {
  compareReleaseVersions,
  type ReleaseNotesIdentity,
  type ReleaseNotesOutcome,
  type ReleaseNotesSource,
} from '../../../frontend/comfyui/ui/core/release_notes/ReleaseNotesSource.js';
import {
  OwnReleaseNotesSource,
  decodeReleaseNotes,
} from '../../../frontend/comfyui/ui/core/release_notes/OwnReleaseNotesSource.js';
import { HostReleaseNotesLink } from '../../../frontend/comfyui/ui/core/release_notes/HostReleaseNotesLink.js';

/** Let settled reads reach the presenter. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

/** A view that records what it was asked to show. */
function view() {
  const shown: (readonly WhatsNewSection[])[] = [];
  let actions: WhatsNewViewActions | null = null;
  const value: WhatsNewView = {
    show(sections, received) {
      shown.push(sections);
      actions = received;
    },
  };
  return {
    value,
    shown,
    last: () => shown[shown.length - 1] ?? [],
    close: () => actions?.closed(),
    retry: (key: string) => actions?.retry(key),
  };
}

function store(initial: Record<string, string> = {}) {
  const values = { ...initial };
  const value: SeenReleaseStore = {
    read: (product) => values[product] ?? null,
    record: (product, version) => {
      values[product] = version;
    },
  };
  return { value, values };
}

/** A scripted notes source whose reads the test answers. */
function source(identity: ReleaseNotesIdentity | null) {
  let current = identity;
  const listeners = new Set<() => void>();
  const answers: ((outcome: ReleaseNotesOutcome) => void)[] = [];
  const value = {
    read: () => current,
    subscribe: (changed: () => void) => {
      listeners.add(changed);
      return () => listeners.delete(changed);
    },
    load: jest.fn(
      (_since: string) =>
        new Promise<ReleaseNotesOutcome>((resolve) => {
          answers.push(resolve);
        }),
    ),
  } satisfies ReleaseNotesSource;
  return {
    value,
    identify(next: ReleaseNotesIdentity) {
      current = next;
      for (const changed of [...listeners]) changed();
    },
    answer(outcome: ReleaseNotesOutcome) {
      answers.shift()?.(outcome);
    },
  };
}

const notes = (version: string): ReleaseNotesOutcome => ({
  kind: 'notes',
  releases: [{ version, notes: `## ${version}\n` }],
  omitted: 0,
});

test('release tokens order numerically and a release follows its prereleases', () => {
  expect(compareReleaseVersions('0.10.0', '0.9.9')).toBeGreaterThan(0);
  expect(compareReleaseVersions('1.0.0', '1.0.0-beta.1')).toBeGreaterThan(0);
  expect(compareReleaseVersions('1.0.0', '1.0.0')).toBe(0);
  expect(compareReleaseVersions('dev', '1.0.0')).toBeNull();
});

test('a first run records the version silently and a later update opens the view once', async () => {
  const seen = store();
  const first = view();
  const sugar = source({ product: 'SugarCubes', currentVersion: '0.15.3' });
  new WhatsNewPresenter(first.value, seen.value, jest.fn()).attach(sugar.value);
  expect(first.shown).toEqual([]);
  expect(seen.values).toEqual({ SugarCubes: '0.15.3' });

  const second = view();
  const updated = source({ product: 'SugarCubes', currentVersion: '0.16.1' });
  new WhatsNewPresenter(second.value, seen.value, jest.fn()).attach(updated.value);
  expect(second.last()).toEqual([
    expect.objectContaining({
      product: 'SugarCubes',
      currentVersion: '0.16.1',
      updatedFrom: '0.15.3',
      status: { kind: 'loading' },
    }),
  ]);
  expect(updated.value.load).toHaveBeenCalledWith('0.15.3');
  expect(seen.values).toEqual({ SugarCubes: '0.16.1' });
  updated.answer(notes('0.16.1'));
  await settle();
  expect(second.last()[0]?.status).toEqual(notes('0.16.1'));

  const third = view();
  new WhatsNewPresenter(third.value, seen.value, jest.fn()).attach(
    source({ product: 'SugarCubes', currentVersion: '0.16.1' }).value,
  );
  expect(third.shown).toEqual([]);
});

test('products that updated in the same start share one view in labelled sections', () => {
  const shown = view();
  const presenter = new WhatsNewPresenter(
    shown.value,
    store({ SugarCubes: '0.15.3', SugarSubstitute: '0.3.0' }).value,
    jest.fn(),
  );
  presenter.attach(source({ product: 'SugarCubes', currentVersion: '0.16.0' }).value);
  const host = source(null);
  presenter.attach(host.value);
  host.identify({ product: 'SugarSubstitute', currentVersion: '0.5.0' });
  expect(
    shown.last().map(({ product, updatedFrom, currentVersion }) => ({
      product,
      updatedFrom,
      currentVersion,
    })),
  ).toEqual([
    { product: 'SugarCubes', updatedFrom: '0.15.3', currentVersion: '0.16.0' },
    { product: 'SugarSubstitute', updatedFrom: '0.3.0', currentVersion: '0.5.0' },
  ]);
  expect(host.value.load).toHaveBeenCalledWith('0.3.0');
});

test('asking to open shows every product, including ones that did not update', () => {
  const shown = view();
  const presenter = new WhatsNewPresenter(
    shown.value,
    store({ SugarCubes: '0.16.0' }).value,
    jest.fn(),
  );
  const sugar = source({ product: 'SugarCubes', currentVersion: '0.16.0' });
  presenter.attach(sugar.value);
  expect(shown.shown).toEqual([]);
  presenter.open();
  expect(shown.last()).toEqual([
    expect.objectContaining({ product: 'SugarCubes', updatedFrom: null }),
  ]);
  expect(sugar.value.load).toHaveBeenCalledWith('0.16.0');
});

test('the host asking to show what is new opens the view', () => {
  const sent: unknown[] = [];
  const listeners = new Set<(value: unknown) => void>();
  const session = 'panel-0123456789abcdef';
  const link = new HostReleaseNotesLink(
    {
      session,
      send: (message) => sent.push(message),
      subscribe: (receive) => {
        listeners.add(receive);
        return () => listeners.delete(receive);
      },
    },
    jest.fn(),
  );
  const shown = view();
  const presenter = new WhatsNewPresenter(
    shown.value,
    store({ SugarSubstitute: '0.5.0' }).value,
    jest.fn(),
  );
  presenter.attachHost(link);
  link.start();
  const envelope = { protocol: 'sugarcubes.release-notes', version: 1, session } as const;
  const deliver = (value: unknown) => {
    for (const listener of [...listeners]) listener(value);
  };
  deliver({
    ...envelope,
    type: 'state',
    revision: 1,
    product: 'SugarSubstitute',
    currentVersion: '0.5.0',
  });
  expect(shown.shown).toEqual([]);
  deliver({ ...envelope, type: 'open' });
  expect(shown.last()).toEqual([expect.objectContaining({ product: 'SugarSubstitute' })]);
  expect(sent).toContainEqual({ ...envelope, type: 'load', request: 'load-1', since: '0.5.0' });
});

test('retry reads a failed section again and reopening retries a failure', async () => {
  const shown = view();
  const presenter = new WhatsNewPresenter(
    shown.value,
    store({ SugarCubes: '0.1.0' }).value,
    jest.fn(),
  );
  const sugar = source({ product: 'SugarCubes', currentVersion: '0.2.0' });
  presenter.attach(sugar.value);
  sugar.answer({ kind: 'failed', reason: 'unavailable' });
  await settle();
  const failed = shown.last()[0];
  expect(failed?.status).toEqual({ kind: 'failed', reason: 'unavailable' });
  shown.retry(failed?.key ?? '');
  expect(shown.last()[0]?.status).toEqual({ kind: 'loading' });
  sugar.answer({ kind: 'failed', reason: 'unavailable' });
  await settle();
  shown.close();
  presenter.open();
  expect(sugar.value.load).toHaveBeenCalledTimes(3);
  sugar.answer(notes('0.2.0'));
  await settle();
  expect(shown.last()[0]?.status).toEqual(notes('0.2.0'));
  shown.close();
  presenter.open();
  expect(sugar.value.load).toHaveBeenCalledTimes(3);
});

test('SugarCubes reads its own notes from its backend and validates the reply', async () => {
  const get = jest.fn(async (path: string) =>
    path === '/sugarcubes/release-notes'
      ? { status: 200, body: { schema: 1, product: 'SugarCubes', currentVersion: '0.16.0' } }
      : {
          status: 200,
          body: { schema: 1, releases: [{ version: '0.16.0', notes: 'x' }], omitted: 0 },
        },
  );
  const own = new OwnReleaseNotesSource({ get }, jest.fn());
  await own.start();
  expect(own.read()).toEqual({ product: 'SugarCubes', currentVersion: '0.16.0' });
  await expect(own.load('0.15.3')).resolves.toEqual({
    kind: 'notes',
    releases: [{ version: '0.16.0', notes: 'x' }],
    omitted: 0,
  });
  expect(get).toHaveBeenLastCalledWith('/sugarcubes/release-notes/notes?since=0.15.3');
  expect(decodeReleaseNotes({ schema: 1, failure: 'not-published' })).toEqual({
    kind: 'failed',
    reason: 'not-published',
  });
  for (const body of [null, { schema: 1, failure: 'unavailable' }, { schema: 1, releases: [] }])
    expect(decodeReleaseNotes(body)).toEqual({ kind: 'failed', reason: 'unavailable' });
  const offline = new OwnReleaseNotesSource(
    { get: () => Promise.reject(new Error('offline')) },
    jest.fn(),
  );
  await expect(offline.load('0.15.3')).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
});
