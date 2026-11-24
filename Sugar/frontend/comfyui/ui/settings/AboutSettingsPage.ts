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
/**
 * The About page as SugarSubstitute's Python application presents it: the product's logo, name
 * and description, the version cards, then Project, License, Supporters and Special thanks. In
 * the SugarSubstitute desktop the page speaks for SugarSubstitute; in a browser, for SugarCubes.
 */
import {
  sugarCubesProject,
  sugarSubstituteProject,
  type AboutComponent,
  type AboutInformationSource,
} from './AboutInformation.js';
import { createFluentIcon, type FluentIconName } from '../core/FluentIcons.js';
import { ModalShell, type ModalAdapter } from '../dialogs/ModalShell.js';
import type { AppearanceSource } from '../surface/appearance/AppearanceState.js';

/** What hosts the page: the SugarSubstitute desktop, with its running version when known. */
export interface AboutHost {
  read(): { readonly desktop: boolean; readonly version: string | null };
  subscribe(listener: () => void): () => void;
}

const BROWSER_HOST: AboutHost = {
  read: () => ({ desktop: false, version: null }),
  subscribe: () => () => undefined,
};

const LICENSE_PREAMBLE =
  'Free software: you can use, study, share and modify it under the terms of the GNU Affero General Public License, version 3 or later.';

const SUPPORTERS = [
  {
    title: 'Contributors and community',
    description:
      'SugarCubes contributors, ComfyUI custom-node authors, and the people testing and supporting the project.',
  },
];

const SPECIAL_THANKS = [
  'Comfy Org',
  'Adobe Spectrum',
  'Microsoft Fluent UI',
  'zhiyiYo’s Fluent Widgets',
].map((title) => ({ title, description: '' }));

