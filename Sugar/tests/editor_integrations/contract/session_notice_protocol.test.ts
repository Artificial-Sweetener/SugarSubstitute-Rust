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
/** Prove the session notice protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { sessionNoticeBindings } from '../../../scripts/integrations/generate-session-notice-contract.js';
import {
  isSessionNoticeClientMessage,
  isSessionNoticeHostMessage,
  isSessionNoticeMessage,
} from '../../../integrations/shared/SessionNoticeValidation.generated.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';

interface Vector {
  readonly label: string;
  readonly valid: boolean;
  readonly value: unknown;
}

const SCHEMA = 'integrations/contracts/session-notice.schema.json';
const vectors = JSON.parse(
  readFileSync('integrations/contracts/session_notice_wire_vectors.json', 'utf8'),
) as Vector[];

/** Read the message type a vector carries, when it carries one. */
function typeOf(value: unknown): unknown {
  return value && typeof value === 'object' ? (value as { type?: unknown }).type : undefined;
}

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await sessionNoticeBindings(readFileSync(SCHEMA, 'utf8'));
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/SessionNoticeProtocol.generated.rs',
    'integrations/contracts/SessionNoticeProtocol.generated.ts',
    'integrations/shared/SessionNoticeValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('a host declares the session notice protocol by its schema name and fixed version', () => {
  expect(HOST_PROTOCOLS.sessionNotice).toEqual({ name: 'sugarcubes.session-notice', version: 1 });
});

test('shared session notice vectors preserve the closed version-one wire contract', () => {
  expect(vectors.some((vector) => vector.valid)).toBe(true);
  expect(vectors.some((vector) => !vector.valid)).toBe(true);
  for (const vector of vectors)
    expect([vector.label, isSessionNoticeMessage(vector.value)]).toEqual([
      vector.label,
      vector.valid,
    ]);
});

test('direction guards admit only their own valid messages', () => {
  for (const { label, valid, value } of vectors) {
    const host = ['not-saved', 'none'].includes(String(typeOf(value)));
    expect([label, isSessionNoticeHostMessage(value)]).toEqual([label, valid && host]);
    expect([label, isSessionNoticeClientMessage(value)]).toEqual([label, valid && !host]);
  }
});
