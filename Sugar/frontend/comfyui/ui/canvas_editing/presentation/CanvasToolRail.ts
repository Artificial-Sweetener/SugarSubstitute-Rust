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
/** Present the vertical tool rail, with grouped tools chosen from flyouts. */
import { defineComponent, h, onBeforeUnmount, type PropType } from 'vue';
import type { CanvasToolId } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { ContextMenu, type ContextMenuEntry } from '../../menus/ContextMenu.js';
import type { CanvasEditSlotId, CanvasEditSlotView } from '../domain/CanvasEditTools.js';
import { canvasEditIcon, createCanvasEditIcon, CANVAS_TOOL_ICONS } from './CanvasEditIcons.js';
import { CANVAS_EDIT_TEXT, CANVAS_TOOL_LABELS } from './CanvasEditText.js';

/**
 * Render one button per slot in rail order.
 *
 * A click activates the slot's presented member. A grouped slot opens its member flyout on the
 * secondary button, the context-menu key or Shift+F10; choosing a member activates it
 * and makes it the slot's presented member.
 */
export const CanvasToolRail = defineComponent({
  name: 'CanvasToolRail',
  props: {
    slots: { type: Array as PropType<readonly CanvasEditSlotView[]>, required: true },
    selectSlot: { type: Function as PropType<(slot: CanvasEditSlotId) => void>, required: true },
    selectTool: { type: Function as PropType<(tool: CanvasToolId) => void>, required: true },
  },
  setup(props) {
    let menu: ContextMenu | null = null;
    onBeforeUnmount(() => menu?.close());
    /** Open one group's members beside its button, or at the pointer for a secondary click. */
    function openFlyout(slot: CanvasEditSlotView, trigger: HTMLElement, event: Event): void {
      event.preventDefault();
      event.stopPropagation();
      const bounds = trigger.getBoundingClientRect();
      menu ??= new ContextMenu(trigger.ownerDocument);
      menu.open({
        label: CANVAS_EDIT_TEXT.toolGroup(CANVAS_TOOL_LABELS[slot.current.id]),
        anchor:
          event instanceof MouseEvent
            ? { kind: 'pointer', x: event.clientX, y: event.clientY }
            : { kind: 'submenu', bounds },
        trigger,
        attributes: { 'data-canvas-edit-flyout': slot.id },
        entries: slot.members.map(
          (member): ContextMenuEntry => ({
            label: CANVAS_TOOL_LABELS[member.id],
            selection: 'single',
            checked: member.id === slot.current.id,
            disabled: !member.available,
            attributes: { 'data-canvas-tool': member.id },
            icon: (document) => createCanvasEditIcon(document, CANVAS_TOOL_ICONS[member.id]),
            select: () => props.selectTool(member.id),
          }),
        ),
      });
    }
    return () =>
      h(
        'div',
        {
          class: 'sc-canvas-edit-chrome sc-canvas-edit-rail',
          role: 'toolbar',
          'aria-orientation': 'vertical',
          'aria-label': CANVAS_EDIT_TEXT.tools,
        },
        props.slots.map((slot) =>
          h(
            'button',
            {
              key: slot.id,
              type: 'button',
              'data-canvas-slot': slot.id,
              'data-canvas-tool': slot.current.id,
              title: CANVAS_TOOL_LABELS[slot.current.id],
              'aria-label': CANVAS_TOOL_LABELS[slot.current.id],
              'aria-pressed': String(slot.current.active),
              'aria-haspopup': slot.grouped ? 'menu' : undefined,
              disabled: !slot.current.available,
              onClick: () => props.selectSlot(slot.id),
              onContextmenu: (event: MouseEvent) => {
                if (slot.grouped && event.currentTarget instanceof HTMLElement)
                  openFlyout(slot, event.currentTarget, event);
              },
              onKeydown: (event: KeyboardEvent) => {
                const opens =
                  event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10');
                if (slot.grouped && opens && event.currentTarget instanceof HTMLElement)
                  openFlyout(slot, event.currentTarget, event);
              },
            },
            [canvasEditIcon(CANVAS_TOOL_ICONS[slot.current.id])],
          ),
        ),
      );
  },
});