/** Keep one stable page while version and license requests complete asynchronously. */
export class AboutSettingsPage {
  readonly element: HTMLElement;
  readonly #name: HTMLElement;
  readonly #description: HTMLElement;
  readonly #projectTitle: HTMLElement;
  readonly #projectDescription: HTMLElement;
  readonly #sourceLink: HTMLAnchorElement;
  readonly #issuesLink: HTMLAnchorElement;
  readonly #versions: HTMLElement;
  readonly #status: HTMLElement;
  readonly #retry: HTMLButtonElement;
  readonly #license: ModalShell;
  readonly #stopHost: () => void;
  #components: readonly AboutComponent[] = [];
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
    private readonly host: AboutHost = BROWSER_HOST,
  ) {
    this.element = document.createElement('section');
    this.element.className = 'sc-settings-about';
    this.#license = new ModalShell({
      adapter,
      dialogClassName: 'sc-settings-license',
      ...(appearance ? { appearanceSource: appearance } : {}),
    });

    const identity = document.createElement('header');
    identity.className = 'sc-settings-about__identity';
    const logo = document.createElement('img');
    logo.className = 'sc-settings-about__logo';
    logo.alt = '';
    logo.src = new URL('../../../orb/app_icon_256.png', import.meta.url).href;
    this.#name = document.createElement('strong');
    this.#description = document.createElement('p');
    identity.append(logo, this.#name, this.#description);
    this.element.append(identity);

    this.#versions = document.createElement('div');
    this.#versions.className = 'sc-settings-about__versions';
    this.#status = document.createElement('p');
    this.#status.className = 'sc-settings-about__status';
    this.#status.setAttribute('role', 'status');
    this.#retry = this.#button('Refresh versions', () => this.refresh(true));
    this.#retry.hidden = true;
    this.#section('Version information', this.#versions, this.#status, this.#retry);

    const project = this.#card('cube-filled', '', '');
    this.#projectTitle = project.querySelector('.sc-sw-card-title') ?? project;
    this.#projectDescription = project.querySelector('.sc-sw-card-description') ?? project;
    this.#sourceLink = this.#link('Source code', sugarCubesProject.url);
    this.#issuesLink = this.#link('Report an issue', `${sugarCubesProject.url}/issues`);
    this.#controls(project).append(this.#sourceLink, this.#issuesLink);
    this.#section('Project', project);

    const license = this.#card(
      'certificate',
      'GNU Affero General Public License v3',
      LICENSE_PREAMBLE,
    );
    license.dataset.stacked = '';
    const read = this.#button('Read AGPLv3', () => this.#openLicense(read));
    read.prepend(createFluentIcon(document, 'document-text', 16));
    this.#controls(license).append(this.#link('Third-party notices', this.source.noticesUrl), read);
    this.#section('License', license);

    this.#section(
      'Supporters',
      ...SUPPORTERS.map((entry) => this.#card('heart', entry.title, entry.description)),
    );
    this.#section(
      'Special thanks',
      ...SPECIAL_THANKS.map((entry) => this.#card('star', entry.title, entry.description)),
    );

    this.#showHost();
    this.#stopHost = host.subscribe(() => this.#showHost());
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
        this.#components = info.components;
        this.#showVersions();
        this.#status.textContent = info.warnings.join('\n');
        this.#retry.hidden = true;
      })
      .catch((error: unknown) => {
        this.#status.textContent = 'Version information is unavailable. Refresh to try again.';
        this.#retry.hidden = false;
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
    this.#stopHost();
    this.#license.close(null);
  }

  /** Speak for the product that hosts the page. */
  #showHost(): void {
    const product = this.host.read().desktop ? sugarSubstituteProject : sugarCubesProject;
    this.element.setAttribute('aria-label', `About ${product.name}`);
    this.#name.textContent = product.name;
    this.#description.textContent = product.description;
    this.#projectTitle.textContent = product.name;
    this.#projectDescription.textContent = product.summary;
    this.#sourceLink.href = product.url;
    this.#issuesLink.href = `${product.url}/issues`;
    if (this.#loaded) this.#showVersions();
  }

  /** The version cards: SugarSubstitute's own first when it hosts the page. */
  #showVersions(): void {
    const host = this.host.read();
    const components: AboutComponent[] = [
      ...(host.desktop && host.version
        ? [{ ...sugarSubstituteProject, version: host.version }]
        : []),
      ...this.#components,
    ];
    this.#versions.replaceChildren(...components.map((component) => this.#versionCard(component)));
  }

  /** One version card, opening its project's website on click as in the Python application. */
  #versionCard(component: AboutComponent): HTMLElement {
    const card = this.#link('', component.url);
    card.className = 'sc-settings-about__version';
    card.title = `Open ${component.name} project website`;
    const text = this.document.createElement('span');
    text.className = 'sc-settings-about__version-text';
    const name = this.document.createElement('strong');
    name.textContent = component.name;
    const description = this.document.createElement('small');
    description.textContent = component.description;
    text.append(name, description);
    const meta = this.document.createElement('span');
    meta.className = 'sc-settings-about__version-meta';
    const version = this.document.createElement('span');
    version.textContent = component.version;
    const author = this.document.createElement('small');
    author.textContent = `by ${component.author}`;
    meta.append(version, author);
    card.append(text, meta);
    return card;
  }

  #openLicense(button: HTMLButtonElement): void {
    button.disabled = true;
    void this.source
      .license()
      .then((text) => {
        if (this.#disposed || !this.element.isConnected) return;
        const body = this.document.createElement('pre');
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
        button.disabled = false;
      });
  }

  /** A titled group with Python's section rhythm. */
  #section(title: string, ...content: HTMLElement[]): void {
    const section = this.document.createElement('section');
    section.className = 'sc-settings-about__section';
    const heading = this.document.createElement('h2');
    heading.textContent = title;
    section.append(heading, ...content);
    this.element.append(section);
  }

  /** A setting card: icon, title and description, with a trailing control slot. */
  #card(icon: FluentIconName, title: string, description: string): HTMLElement {
    const card = this.document.createElement('article');
    card.className = 'sc-sw-card sc-settings-about__card';
    const head = this.document.createElement('div');
    head.className = 'sc-sw-card-head';
    const slot = this.document.createElement('span');
    slot.className = 'sc-sw-card-icon';
    slot.append(createFluentIcon(this.document, icon, 20));
    const text = this.document.createElement('div');
    text.className = 'sc-sw-card-text';
    const name = this.document.createElement('div');
    name.className = 'sc-sw-card-title';
    name.textContent = title;
    const detail = this.document.createElement('div');
    detail.className = 'sc-sw-card-description';
    detail.textContent = description;
    text.append(name, detail);
    const controls = this.document.createElement('div');
    controls.className = 'sc-sw-card-controls';
    controls.hidden = true;
    head.append(slot, text, controls);
    card.append(head);
    return card;
  }

  #controls(card: HTMLElement): HTMLElement {
    const controls = card.querySelector<HTMLElement>('.sc-sw-card-controls');
    if (!controls) throw new Error('About card has no control slot.');
    controls.hidden = false;
    return controls;
  }

  #button(label: string, run: () => void): HTMLButtonElement {
    const button = this.document.createElement('button');
    button.type = 'button';
    button.className = 'sc-sw-button';
    button.textContent = label;
    button.addEventListener('click', run);
    return button;
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
