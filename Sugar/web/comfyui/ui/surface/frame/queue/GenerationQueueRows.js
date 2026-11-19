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
/** Turn ComfyUI's jobs into the generation queue's rows: their order, titles and status lines. */
/** Nothing reported yet. */
export const NO_QUEUE_PROGRESS = {
    percent: null,
    node: null,
    nodePercent: null,
    previewUrl: null,
};
/** The view a queue opens with: everything, newest first. */
export const DEFAULT_QUEUE_VIEW = {
    tab: 'all',
    workflow: 'all',
    currentWorkflowId: null,
    sort: 'recent',
    search: '',
};
const FAILURE_SUMMARY_LIMIT = 64;
/** Count the jobs still waiting to run, which is what the queue button's badge shows. */
export function pendingJobCount(jobs) {
    return jobs.filter((job) => job.status === 'pending').length;
}
/** Whether any job failed or was cancelled, which is when ComfyUI offers its Failed tab. */
export function hasFailedJobs(jobs) {
    return jobs.some((job) => job.status === 'failed' || job.status === 'cancelled');
}
/**
 * Lay the jobs out as the generation queue always has: waiting jobs with the newest on top and the
 * next to run at the bottom, then the running job, then finished jobs with the newest first, under
 * the day they finished.
 */
export function queueItems(jobs, progress = NO_QUEUE_PROGRESS, view = DEFAULT_QUEUE_VIEW, clock = localClock()) {
    const shown = jobs.filter((job) => matches(job, view));
    const waiting = shown
        .filter((job) => job.status === 'pending')
        .sort((a, b) => b.priority - a.priority);
    const active = shown.filter((job) => job.status === 'running');
    const resolved = shown
        .filter((job) => job.status !== 'pending' && job.status !== 'running')
        .sort((a, b) => view.sort === 'duration'
        ? (duration(b) ?? -1) - (duration(a) ?? -1) || recency(b) - recency(a)
        : recency(b) - recency(a) || b.id.localeCompare(a.id));
    // A waiting job's place is counted among all waiting jobs, not only the ones a filter left.
    const order = jobs
        .filter((job) => job.status === 'pending')
        .sort((a, b) => a.priority - b.priority)
        .map((job) => job.id);
    const items = [];
    for (const job of waiting)
        items.push(row(job, 'waiting', waitingLine(order.indexOf(job.id)), job.createTime === null ? '' : clock.time(job.createTime), job.previewUrl));
    for (const job of active)
        items.push(row(job, 'active', progress.percent === null ? 'Running' : `${String(progress.percent)}% complete`, progress.node === null
            ? ''
            : progress.nodePercent === null
                ? progress.node
                : `${progress.node}: ${String(progress.nodePercent)}%`, progress.previewUrl ?? job.previewUrl));
    // Finished jobs sit under the day they ended. One day with nothing above it needs no heading.
    const days = resolved.map((job) => dayLabel(recencyOrNull(job), clock));
    const headed = waiting.length + active.length > 0 || new Set(days).size > 1;
    resolved.forEach((job, index) => {
        if (headed && view.sort === 'recent' && days[index] !== days[index - 1])
            items.push({
                kind: 'caption',
                text: index === 0 && active.length === 0 && waiting.length > 0
                    ? `Resolved - ${days[index] ?? ''}`
                    : (days[index] ?? ''),
            });
        // A job named after its file already says the file in its title.
        const file = job.outputFile && title(job) !== job.outputFile.trim() ? job.outputFile : '';
        items.push(row(job, 'resolved', resolvedLine(job), file, job.previewUrl));
    });
    return items;
}
/** Summarize what is running and waiting, as ComfyUI does above its own list. */
export function queueSummary(jobs) {
    const running = jobs.filter((job) => job.status === 'running').length;
    const queued = pendingJobCount(jobs);
    return [
        running > 0 ? `${String(running)} running` : '',
        queued > 0 ? `${String(queued)} queued` : '',
    ]
        .filter(Boolean)
        .join(', ');
}
/** Write a duration the way the queue does: 12.3s, 4m5s, 1h2m. */
export function formatDuration(milliseconds) {
    const seconds = Math.max(0, milliseconds) / 1000;
    if (seconds < 60)
        return `${seconds.toFixed(1)}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60)
        return `${String(minutes)}m${String(Math.floor(seconds % 60))}s`;
    return `${String(Math.floor(minutes / 60))}h${String(minutes % 60)}m`;
}
function row(job, role, subtitle, detail, previewUrl) {
    return {
        kind: 'row',
        row: {
            jobId: job.id,
            role,
            status: job.status,
            title: title(job),
            subtitle,
            detail,
            previewUrl,
            video: job.outputKind === 'video',
            action: role === 'resolved' ? 'remove' : 'cancel',
        },
    };
}
/** Name a job by its workflow and run number, falling back to what ComfyUI itself would show. */
function title(job) {
    const name = job.workflowName?.trim();
    if (name)
        return job.runNumber === null ? name : `${name} #${String(job.runNumber).padStart(3, '0')}`;
    return job.outputFile?.trim() || `Job ${job.id.slice(0, 8)}`;
}
function waitingLine(ahead) {
    return ahead <= 0 ? 'Next' : `Waiting - ${String(ahead)} ahead`;
}
function resolvedLine(job) {
    if (job.status === 'completed') {
        const took = duration(job);
        return took === null ? 'Completed' : `Completed, ${formatDuration(took)}`;
    }
    if (job.status === 'cancelled') {
        if (job.outputsCount <= 0)
            return 'Cancelled - No outputs saved';
        const outputs = job.outputsCount === 1 ? '1 output' : `${String(job.outputsCount)} outputs`;
        return `Cancelled - ${outputs} saved`;
    }
    const summary = job.error?.replace(/\s+/g, ' ').trim();
    if (!summary)
        return 'Generation failed';
    return `Failed - ${summary.length > FAILURE_SUMMARY_LIMIT
        ? `${summary.slice(0, FAILURE_SUMMARY_LIMIT)}...`
        : summary}`;
}
function duration(job) {
    return job.startTime !== null && job.endTime !== null && job.endTime >= job.startTime
        ? job.endTime - job.startTime
        : null;
}
function recencyOrNull(job) {
    return job.endTime ?? job.createTime;
}
/** Name a moment's day the way ComfyUI groups its list: today, yesterday, or the date. */
function dayLabel(moment, clock) {
    if (moment === null)
        return 'Undated';
    const start = (value) => {
        const day = new Date(value);
        day.setHours(0, 0, 0, 0);
        return day.getTime();
    };
    const days = Math.round((start(clock.now) - start(moment)) / 86_400_000);
    return days === 0 ? 'Today' : days === 1 ? 'Yesterday' : clock.date(moment);
}
/** The clock and wording of the machine the page runs on. */
export function localClock() {
    return {
        now: Date.now(),
        time: (moment) => new Date(moment).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }),
        date: (moment) => new Date(moment).toLocaleDateString(),
    };
}
function recency(job) {
    return job.endTime ?? job.createTime ?? 0;
}
function matches(job, view) {
    if (view.tab === 'completed' && job.status !== 'completed')
        return false;
    if (view.tab === 'failed' && job.status !== 'failed' && job.status !== 'cancelled')
        return false;
    if (view.workflow === 'current' && job.workflowId !== view.currentWorkflowId)
        return false;
    const search = view.search.trim().toLocaleLowerCase();
    if (!search)
        return true;
    return [title(job), job.outputFile ?? '', job.id].some((text) => text.toLocaleLowerCase().includes(search));
}
