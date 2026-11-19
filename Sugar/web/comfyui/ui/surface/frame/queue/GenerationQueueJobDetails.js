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
/** Say everything ComfyUI knows about one job, as the lines of its details dialog. */
import { formatDuration } from './GenerationQueueRows.js';
/**
 * Describe one job in full: what it is, when it was queued, and where it stands or how it ended.
 *
 * These are the facts ComfyUI's own job details show. A waiting job's estimate is the average of
 * the jobs that have finished, times the jobs ahead of it, and is left out until one has finished.
 */
export function jobDetailLines(job, jobs, clock) {
    const lines = [];
    if (job.workflowName)
        lines.push(`Workflow: ${job.workflowName}`);
    lines.push(`Job ID: ${job.id}`);
    if (job.createTime !== null)
        lines.push(`Queued at: ${clock.date(job.createTime)} ${clock.time(job.createTime)}`);
    if (job.status === 'pending') {
        const ahead = jobs.filter((other) => other.status === 'running' || (other.status === 'pending' && other.priority < job.priority)).length;
        lines.push(`Queue position: ${String(ahead + 1)}`);
        const average = averageDuration(jobs);
        if (average !== null)
            lines.push(`Estimated to start in: ${formatDuration(average * ahead)}`);
    }
    const elapsed = duration(job, clock);
    if (job.status === 'running' && elapsed !== null)
        lines.push(`Time elapsed: ${formatDuration(elapsed)}`);
    if (job.status === 'completed') {
        if (job.endTime !== null)
            lines.push(`Generated on: ${clock.date(job.endTime)} ${clock.time(job.endTime)}`);
        if (elapsed !== null)
            lines.push(`Total generation time: ${formatDuration(elapsed)}`);
        if (job.outputFile)
            lines.push(`Output: ${job.outputFile}`);
    }
    if (job.status === 'failed' && elapsed !== null)
        lines.push(`Failed after: ${formatDuration(elapsed)}`);
    if (job.status === 'cancelled' && elapsed !== null)
        lines.push(`Cancelled after: ${formatDuration(elapsed)}`);
    if (job.error !== null)
        lines.push(`Error message: ${job.error}`);
    return lines;
}
/** How long a job has run, or ran. */
function duration(job, clock) {
    if (job.startTime === null)
        return null;
    if (job.status === 'running')
        return clock.now - job.startTime;
    return job.endTime === null ? null : job.endTime - job.startTime;
}
/** The mean generation time of the jobs that completed. */
function averageDuration(jobs) {
    const spans = jobs.flatMap((job) => job.status === 'completed' && job.startTime !== null && job.endTime !== null
        ? [job.endTime - job.startTime]
        : []);
    return spans.length ? spans.reduce((total, span) => total + span, 0) / spans.length : null;
}
