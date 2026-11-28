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
/** Compare the actual live host objects used throughout the source dependency inventory. */
import { readFile } from 'node:fs/promises';
import { test, expect } from '../support/LiveComfy.js';

interface MemberRequest {
  owner: string;
  key: string;
}

/** Group exact source expressions by the representative host object that supplies the member. */
function requests(inventory: Record<string, string[]>): MemberRequest[] {
  const members = new Map<string, MemberRequest>();
  const aliases: Record<string, string> = {
    appref: 'app',
    apprecord: 'app',
    comfyapp: 'app',
    graphcanvas: 'canvas',
    workflowstore: 'workflow',
    graph: 'graph',
    canvas: 'canvas',
    comfyapplication: 'app',
    comfygraph: 'graph',
    comfynode: 'node',
    comfywidget: 'widget',
    comfyoutput: 'output',
    comfycanvas: 'canvas',
    litegraphhost: 'litegraph',
  };
  for (const entries of Object.values(inventory)) {
    for (const expression of entries) {
      const match =
        /^host member (?:this\.(?:#?options\.)?)?(app(?:Ref|Record)?|comfyApp|graph|canvas|graphCanvas|liteGraph|workflowStore|extensionManager|nodeData)\??\.([a-zA-Z_$][\w$]*)$/i.exec(
          expression,
        );
      const typed = /^typed host member (\w+)\.(\w+)$/.exec(expression);
      const privateAdapter = /^host adapter member (workflow|api|canvas|graph|app)\.([\w$]+)$/.exec(
        expression,
      );
      const access = typed ?? match ?? privateAdapter;
      if (!access || (typed && !aliases[access[1]!.toLowerCase()])) continue;
      const owner = aliases[access[1]!.toLowerCase()] ?? access[1]!.toLowerCase();
      const key = access[2]!;
      members.set(owner + '.' + key, { owner, key });
    }
  }
  return [...members.entries()]
    .sort(([a], [b]) => a.localeCompare(b, 'en'))
    .map(([, value]) => value);
}

test('live host members consumed by SugarCubes remain reachable on the representative graph', async ({
  loaded: page,
}) => {
  const inventory = JSON.parse(
    await readFile(new URL('../support/host-dependencies.json', import.meta.url), 'utf8'),
  ) as Record<string, string[]>;
  const members = await page.evaluate(async (targets) => {
    const { app } = await import('/scripts/app.js');
    const manager: unknown = Reflect.get(app, 'extensionManager');
    const node = window.LiteGraph?.createNode('EmptyImage');
    const objects: Record<string, unknown> = {
      app,
      graph: app.graph,
      canvas: Reflect.get(app, 'canvas'),
      litegraph: window.LiteGraph,
      api: Reflect.get(app, 'api'),
      extensionmanager: manager,
      workflow: manager && typeof manager === 'object' ? Reflect.get(manager, 'workflow') : null,
      node,
      widget: node?.widgets?.[0],
      output: node?.outputs?.[0],
    };
    const result: Record<string, string[]> = {};
    for (const { owner, key } of targets) {
      const host = objects[owner];
      if (!host || (typeof host !== 'object' && typeof host !== 'function') || !(key in host))
        continue;
      (result[owner] ??= []).push(key);
    }
    return result;
  }, requests(inventory));
  expect(Object.keys(members)).toEqual(
    expect.arrayContaining(['app', 'graph', 'canvas', 'node', 'widget']),
  );
  expect(JSON.stringify(members, null, 2)).toMatchSnapshot('native-members.json');
});
