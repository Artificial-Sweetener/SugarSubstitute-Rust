import { isRecord } from '../types/common.js';
import { VISUAL_STYLE_SETTING, readVisualStylePreference, } from '../surface/appearance/VisualStylePreference.js';
import { readComfyDarkTheme } from '../surface/appearance/ComfyColorScheme.js';
import { applyOwnedTheme, visualStyleTokens } from '../surface/appearance/ThemeService.js';
import { modelPickerCss } from './ModelPickerStyles.js';
/** Own scoped theme annotations, styles and subscriptions for mounted picker surfaces. */
export class ModelPickerAppearance {
    app;
    #document;
    #settings;
    #style;
    #observer;
    #surfaces = new Set();
    #active = false;
    #dark = true;
    /** Bind appearance to the same persisted preference as native Fluent cards. */
    constructor(documentRef, app) {
        this.app = app;
        this.#document = documentRef;
        const ui = isRecord(app) && isRecord(app.ui) ? app.ui : {};
        this.#settings = isRecord(ui.settings) ? ui.settings : {};
        this.#style = documentRef.createElement('style');
        this.#style.id = 'sugarcubes-model-picker';
        this.#observer = new MutationObserver(this.#refresh);
        for (const root of [documentRef.documentElement, documentRef.body])
            this.#observer.observe(root, { attributes: true, attributeFilter: ['class', 'style'] });
        if (typeof this.#settings.addEventListener === 'function')
            this.#settings.addEventListener.call(this.#settings, `${VISUAL_STYLE_SETTING}.change`, this.#onPreferenceChange);
    }
    /** Enable styling for the supported renderer lifetime. */
    activate() {
        this.#active = true;
        this.#document.head.append(this.#style);
        this.#refresh();
    }
    /** Theme one new field or portalled popup using the current host preference. */
    track(surface) {
        this.#surfaces.add(surface);
        this.#apply(surface);
    }
    /** Release scoped tokens before removing an owned surface. */
    release(surface) {
        this.#surfaces.delete(surface);
        applyOwnedTheme(surface, null);
        delete surface.dataset.scModelAppearance;
    }
    /** Restore host appearance when the supported renderer lifetime ends. */
    deactivate() {
        this.#active = false;
        this.#style.remove();
        for (const surface of [...this.#surfaces])
            this.release(surface);
    }
    /** Remove preference observers after releasing all scoped presentation. */
    dispose() {
        this.deactivate();
        this.#observer.disconnect();
        if (typeof this.#settings.removeEventListener === 'function')
            this.#settings.removeEventListener.call(this.#settings, `${VISUAL_STYLE_SETTING}.change`, this.#onPreferenceChange);
    }
    /** Read only the authoritative host preference, leaving control enablement independent. */
    #visualStyle() {
        return readVisualStylePreference(this.app);
    }
    /** Read after Comfy commits the value following its synchronous change dispatch. */
    #onPreferenceChange = () => {
        queueMicrotask(this.#refresh);
    };
    /** Refresh live surfaces in place so search, focus and keyboard selection survive theme changes. */
    #refresh = () => {
        if (!this.#active)
            return;
        this.#dark = readComfyDarkTheme(this.#document);
        const style = this.#visualStyle();
        this.#style.textContent = `${modelPickerCss}\n[data-sc-model-appearance="${style}"]{${style === 'comfy' ? '' : visualStyleTokens(style, this.#dark)}}\n[data-sc-model-native]{display:none!important}.sc-model-mount{width:100%;min-width:0}`;
        for (const surface of this.#surfaces)
            this.#apply(surface);
    };
    /** Apply Microsoft tokens only to Fluent surfaces; native controls inherit Comfy tokens. */
    #apply(surface) {
        const style = this.#visualStyle();
        surface.dataset.scModelAppearance = style === 'comfy' ? 'native' : style;
        applyOwnedTheme(surface, style === 'comfy' ? null : this.#dark, style === 'comfy' ? 'fluent' : style);
    }
}
