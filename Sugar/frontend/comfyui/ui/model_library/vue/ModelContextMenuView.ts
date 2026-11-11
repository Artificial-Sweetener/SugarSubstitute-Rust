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
/** Adapt model actions to the universal SugarCubes menu without owning popup behavior. */
import { defineComponent, onBeforeUnmount, onMounted, watch } from 'vue';
import type { PropType } from 'vue';
import { ContextMenu } from '../../menus/ContextMenu.js';
import type { ModelContextEntry } from '../ModelContextMenu.js';
import { projectModelContext } from '../ModelContextProjection.js';

/** Keep model policy callbacks intact while the shared owner handles icons, layout and interaction. */
export const ModelContextMenuView = defineComponent({
  name: 'SugarCubesModelContextMenuView',
  props: {
    entries: { type: Array as PropType<readonly ModelContextEntry[]>, required: true },
    x: { type: Number, required: true },
    y: { type: Number, required: true },
    label: { type: String, default: 'Model actions' },
  },
  emits: { close: () => true, action: (_entry: ModelContextEntry) => true },
  setup(props, { emit }) {
    const menu = new ContextMenu(document);
    let disposed = false;
    /** Replace an open target when another card requests a menu in the same Vue lifetime. */
    const open = (): void => {
      disposed = true;
      menu.close();
      disposed = false;
      menu.open({
        label: props.label,
        entries: projectModelContext(props.entries, (item) => emit('action', item)),
        anchor: { kind: 'pointer', x: props.x, y: props.y },
        closed: () => {
          if (!disposed) emit('close');
        },
      });
    };
    onMounted(open);
    watch(() => [props.entries, props.x, props.y], open);
    onBeforeUnmount(() => {
      disposed = true;
      menu.close();
    });
    return () => null;
  },
});
