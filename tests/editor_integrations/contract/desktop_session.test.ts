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
/** Qualify the shared desktop-session boundary against language-neutral acceptance vectors. */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type {
  SessionCapabilities,
  SessionFence,
  SessionScope,
} from '../../../integrations/contracts/SessionProtocol.generated.js';
import {
  decodeDesktopCheckpoint,
  decodeSessionMessage,
} from '../../../integrations/shared/SessionDecode.js';
import {
  sessionAdmission,
  sessionMessageError,
} from '../../../integrations/shared/SessionProtocol.js';
import { isDesktopCheckpoint } from '../../../integrations/shared/SessionSnapshot.js';
import { sessionDirectory, sessionGraph } from '../../../integrations/shared/SessionPrimitives.js';
import { sessionBindings } from '../../../scripts/integrations/generate-session-contract.js';

interface Vector {
  readonly label: string;
  readonly valid: boolean;
  readonly value: unknown;
  readonly error?: string;
}
interface Vectors {
  readonly scope: SessionScope;
  readonly origin: string;
  readonly checkpoints: readonly Vector[];
  readonly messages: readonly Vector[];
  readonly admissions: readonly {
    readonly label: string;
    readonly message: number;
    readonly capabilities: SessionCapabilities;
    readonly committed: boolean;
    readonly persistence: boolean;
    readonly currentFence: SessionFence;
    readonly error: string | null;
  }[];
  readonly raw: readonly {
    readonly label: string;
    readonly kind: string;
    readonly text: string;
    readonly baseText: string;
    readonly mutation: string;
    readonly replaceFrom?: string;
    readonly replaceTo?: string;
    readonly valid: boolean;
  }[];
}
const root = process.cwd();
const vectors = JSON.parse(
  readFileSync(path.join(root, 'integrations/contracts/session_vectors.json'), 'utf8'),
) as Vectors;

test.each(vectors.checkpoints)('checkpoint: $label', ({ value, valid }) => {
  const text = JSON.stringify(value);
  expect(isDesktopCheckpoint(value)).toBe(valid);
  const decoded = decodeDesktopCheckpoint(text);
  expect(decoded !== null).toBe(valid);
  if (decoded) {
    expect(decoded).toEqual(value);
    expect(decodeDesktopCheckpoint(JSON.stringify(decoded))).toEqual(decoded);
  }
});

test.each(vectors.messages)('message: $label', ({ value, valid, error }) => {
  expect(sessionMessageError(value, vectors.scope, vectors.origin) === null).toBe(valid);
  if (error) expect(sessionMessageError(value, vectors.scope, vectors.origin)).toBe(error);
  const decoded = decodeSessionMessage(JSON.stringify(value), vectors.scope, vectors.origin);
  expect(decoded !== null).toBe(valid);
  if (decoded) expect(decoded).toEqual(value);
});

test.each(vectors.admissions)('capable and limited host admission: $label', (vector) => {
  const value = vectors.messages[vector.message];
  if (!value) throw new Error('Admission vector refers to an absent message');
  const message = decodeSessionMessage(JSON.stringify(value.value), vectors.scope, vectors.origin);
  if (!message) throw new Error('Admission vector requires a valid message');
  expect(
    sessionAdmission(
      message,
      vector.capabilities,
      vector.committed,
      vector.persistence,
      vector.currentFence,
    ),
  ).toBe(vector.error);
});

test.each(vectors.raw)('wire rejection: $label', (vector) => {
  const baseline =
    vector.kind === 'checkpoint'
      ? decodeDesktopCheckpoint(vector.baseText)
      : decodeSessionMessage(vector.baseText, vectors.scope, vectors.origin);
  expect(baseline).not.toBeNull();
  if (vector.mutation === 'duplicate' || vector.mutation === 'escaped-duplicate') {
    const suffix = vector.mutation === 'duplicate' ? ',"version":1}' : ',"vers\\u0069on":1}';
    expect(vector.text).toBe(vector.baseText.slice(0, -1) + suffix);
    expect(JSON.parse(vector.text) as unknown).toEqual(JSON.parse(vector.baseText) as unknown);
  } else if (vector.mutation === 'truncated')
    expect(vector.text).toBe(vector.baseText.slice(0, -1));
  else if (vector.mutation === 'unpaired-unicode') {
    if (!vector.replaceFrom || !vector.replaceTo) throw new Error('Missing exact Unicode mutation');
    expect(vector.baseText.split(vector.replaceFrom)).toHaveLength(2);
    expect(vector.text).toBe(vector.baseText.replace(vector.replaceFrom, vector.replaceTo));
  } else throw new Error('Unknown raw rejection mutation');
  const decoded =
    vector.kind === 'checkpoint'
      ? decodeDesktopCheckpoint(vector.text)
      : decodeSessionMessage(vector.text, vectors.scope, vectors.origin);
  expect(decoded !== null).toBe(vector.valid);
});

