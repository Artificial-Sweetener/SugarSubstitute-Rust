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
/** Prove native observer ownership is enforced through aliases and scoped host contracts. */
import { afterEach, expect, test } from '@jest/globals';
import ts from 'typescript';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { auditInvalidation } from '../../../scripts/invalidation-audit.js';

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

/** Compile a real DOM-typed source fixture with isolated policy ownership. */
function audit(source: string, approved = false) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'sc-observer-audit-'));
  roots.push(root);
  mkdirSync(path.join(root, 'frontend'));
  const file = path.join(root, 'frontend/feature.ts');
  writeFileSync(file, source);
  const program = ts.createProgram([file], { target: ts.ScriptTarget.ES2022, strict: true });
  return auditInvalidation(program, root, new Set(approved ? ['frontend/feature.ts'] : []));
}

test('rejects direct, alias and defaultView constructors outside the native owner', () => {
  expect(audit('new MutationObserver(() => {});')).toEqual([
    expect.objectContaining({ rule: 'UI_OBSERVER_OWNER' }),
  ]);
  expect(audit('const Watch = window.MutationObserver; new Watch(() => {});')).toHaveLength(1);
  expect(
    audit('const Watch = document.defaultView?.MutationObserver; if(Watch) new Watch(() => {});'),
  ).toHaveLength(1);
  expect(audit('new MutationObserver(() => {});', true)).toEqual([]);
  expect(audit('class Watch extends MutationObserver {} new Watch(() => {});')).toHaveLength(1);
  expect(
    audit(
      'declare const observer:MutationObserver; observer.observe(document.body,{subtree:true});',
    ),
  ).toEqual([
    expect.objectContaining({ rule: 'UI_OBSERVER_OWNER' }),
    expect.objectContaining({ rule: 'UI_DOCUMENT_OBSERVER' }),
  ]);
});

test('does not confuse an unrelated same-name class with the native browser contract', () => {
  expect(audit('export {}; class MutationObserver {} new MutationObserver();')).toEqual([]);
});

test('approved local owners cannot start observing the document subtree through a root alias', () => {
  expect(
    audit(
      'const observer=new MutationObserver(()=>{}); const root=document.body; observer.observe(root,{subtree:true});',
      true,
    ),
  ).toEqual([expect.objectContaining({ rule: 'UI_DOCUMENT_OBSERVER' })]);
  expect(
    audit(
      'const observer=new MutationObserver(()=>{}); observer.observe(document.body,{attributes:true});',
      true,
    ),
  ).toEqual([]);
  expect(
    audit(
      'const observer=new MutationObserver(()=>{}); observer.observe(document.body,{subtree:false,attributes:true});',
      true,
    ),
  ).toEqual([]);
});

test('rejects wildcard and dynamic surface selectors and permits declared scoped constants', () => {
  const declaration = 'class HostSurfaceObserver {constructor(root:Node,selector:string){}};';
  expect(audit(declaration + 'new HostSurfaceObserver(document.body,"*");')).toHaveLength(1);
  expect(
    audit(
      declaration +
        'const selector=location.hash; new HostSurfaceObserver(document.body,selector);',
    ),
  ).toHaveLength(1);
  expect(
    audit(
      declaration + 'const selector=".lg-node"; new HostSurfaceObserver(document.body,selector);',
    ),
  ).toEqual([]);
});
