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
 * Replay ComfyUI queue events for several workflow tabs through the real execution and canvas
 * owners, and check after every step what each tab's output canvas would show.
 *
 * The story keeps its own plain record of what ComfyUI did (which job of which tab is waiting,
 * running or finished, and what each produced) and compares the owners' published state with it.
 * A difference is recorded as a violation naming the step, so one scenario reports every step
 * that went wrong rather than stopping at the first.
 */

import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import { WorkflowExecutionIngestor } from '../../../frontend/comfyui/ui/execution/application/WorkflowExecutionIngestor.js';
import {
  outputSourceId,
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowSubmissionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSubmission.js';
import type { WorkflowOrderedSubmission } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSubmission.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';
import { canvasSceneCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import {
  canvasSessionId,
  type CanvasViewId,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

type Terminal = 'complete' | 'cancelled' | 'error';

interface Job {
  readonly id: string;
  readonly tab: string;
  submission: WorkflowOrderedSubmission | null;
  state: 'submitted' | 'waiting' | 'running' | Terminal | 'removed';
  readonly finals: Set<string>;
  preview: string | null;
}

interface Tab {
  readonly context: WorkflowSubmissionContext;
  readonly view: CanvasViewId;
  readonly jobs: Job[];
  /** The newest started job that has produced a preview or a final. */
  shown: Job | null;
  count: number;
}

/** Drive real owners with ComfyUI's events and record every step where a canvas would be wrong. */
export class CanvasQueueStory {
  readonly violations: string[] = [];
  readonly results = new WorkflowResultStore();
  readonly runs = new WorkflowRunRegistry();
  readonly views: CanvasViewRegistry;
  readonly selection: CanvasSelectionController;
  readonly #ingestor: WorkflowExecutionIngestor;
  readonly #tabs = new Map<string, Tab>();
  readonly #jobs = new Map<string, Job>();
  readonly #stages: readonly string[];
  /** Jobs in the order ComfyUI started them. */
  readonly #startOrder: Job[] = [];
  #sequence = 0;
  #step = '';

  /** Open one workflow tab per name, each with the same output stages in order. */
  constructor(tabs: readonly string[], stages: readonly string[] = ['base', 'upscale']) {
    this.#stages = stages;
    let key = 0;
    this.views = new CanvasViewRegistry(() => String(++key));
    this.selection = new CanvasSelectionController(this.views, this.runs);
    this.#ingestor = new WorkflowExecutionIngestor({
      runs: this.runs,
      results: this.results,
      clock: new WorkflowTestClock(),
      logger: { warn: () => {} },
      recover: (id) => this.#ingestor.recovered(id, 'complete'),
    });
    const contexts = tabs.map((name) => ({
      session: { sessionId: workflowSessionId(name), title: name, path: '', generation: 0 },
      sources: stages.map((stage, order) => ({
        id: outputSourceId(stage),
        executionPath: stage,
        label: stage,
        breadcrumb: [],
        order,
      })),
    }));
    this.results.synchronize(contexts.map((context) => context.session));
    for (const context of contexts)
      this.#tabs.set(context.session.title, {
        context,
        view: this.views.create(canvasSessionId(context.session.sessionId)).viewId,
        jobs: [],
        shown: null,
        count: 0,
      });
    // Every tab's view follows its results whether or not it is the tab on screen.
    this.results.subscribe((sessionId) => {
      for (const tab of this.#tabs.values())
        if (tab.context.session.sessionId === sessionId) this.#reconcile(tab);
    });
  }

  /** Press Queue in a tab: the prompt is submitted and ComfyUI's acceptance arrives. */
  queue(tab: string): string {
    const id = this.submit(tab);
    this.accept(id);
    return id;
  }

  /** Submit a prompt whose acceptance has not arrived yet. */
  submit(tab: string): string {
    const owner = this.#tab(tab);
    const id = `${tab}${++owner.count}`;
    const job: Job = {
      id,
      tab,
      submission: this.#ingestor.submitting(owner.context),
      state: 'submitted',
      finals: new Set(),
      preview: null,
    };
    owner.jobs.push(job);
    this.#jobs.set(id, job);
    this.#check(`submit ${id}`);
    return id;
  }

  /** Deliver ComfyUI's queue acceptance for a submitted prompt. */
  accept(id: string): void {
    const job = this.#job(id);
    if (!job.submission) throw new Error(`${id} was already accepted.`);
    const early = job.state;
    this.#ingestor.accepted(job.submission, promptId(id));
    job.submission = null;
    if (early === 'submitted') job.state = 'waiting';
    this.#check(`accept ${id}`);
  }

  /** ComfyUI starts executing a job. */
  start(id: string): void {
    const job = this.#job(id);
    job.state = 'running';
    this.#ingestor.started(promptId(id));
    this.#check(`start ${id}`);
  }

  /** A live preview frame of one stage of the running job. */
  preview(id: string, stage: string): void {
    const job = this.#job(id);
    const tab = this.#tab(job.tab);
    const expected = job.state === 'running' && !job.finals.has(stage) && !job.submission;
    const attributed = this.#ingestor.resolveSource(promptId(id), [stage]);
    const admitted =
      !!attributed &&
      this.results.publishPreview({
        sessionId: attributed.run.session.sessionId,
        generation: attributed.run.session.generation,
        promptId: attributed.run.promptId,
        sourceId: attributed.source.id,
        runOrder: attributed.run.order,
        sequence: ++this.#sequence,
        token: `preview:${this.#sequence}`,
        dimensions: null,
      });
    this.#step = `preview ${id} ${stage}`;
    if (expected && !admitted) this.#violation('preview of the running job was refused');
    if (!expected && admitted) this.#violation('a preview that should not show was admitted');
    if (expected) {
      job.preview = stage;
      tab.shown = this.#newestWithContent(tab);
    }
    this.#check(this.#step, expected ? stage : null);
  }

  /** The finished image of one stage of a job. */
  final(id: string, stage: string): void {
    const job = this.#job(id);
    const tab = this.#tab(job.tab);
    const media = decodeComfyMedia({
      images: [{ filename: `${id}-${stage}.png`, type: 'output' }],
    });
    if (!media) throw new Error('Invalid story media.');
    this.#ingestor.final({
      promptId: promptId(id),
      executionPaths: [stage],
      ...media,
      merge: false,
    });
    if (!job.submission) {
      job.finals.add(stage);
      if (job.preview === stage) job.preview = null;
      tab.shown = this.#newestWithContent(tab);
    }
    this.#check(`final ${id} ${stage}`, job.submission ? null : stage);
  }

  /** The job ends: complete, stopped by the person, or failed. */
  finish(id: string, status: Terminal = 'complete'): void {
    const job = this.#job(id);
    job.state = status;
    if (status === 'complete') job.preview = null;
    this.#ingestor.terminal(promptId(id), status);
    this.#check(`${status} ${id}`);
  }

  /** The person deletes a waiting job from ComfyUI's queue: no execution event ever names it. */
  remove(id: string): void {
    this.#job(id).state = 'removed';
    // The queue reconciler, proven with the event adapter, ends a job ComfyUI no longer holds.
    this.#ingestor.terminal(promptId(id), 'cancelled');
    this.#check(`remove ${id} from the queue`);
  }

  /** The connection to ComfyUI drops: live previews are gone until the next frame. */
  disconnect(): void {
    this.#ingestor.disconnected();
    for (const job of this.#jobs.values()) job.preview = null;
    for (const tab of this.#tabs.values()) tab.shown = this.#newestWithContent(tab);
    this.#check('disconnect');
  }

  /** Run every stage of a job from start to finish. */
  run(id: string): void {
    this.start(id);
    for (const stage of this.#stages) {
      this.preview(id, stage);
      this.final(id, stage);
    }
    this.finish(id);
  }

  /** Click one of a tab's canvas tabs. */
  click(tab: string, stage: string): void {
    const owner = this.#tab(tab);
    const listed = this.#catalog(owner).entries.some((entry) => entry.source.label === stage);
    this.selection.apply(owner.view, this.#catalog(owner), {
      type: 'select-source',
      selection: { sourceId: outputSourceId(stage), batchIndex: 0 },
    });
    this.#step = `click ${tab} ${stage}`;
    if (listed && this.views.read(owner.view).selection?.sourceId !== outputSourceId(stage))
      this.#violation('a click on a listed canvas tab did not select it');
    this.#check(this.#step);
  }

  /** What one tab's canvas shows now: a final's file name, `preview:<stage>`, or null. */
  shows(tab: string): string | null {
    const owner = this.#tab(tab);
    const catalog = this.#catalog(owner);
    const item = this.selection.items(owner.view, catalog).primary;
    if (item) return item.file.filename;
    const preview = this.selection.preview(owner.view, catalog);
    return preview ? `preview:${preview.sourceId.replace(/^source:/, '')}` : null;
  }

  /** The canvas tabs one workflow tab lists, in order. */
  tabs(tab: string): readonly string[] {
    return this.#catalog(this.#tab(tab)).entries.map((entry) => entry.source.label);
  }

  /** The run status one workflow tab reports. */
  status(tab: string): string {
    return this.#result(this.#tab(tab)).runStatus;
  }

  #tab(name: string): Tab {
    const tab = this.#tabs.get(name);
    if (!tab) throw new Error(`Unknown story tab ${name}.`);
    return tab;
  }

  #job(id: string): Job {
    const job = this.#jobs.get(id);
    if (!job) throw new Error(`Unknown story job ${id}.`);
    return job;
  }

  #result(tab: Tab) {
    const result = this.results.read(tab.context.session.sessionId);
    if (!result) throw new Error('Story tab has no result session.');
    return result;
  }

  #catalog(tab: Tab) {
    return canvasSceneCatalog(
      this.#result(tab),
      tab.context.sources,
      this.selection.inspection(tab.view),
      this.views.read(tab.view).scene,
    );
  }

  #reconcile(tab: Tab): void {
    this.selection.reconcile(tab.view, this.#catalog(tab));
  }

  /** The newest job, in the order ComfyUI started them, that has something to show. */
  #newestWithContent(tab: Tab): Job | null {
    const started = this.#startOrder.filter((job) => job.tab === tab.context.session.title);
    for (let index = started.length - 1; index >= 0; index--) {
      const job = started[index];
      if (job && (job.finals.size > 0 || job.preview !== null)) return job;
    }
    return null;
  }

  #violation(message: string): void {
    this.violations.push(`${this.#step}: ${message}`);
  }

  /** Compare every tab's published canvas state with what ComfyUI did. */
  #check(step: string, content: string | null = null): void {
    this.#step = step;
    const starting = /^start (\S+)$/.exec(step)?.[1];
    if (starting) this.#startOrder.push(this.#job(starting));
    for (const [name, tab] of this.#tabs) {
      this.#reconcile(tab);
      const result = this.#result(tab);
      const catalog = this.#catalog(tab);
      const state = this.views.read(tab.view);
      const running = tab.jobs.find((job) => job.state === 'running' && !job.submission);
      const at = (message: string) => this.#violation(`[${name}] ${message}`);
      if (running && result.runStatus !== 'running')
        at(`${running.id} is running but the tab reports ${result.runStatus}`);
      if (!running && result.runStatus === 'running')
        at('nothing of this tab is running but the tab reports running');
      for (const entry of catalog.entries) {
        for (const item of entry.items)
          if (item.provenance.sessionId !== tab.context.session.sessionId)
            at(`${entry.source.label} lists an image of another workflow tab`);
        if (entry.preview && entry.preview.sessionId !== tab.context.session.sessionId)
          at(`${entry.source.label} lists a preview of another workflow tab`);
      }
      const shown = tab.shown;
      if (shown) {
        const expected = this.#stages.filter(
          (stage) => shown.finals.has(stage) || shown.preview === stage,
        );
        const listed = catalog.entries.map((entry) => entry.source.label);
        if (JSON.stringify(listed) !== JSON.stringify(expected))
          at(`canvas tabs are [${listed.join(', ')}], ${shown.id} has [${expected.join(', ')}]`);
        for (const entry of catalog.entries)
          for (const item of entry.items)
            if (!item.file.filename.startsWith(`${shown.id}-`))
              at(`${entry.source.label} lists ${item.file.filename}, not an image of ${shown.id}`);
      }
      if (catalog.entries.length) {
        const selected = state.selection?.sourceId;
        if (!catalog.entries.some((entry) => entry.source.id === selected))
          at(`the selected canvas tab ${selected ?? 'none'} is not in the list`);
        else if (this.shows(name) === null) at('the selected canvas tab shows nothing');
      }
      const owner = /^(?:preview|final) (\S+) /.exec(step)?.[1];
      if (
        content &&
        owner &&
        this.#job(owner).tab === name &&
        shown === this.#job(owner) &&
        state.follow === 'auto' &&
        state.selection?.sourceId !== outputSourceId(content)
      )
        at(`following the run, but ${content} was not selected when it arrived`);
    }
  }
}
