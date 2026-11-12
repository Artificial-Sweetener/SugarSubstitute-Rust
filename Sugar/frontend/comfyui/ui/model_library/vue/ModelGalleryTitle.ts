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
/** Reveal the active overflowing gallery title while keeping animation local to its lifetime. */
import { defineComponent, h, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { modelTitleMarquee } from '../ModelTitleMarquee.js';
import type { ModelTitleMarqueeState } from '../ModelTitleMarquee.js';

/** Measure real glyph width and animate only a visible, active title that needs scrolling. */
export const ModelGalleryTitle = defineComponent({
  name: 'SugarCubesModelGalleryTitle',
  props: { title: { type: String, required: true }, active: { type: Boolean, required: true } },
  setup(props) {
    const viewport = ref<HTMLElement | null>(null);
    const text = ref<HTMLElement | null>(null);
    const state = ref<ModelTitleMarqueeState | null>(null);
    let observer: ResizeObserver | null = null;
    let timer: ReturnType<typeof setInterval> | null = null;
    let preference: MediaQueryList | null = null;
    let disposed = false;

    /** Stop owned work before restarting a title or disposing its tile. */
    const stop = (): void => {
      if (timer !== null) clearInterval(timer);
      timer = null;
      state.value = null;
    };
    /** Restart the reference start hold after selection, glyph size or motion preference changes. */
    const restart = (): void => {
      stop();
      if (disposed || !props.active || preference?.matches || !viewport.value || !text.value)
        return;
      const overflow = text.value.getBoundingClientRect().width - viewport.value.clientWidth;
      if (overflow <= 0) return;
      const started = performance.now();
      state.value = modelTitleMarquee(0, overflow);
      timer = setInterval(() => {
        state.value = modelTitleMarquee(performance.now() - started, overflow);
      }, 33);
    };
    onMounted(() => {
      preference = window.matchMedia?.('(prefers-reduced-motion: reduce)') ?? null;
      preference?.addEventListener('change', restart);
      observer = new ResizeObserver(restart);
      if (viewport.value) observer.observe(viewport.value);
      document.fonts?.addEventListener('loadingdone', restart);
      restart();
    });
    watch(() => [props.active, props.title], restart, { flush: 'post' });
    onBeforeUnmount(() => {
      disposed = true;
      stop();
      observer?.disconnect();
      preference?.removeEventListener('change', restart);
      document.fonts?.removeEventListener('loadingdone', restart);
    });
    return () => {
      const motion = state.value;
      return h(
        'span',
        {
          ref: viewport,
          class: 'sc-model-title',
          'data-marquee-phase': motion?.phase ?? 'static',
          'data-marquee-left-fade': motion?.leftFade ?? false,
          'data-marquee-right-fade': motion?.rightFade ?? false,
        },
        h(
          'span',
          {
            ref: text,
            class: 'sc-model-marquee-text',
            dir: 'ltr',
            style:
              motion?.phase === 'scroll'
                ? { transform: `translateX(-${String(motion.offset)}px)` }
                : undefined,
          },
          props.title,
        ),
      );
    };
  },
});
