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
/** Prove the page joins a desktop host's memory budget and stores its limit in Comfy settings. */
import { readFileSync } from 'node:fs';
import { expect, jest, test } from '@jest/globals';
import { isMemoryClientMessage } from '../../../integrations/shared/MemoryValidation.generated.js';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';
import { BudgetedMemoryCache } from '../../../frontend/comfyui/ui/core/memory/BudgetedMemoryCache.js';
import {
  MemoryHostLink,
  type MemoryPreferenceSource,
} from '../../../frontend/comfyui/ui/core/memory/MemoryHostLink.js';
import { MemoryBudgetSettings } from '../../../frontend/comfyui/ui/settings/MemoryBudgetSettings.js';
import {
  type MemoryClientMessage,
  type MemoryPreference,
  preferenceMessage,
  readAllowance,
  usageMessage,
} from '../../../frontend/comfyui/ui/core/memory/MemoryHostProtocol.js';

const MIB = 1024 * 1024;
const session = 'panel-0123456789abcdef';

function fixedPreferences(preference: MemoryPreference) {
  const listeners = new Set<(value: MemoryPreference) => void>();
  const source: MemoryPreferenceSource = {
    read: () => preference,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  return {
    source,
    change(value: MemoryPreference) {
      preference = value;
      for (const listener of listeners) listener(value);
    },
  };
}

function transport() {
  const sent: MemoryClientMessage[] = [];
  let receive: ((value: unknown) => void) | null = null;
  return {
    sent,
    deliver: (value: unknown) => receive?.(value),
    link: {
      session,
      send: (message: MemoryClientMessage) => sent.push(message),
      subscribe: (listener: (value: unknown) => void) => {
        receive = listener;
        return () => (receive = null);
      },
    },
  };
}

function allowance(generation: number, allowanceBytes: number, level = 'normal') {
  return {
    protocol: 'sugarcubes.memory',
    version: 1,
    session,
    type: 'allowance',
    generation,
    level,
    allowanceBytes,
    ceilingBytes: 16 * 1024 * MIB,
    totalBytes: 64 * 1024 * MIB,
    preference: { mode: 'share', percent: 25 },
  };
}

test('announces the preference and usage, then applies only newer host allowances', async () => {
  const registry = new MemoryBudgetRegistry(64 * MIB);
  const store = new BudgetedMemoryCache<string, string>({ id: 'images', registry });
  for (let index = 0; index < 4; index += 1)
    store.set(`image-${index}`, 'x', { tier: 2, bytes: 8 * MIB });
  const host = transport();
  const preferences = fixedPreferences({ mode: 'share', percent: 25 });
  const link = new MemoryHostLink(host.link, registry, preferences.source, jest.fn());
  link.start();
  expect(host.sent.map((message) => message.type)).toEqual(['preference', 'usage']);
  expect(host.sent[1]).toMatchObject({ usedBytes: 32 * MIB, demandBytes: 224 * MIB });
  host.deliver(allowance(2, 16 * MIB));
  expect(registry.usage().usedBytes).toBeLessThanOrEqual(16 * MIB);
  // An older or foreign allowance is ignored.
  host.deliver(allowance(1, 0, 'critical'));
  host.deliver({ ...allowance(3, 0), session: 'panel-ffffffffffffffff' });
  expect(registry.usage().usedBytes).toBe(16 * MIB);
  await Promise.resolve();
  expect(host.sent.at(-1)).toMatchObject({ type: 'usage', usedBytes: 16 * MIB });
  preferences.change({ mode: 'fixed', bytes: 8 * 1024 * MIB });
  expect(host.sent.at(-1)).toMatchObject({
    type: 'preference',
    preference: { mode: 'fixed', bytes: 8 * 1024 * MIB },
  });
  link.dispose();
  const count = host.sent.length;
  preferences.change({ mode: 'share', percent: 50 });
  expect(host.sent).toHaveLength(count);
});

test('a host that rejects the protocol stops the link after one report', () => {
  const registry = new MemoryBudgetRegistry();
  const report = jest.fn();
  const send = jest.fn(() => {
    throw new TypeError('unsupported');
  });
  const link = new MemoryHostLink(
    { session, send, subscribe: () => () => undefined },
    registry,
    fixedPreferences({ mode: 'share', percent: 25 }).source,
    report,
  );
  link.start();
  expect(send).toHaveBeenCalledTimes(1);
  expect(report).toHaveBeenCalledTimes(1);
});

test('usage reports bound consumer identities and allowances reject malformed values', () => {
  const message = usageMessage(session, {
    usedBytes: 10.4,
    consumers: [
      { id: 'canvas.images', bytes: 6 },
      { id: 'Invalid Id', bytes: 4 },
      ...Array.from({ length: 20 }, (_, index) => ({ id: `c${index}`, bytes: 1 })),
    ],
  });
  expect(message.usedBytes).toBe(10);
  expect(message.consumers).toHaveLength(16);
  expect(message.consumers[0]).toEqual({ id: 'canvas.images', bytes: 6 });
  expect(readAllowance(allowance(1, 5), session)).toMatchObject({ allowanceBytes: 5 });
  expect(readAllowance({ ...allowance(1, -1) }, session)).toBeNull();
  expect(readAllowance({ ...allowance(1, 5), level: 'panic' }, session)).toBeNull();
  expect(readAllowance({ ...allowance(1, 5), version: 3 }, session)).toBeNull();
});

test('memory settings register in Comfy and select a clamped share or fixed limit', () => {
  const values = new Map<string, unknown>();
  const definitions: { id: string; onChange(value: unknown): void }[] = [];
  const app = {
    ui: {
      settings: {
        addSetting: (definition: { id: string; onChange(value: unknown): void }) =>
          definitions.push(definition),
        getSettingValue: (id: string) => values.get(id),
      },
    },
  };
  const settings = new MemoryBudgetSettings(app);
  expect(definitions.map((definition) => definition.id)).toEqual([
    'SugarCubes.Memory.Mode',
    'SugarCubes.Memory.SharePercent',
    'SugarCubes.Memory.FixedGiB',
  ]);
  expect(settings.read()).toEqual({ mode: 'share', percent: 25 });
  values.set('SugarCubes.Memory.SharePercent', 90);
  expect(settings.read()).toEqual({ mode: 'share', percent: 75 });
  const changes: MemoryPreference[] = [];
  settings.subscribe((preference) => changes.push(preference));
  // The callback's value wins even before Comfy's store reports it.
  definitions[2]?.onChange(12);
  definitions[0]?.onChange('Fixed size');
  expect(changes.at(-1)).toEqual({ mode: 'fixed', bytes: 12 * 1024 * MIB });
  expect(() => new MemoryBudgetSettings({})).toThrow();
});

/** The shared vectors are the same documents the Rust host vendors. */
test('admits exactly the schema-valid allowances of the shared memory vectors', () => {
  const vectors = JSON.parse(
    readFileSync('tests/editor_integrations/support/memory_wire_vectors.json', 'utf8'),
  ) as { label: string; valid: boolean; value: unknown }[];
  const vectorSession = 'panel-0123456789abcdef';
  const allowances = vectors.filter(
    ({ value }) => (value as { type?: unknown } | null)?.type === 'allowance',
  );
  expect(allowances.length).toBeGreaterThan(2);
  for (const { label, valid, value } of allowances)
    expect([label, readAllowance(value, vectorSession) !== null]).toEqual([label, valid]);
  const accepted = allowances.find(({ valid }) => valid)?.value;
  expect(readAllowance(accepted, 'panel-ffffffffffffffff')).toBeNull();
});

test('every page report satisfies the generated memory schema, even from unbounded measurements', () => {
  const reports: MemoryClientMessage[] = [
    usageMessage(session, { usedBytes: 0, consumers: [] }),
    usageMessage(session, {
      usedBytes: Number.MAX_SAFE_INTEGER * 4,
      consumers: [
        { id: 'canvas.images', bytes: -5 },
        { id: 'huge', bytes: Number.MAX_SAFE_INTEGER * 2 },
      ],
    }),
    preferenceMessage(session, { mode: 'share', percent: 10 }),
    preferenceMessage(session, { mode: 'share', percent: 75 }),
    preferenceMessage(session, { mode: 'fixed', bytes: 1024 * 1024 * MIB }),
  ];
  for (const report of reports) expect(isMemoryClientMessage(report)).toBe(true);
  const settings = new MemoryBudgetSettings({
    ui: { settings: { addSetting: () => undefined, getSettingValue: () => undefined } },
  });
  expect(isMemoryClientMessage(preferenceMessage(session, settings.read()))).toBe(true);
});
