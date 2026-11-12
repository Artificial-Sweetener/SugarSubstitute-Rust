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
/** Render the prompt-scene choice of a scene run as one combo-box layer of the navigation bar. */

import { defineComponent, h, type PropType } from 'vue';
import type { CanvasCommand, CanvasSceneSummary } from '../../domain/CanvasViewPort.js';
import { CanvasAnchoredPicker } from './CanvasAnchoredPicker.js';

/** The row that shows every scene together; no normalized scene title can spell it. */
const ALL = '\u0000all';

/**
 * Offer All first, then each scene by its literal title; scenes with nothing to show are disabled.
 */
export const CanvasScenePicker = defineComponent({
  name: 'CanvasScenePicker',
  props: {
    scenes: { type: Array as PropType<readonly CanvasSceneSummary[]>, required: true },
    scene: { type: String as PropType<string | null>, default: null },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Translate a picker row into the scene command of the shared view authority. */
  setup(props) {
    /** Choose every scene or one declared, available scene. */
    function choose(value: string): void {
      if (value === ALL) {
        props.send({ type: 'select-scene', scene: null });
        return;
      }
      const scene = props.scenes.find((entry) => entry.key === value);
      if (scene?.available) props.send({ type: 'select-scene', scene: scene.key });
    }
    return () =>
      h('span', { class: 'sc-output-scene', 'data-sc-fixed-navigation': 'true' }, [
        h(CanvasAnchoredPicker, {
          label: 'Output scene',
          value: props.scenes.some((entry) => entry.key === props.scene)
            ? (props.scene ?? ALL)
            : ALL,
          items: [
            { value: ALL, label: 'All' },
            ...props.scenes.map((entry) => ({
              value: entry.key,
              label: entry.title,
              disabled: !entry.available,
            })),
          ],
          stable: true,
          choose,
        }),
      ]);
  },
});
