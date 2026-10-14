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
/** Prove context presentation ownership through real compiler resolution and DOM semantics. */
import { afterEach, expect, test } from '@jest/globals';
import ts from 'typescript';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  auditContextMenus,
  isMenuOwnership,
  type MenuOwnership,
} from '../../../scripts/context-menu-audit.js';

const roots: string[] = [];
const policy: MenuOwnership = {
  context_menu: 'frontend/menus/ContextMenu.ts',
  floating_surface: 'frontend/menus/FloatingSurface.ts',
  fluent_runtime: 'frontend/menus/FluentMenuRuntime.ts',
  search_dialogs: ['frontend/SearchDialog.ts'],
  choice_controls: ['frontend/Choice.ts'],
};
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

/** Compile exact owner declarations and one feature without evaluating authored code. */
function audit(source: string, file = 'frontend/feature.ts') {
  const root = mkdtempSync(path.join(os.tmpdir(), 'sc-context-audit-'));
  roots.push(root);
  const files: Record<string, string> = {
    [policy.context_menu]: 'export class ContextMenu { open(value:unknown):void {} }',
    [policy.floating_surface]:
      'export class FloatingSurface { constructor(element:HTMLElement) {} }',
    [policy.fluent_runtime]:
      'export function createFluentMenuList(doc:Document):HTMLElement {return doc.createElement("fluent-menu-list");}',
    [file]: source,
  };
  for (const [relative, content] of Object.entries(files)) {
    const target = path.join(root, relative);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, content);
  }
  const program = ts.createProgram(
    Object.keys(files).map((relative) => path.join(root, relative)),
    { target: ts.ScriptTarget.ES2022, strict: true },
  );
  return auditContextMenus(program, root, policy);
}

test('allows shared ContextMenu aliases and ordinary feature event delegation', () => {
  expect(
    audit(
      'import {ContextMenu as Actions} from "./menus/ContextMenu"; const menu=new Actions();document.body.addEventListener("contextmenu",()=>menu.open({}));',
    ),
  ).toEqual([]);
});

test.each([
  'class ContextMenu {} new ContextMenu();',
  'declare const LiteGraph:{ContextMenu:new()=>object};const Foreign=LiteGraph.ContextMenu;new Foreign();',
  'class HostMenu {} declare const LiteGraph:{ContextMenu:new()=>HostMenu};const Foreign=LiteGraph.ContextMenu;new Foreign();',
  'import {ContextMenu} from "./menus/ContextMenu";class Independent extends ContextMenu {}',
  'import {FloatingSurface as Popup} from "./menus/FloatingSurface";new Popup(document.body);',
])('rejects independent or inherited menu interaction owners: %s', (source) => {
  expect(audit(source)).toEqual(
    expect.arrayContaining([expect.objectContaining({ rule: 'UI_CONTEXT_MENU_OWNER' })]),
  );
});

test.each([
  'const role="menu"; document.body.setAttribute("role",role);',
  'document.body.role="menuitemcheckbox";',
  'const assign=document.body.setAttribute.bind(document.body);assign("role","menu");',
  'declare function render(tag:string,props:object):void;render("div",{role:"menu"});',
  'document.createElement("fluent-menu-item");',
  'import {createFluentMenuList as list} from "./menus/FluentMenuRuntime";const build=list;build(document);',
])(
  'rejects feature-owned context rendering through DOM, Vue props or factory aliases: %s',
  (source) => {
    expect(audit(source)).toEqual(
      expect.arrayContaining([expect.objectContaining({ rule: 'UI_CONTEXT_MENU_RENDERER' })]),
    );
  },
);

test.each([
  'document.body.showPopover();',
  'import {FloatingSurface} from "./menus/FloatingSurface";class Independent extends FloatingSurface {}',
  'const open=document.body.showPopover.bind(document.body);open();',
  'document.body.setAttribute("popover","manual");',
  'declare function render(tag:string,props:object):void;render("div",{popover:"manual"});',
])('rejects independent popup transport: %s', (source) => {
  expect(audit(source)).toEqual(
    expect.arrayContaining([expect.objectContaining({ rule: 'UI_POPUP_OWNER' })]),
  );
});

test('allows declared search dialog result rows and unrelated accessible controls', () => {
  expect(audit('document.body.setAttribute("role","menu");', 'frontend/SearchDialog.ts')).toEqual(
    [],
  );
  expect(audit('document.body.setAttribute("role","dialog");document.body.role="button";')).toEqual(
    [],
  );
  expect(audit('document.body.showPopover();', policy.floating_surface)).toEqual([]);
  expect(
    audit(
      'import {FloatingSurface} from "./menus/FloatingSurface";new FloatingSurface(document.body);',
      'frontend/Choice.ts',
    ),
  ).toEqual([]);
  expect(audit('document.body.role="menu";', 'frontend/Choice.ts')).toEqual(
    expect.arrayContaining([expect.objectContaining({ rule: 'UI_CONTEXT_MENU_RENDERER' })]),
  );
});

test('fails closed when an ownership descriptor is malformed', () => {
  for (const value of [
    null,
    {},
    { ...policy, search_dialogs: [1] },
    { ...policy, context_menu: 1 },
  ])
    expect(isMenuOwnership(value)).toBe(false);
  expect(isMenuOwnership(policy)).toBe(true);
});
