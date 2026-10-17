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
/** Present SugarCubes identity, component versions, licensing and acknowledgements in Comfy settings. */
import { sugarCubesProject, type AboutInformationSource } from './AboutInformation.js';
import { ModalShell, type ModalAdapter } from '../dialogs/ModalShell.js';
import type { AppearanceSource } from '../surface/appearance/AppearanceState.js';

/** Keep one stable page while version and license requests complete asynchronously. */
export class AboutSettingsPage {
  readonly element: HTMLElement;
  readonly #versions: HTMLElement;
  readonly #status: HTMLElement;
  readonly #retry: HTMLButtonElement;
  readonly #license: ModalShell;
  #loading = false;
  #loaded = false;
  #disposed = false;

  /** Bind host-neutral metadata and the established SugarCubes modal/feedback system. */
  constructor(
    private readonly document: Document,
    private readonly source: AboutInformationSource,
    adapter: ModalAdapter,
    private readonly report: (error: unknown) => void,
    appearance?: AppearanceSource,
  ) {
    this.element = document.createElement('section');
    this.element.className = 'sc-settings-about';
    this.element.setAttribute('aria-label', 'About SugarCubes');
    this.#license = new ModalShell({
      adapter,
      dialogClassName: 'sc-settings-license',
      ...(appearance ? { appearanceSource: appearance } : {}),
    });
    const identity = document.createElement('header');
    identity.className = 'sc-settings-about__identity';
    const title = document.createElement('h1');
    title.textContent = sugarCubesProject.name;
    const description = document.createElement('p');
    description.textContent = sugarCubesProject.description;
    identity.append(title, description);
    this.element.append(identity);
    this.#versions = document.createElement('div');
    this.#versions.className = 'sc-settings-about__versions';
    this.#group('Versions', this.#versions);
    this.#status = document.createElement('p');
    this.#status.className = 'sc-settings-about__status';
    this.#status.setAttribute('role', 'status');
    this.#retry = document.createElement('button');
    this.#retry.type = 'button';
    this.#retry.className = 'p-button p-button-secondary';
    this.#retry.textContent = 'Refresh versions';
    this.#retry.addEventListener('click', () => this.refresh(true));
    this.element.append(this.#status, this.#retry);
    const project = this.#card('SugarCubes', sugarCubesProject.summary);
    const links = document.createElement('div');
    links.className = 'sc-settings-about__actions';
    links.append(
      this.#link('Source code', sugarCubesProject.url),
      this.#link('Report an issue', `${sugarCubesProject.url}/issues`),
    );
    project.append(links);
    this.#group('Project', project);
    const license = this.#card(
      'GNU Affero General Public License v3 or later',
      'Free software: you can use, study, share and modify SugarCubes under the terms of the AGPL.',
    );
    const licenseActions = document.createElement('div');
    licenseActions.className = 'sc-settings-about__actions';
    const viewLicense = document.createElement('button');
    viewLicense.type = 'button';
    viewLicense.className = 'p-button p-button-secondary';
    viewLicense.textContent = 'View license';
    viewLicense.addEventListener('click', () => {
      viewLicense.disabled = true;
      void this.source
        .license()
        .then((text) => {
          if (this.#disposed || !this.element.isConnected) return;
          const body = document.createElement('pre');
          body.style.cssText =
            'white-space:pre-wrap;max-width:70ch;max-height:min(520px,max(80px,calc(80vh - 184px)));overflow:auto;margin:0;font:12px/1.6 monospace';
          body.tabIndex = 0;
          body.setAttribute('aria-label', 'License text');
          body.textContent = text;
          void this.#license.open({
            title: 'GNU Affero General Public License',
            body,
            showCancel: false,
            confirmLabel: 'Close',
            initialFocus: () => body,
            onConfirm: () => this.#license.close(null),
          });
        })
        .catch(this.report)
        .finally(() => {
          viewLicense.disabled = false;
        });
    });
    licenseActions.append(viewLicense, this.#link('Third-party notices', this.source.noticesUrl));
    license.append(licenseActions);
    this.#group('License', license);
    this.#group(
      'Acknowledgements',
      this.#card(
        'Contributors and community',
        'Thank you to SugarCubes contributors, ComfyUI custom-node authors, and the people testing and supporting the project.',
      ),
      this.#card(
        'Special thanks',
        'Comfy Org, Adobe Spectrum, Microsoft Fluent UI, and zhiyiYo’s Fluent Widgets for the foundations and design references used in this workspace.',
      ),
    );
  }

  /** Load lazily once per page lifetime and support explicit retry without rebuilding controls. */
  refresh(force = false): void {
    if (this.#disposed || this.#loading || (this.#loaded && !force)) return;
    this.#loading = true;
    this.#retry.disabled = true;
    this.#status.textContent = 'Reading installed versions…';
    void this.source
      .load()
      .then((info) => {
        if (this.#disposed) return;
        this.#loaded = true;
        this.#versions.replaceChildren(
          ...info.components.map((component) => {
            const card = this.#card(
              component.name,
              `${component.description} · ${component.author}`,
            );
            const version = this.document.createElement('span');
            version.textContent = component.version;
            card.append(version, this.#link('Source', component.url));
            return card;
          }),
        );
        this.#status.textContent = info.warnings.join('\n');
      })
      .catch((error: unknown) => {
        this.#status.textContent = 'Version information is unavailable. Refresh to try again.';
        this.report(error);
      })
      .finally(() => {
        this.#loading = false;
        this.#retry.disabled = false;
      });
  }

  /** Release a license modal when its owning page lifetime ends. */
  dispose(): void {
    this.#disposed = true;
    this.#license.close(null);
  }

  /** Keep section hierarchy legible in both native Comfy and Substitute presentations. */
  #group(title: string, ...content: HTMLElement[]): void {
    const heading = this.document.createElement('h2');
    heading.textContent = title;
    this.element.append(heading, ...content);
  }

  /** Render metadata as text, including values originating from runtime adapters. */
  #card(title: string, description: string): HTMLElement {
    const card = this.document.createElement('article');
    card.className = 'sc-settings-about__card';
    const body = this.document.createElement('div');
    const name = this.document.createElement('strong');
    name.textContent = title;
    const detail = this.document.createElement('small');
    detail.textContent = description;
    body.append(name, detail);
    card.append(body);
    return card;
  }

  /** Open trusted project resources without replacing the current workflow. */
  #link(label: string, url: string): HTMLAnchorElement {
    const link = this.document.createElement('a');
    link.textContent = label;
    const parsed = new URL(url, this.document.baseURI);
    if (parsed.protocol !== 'https:' && parsed.origin !== this.document.location.origin)
      throw new Error('About links must use HTTPS or the installed same-origin resources.');
    link.href = parsed.href;
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    return link;
  }
}