test('the authoritative schema regenerates every checked-in language binding', async () => {
  const outputs = await sessionBindings(
    readFileSync(path.join(root, 'integrations/contracts/session.schema.json'), 'utf8'),
  );
  expect(outputs['integrations/contracts/SessionProtocol.generated.rs']).toContain(
    'output: Box<SessionOutput>',
  );
  for (const [relative, source] of Object.entries(outputs)) {
    expect(readFileSync(path.join(root, relative), 'utf8').replaceAll('\r\n', '\n')).toBe(source);
  }
});

test('session namespace is relative to the installation user-data root', () => {
  expect(sessionDirectory(vectors.scope.profile)).toBe(`sessions/${vectors.scope.profile}/`);
  expect(sessionDirectory('../desktop-data')).toBeNull();
  expect(sessionDirectory('real')).toBe('sessions/real/');
  expect(sessionDirectory('')).toBeNull();
  expect(sessionDirectory('x'.repeat(64))).toBe(`sessions/${'x'.repeat(64)}/`);
  expect(sessionDirectory('x'.repeat(65))).toBeNull();
});

test('rejects nonfinite cameras and invalid Unicode before JSON serialization erases them', () => {
  const base = vectors.checkpoints[0];
  if (!base) throw new Error('Missing mixed-workspace fixture');
  const value = decodeDesktopCheckpoint(JSON.stringify(base.value));
  const entry = value?.entries[0];
  const camera = entry?.view.cameras[0];
  if (!value || !entry || !camera) throw new Error('Missing final camera fixture');
  for (const spanX of [Number.NaN, Infinity, -Infinity])
    expect(
      isDesktopCheckpoint({
        ...value,
        entries: [
          {
            ...entry,
            view: {
              ...entry.view,
              cameras: [{ ...camera, inspection: { ...camera.inspection, spanX } }],
            },
          },
        ],
      }),
    ).toBe(false);
  expect(isDesktopCheckpoint({ ...value, producer: '\ud800' })).toBe(false);
  expect(decodeDesktopCheckpoint(JSON.stringify({ ...value, producer: '\ud800' }))).toBeNull();
});

test('bounds encoded messages, Unicode chunks and Comfy-owned graph envelopes independently', () => {
  expect(decodeSessionMessage(' '.repeat(524289), vectors.scope, vectors.origin)).toBeNull();
  const base = vectors.messages[4];
  if (!base) throw new Error('Missing chunk fixture');
  const message = decodeSessionMessage(JSON.stringify(base.value), vectors.scope, vectors.origin);
  if (!message || message.operation.kind !== 'chunk') throw new Error('Expected chunk fixture');
  expect(
    sessionMessageError(
      { ...message, operation: { ...message.operation, text: '🍬'.repeat(16384) } },
      vectors.scope,
      vectors.origin,
    ),
  ).toBeNull();
  expect(
    sessionMessageError(
      { ...message, operation: { ...message.operation, text: '🍬'.repeat(16385) } },
      vectors.scope,
      vectors.origin,
    ),
  ).toBe('invalid');
  expect(sessionGraph('{"arbitraryComfyOwnedFacts":true}')).toBe(true);
  expect(sessionGraph('{"value":1e999}')).toBe(false);
  expect(sessionGraph(`{"value":${'['.repeat(65)}0${']'.repeat(65)}}`)).toBe(false);
  expect(sessionGraph(' '.repeat(33554433))).toBe(false);
});
