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
/** Prove the file choice protocol is one closed schema shared by the page and every host. */
import { readFileSync } from 'node:fs';
import { expect, test } from '@jest/globals';
import { fileChoiceBindings } from '../../../scripts/integrations/generate-file-choice-contract.js';
import {
  isFileChoiceHostMessage,
  isFileChoiceMessage,
} from '../../../integrations/shared/FileChoiceValidation.generated.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';

const SCHEMA = 'integrations/contracts/file-choice.schema.json';
const envelope = {
  protocol: 'sugarcubes.file-choice',
  version: 1,
  session: 'session-0123456789abcdef',
};

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await fileChoiceBindings(readFileSync(SCHEMA, 'utf8'));
  expect(Object.keys(bindings).sort()).toEqual([
    'integrations/contracts/FileChoiceProtocol.generated.rs',
    'integrations/contracts/FileChoiceProtocol.generated.ts',
    'integrations/shared/FileChoiceValidation.generated.ts',
  ]);
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('a host declares the file choice protocol by its schema name and fixed version', () => {
  expect(HOST_PROTOCOLS.fileChoice).toEqual({ name: 'sugarcubes.file-choice', version: 1 });
});

test('the closed wire contract admits a page’s two questions and a host’s three answers', () => {
  const open = {
    ...envelope,
    type: 'open',
    request: 'open-1',
    folder: 'C:\\p',
    start: '',
    extension: 'sugar',
  };
  const valid = [
    open,
    { ...open, type: 'save', request: 'save-2', name: 'Demo.sugar' },
    { ...open, type: 'save', request: 'save-2', name: '' },
    {
      ...envelope,
      type: 'chosen',
      request: 'open-1',
      name: 'a.sugar',
      relative: 'Demo/a.sugar',
      path: 'C:/p/Demo/a.sugar',
    },
    {
      ...envelope,
      type: 'chosen',
      request: 'open-1',
      name: 'a.sugar',
      relative: '',
      path: 'D:/a.sugar',
    },
    { ...envelope, type: 'dismissed', request: 'open-1' },
    { ...envelope, type: 'failed', request: 'open-1', reason: 'busy' },
    { ...envelope, type: 'failed', request: 'open-1', reason: 'unavailable' },
  ];
  const invalid = [
    { ...open, extension: '*.sugar' },
    { ...envelope, type: 'open', request: 'open-1', folder: '/p', extension: 'sugar' },
    { ...open, request: 'Open 1' },
    { ...open, text: 'content' },
    { ...open, type: 'save' },
    { ...open, version: 2 },
    { ...open, session: 'short' },
    { ...envelope, type: 'chosen', request: 'open-1', name: 'a.sugar' },
    { ...envelope, type: 'chosen', request: 'open-1', name: 'a.sugar', relative: '' },
    {
      ...envelope,
      type: 'chosen',
      request: 'open-1',
      name: 'a.sugar',
      relative: '',
      path: 'C:\\a',
    },
    { ...envelope, type: 'failed', request: 'open-1', reason: 'denied' },
  ];
  for (const value of valid) expect([value, isFileChoiceMessage(value)]).toEqual([value, true]);
  for (const value of invalid) expect([value, isFileChoiceMessage(value)]).toEqual([value, false]);
  // A page's own question is never taken for a host's answer.
  expect(isFileChoiceHostMessage(valid[0])).toBe(false);
  expect(isFileChoiceHostMessage(valid[3])).toBe(true);
});
