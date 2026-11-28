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
/** Inventory every route and node the installed pack actually registered in Comfy's server. */
import { test, expect } from '../support/LiveComfy.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

test('all contributed routes, node schemas and consumed backend members survive host startup', async ({
  request,
}) => {
  const response = await request.get('/sugar-test-host/facts');
  await expect(response).toBeOK();
  const facts: unknown = await response.json();
  if (
    !isRecord(facts) ||
    !Array.isArray(facts.routes) ||
    !isRecord(facts.nodeDefinitions) ||
    !isRecord(facts.members)
  )
    throw new Error('Test host did not report its running registration tables');
  expect(facts.routes).toContain('POST /sugarcubes/v2/executions/queue');
  expect(facts.nodeDefinitions).toHaveProperty(['SugarCubes.CubeOutput']);
  const wireSchemas: Record<string, unknown> = {};
  for (const name of Object.keys(facts.nodeDefinitions).sort()) {
    const response = await request.get('/object_info/' + encodeURIComponent(name));
    await expect(response).toBeOK();
    const body: unknown = await response.json();
    const definition = isRecord(body) ? body[name] : null;
    if (!isRecord(definition)) throw new Error('Comfy omitted the registered schema for ' + name);
    wireSchemas[name] = {
      inputs: definition.input,
      order: definition.input_order,
      outputs: definition.output,
      names: definition.output_name,
      lists: definition.output_is_list,
      outputNode: definition.output_node,
      category: definition.category,
      display: definition.display_name,
    };
  }
  // Bind each member to the object its SugarCubes consumer actually calls.
  const consumed: Record<string, string[]> = {
    execution: ['SENSITIVE_EXTRA_DATA_KEYS', 'validate_prompt'],
    server: [
      'add_on_prompt_handler',
      'instance',
      'node_replace_manager',
      'number',
      'prompt_queue',
      'queue_updated',
      'routes',
      'start_multi_address',
    ],
    queue: [
      'delete_history_item',
      'delete_queue_item',
      'get_current_queue',
      'get_history',
      'mutex',
      'put',
      'queue',
      'server',
    ],
    replacements: ['apply_replacements'],
    folder_paths: [
      'annotated_filepath',
      'base_path',
      'filename_list_cache',
      'folder_names_and_paths',
      'get_filename_list',
      'get_folder_paths',
      'get_full_path',
      'get_input_directory',
      'get_output_directory',
      'get_save_image_path',
      'get_system_user_directory',
      'get_temp_directory',
      'models_dir',
    ],
  };
  const members: Record<string, string[]> = {};
  for (const [target, required] of Object.entries(consumed)) {
    const keys = facts.members[target];
    if (!Array.isArray(keys)) throw new Error('Invalid host member list for ' + target);
    for (const member of required) expect(keys, target + '.' + member).toContain(member);
    members[target] = required;
  }
  const snapshot = {
    routes: facts.routes,
    nodes: Object.fromEntries(Object.entries(facts.nodeDefinitions).sort()),
    wireSchemas,
    members,
  };
  expect(JSON.stringify(snapshot, null, 2)).toMatchSnapshot('backend-contract.json');
});
