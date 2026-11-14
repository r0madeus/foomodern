<script lang="ts">
  import Icon from './Icon.svelte'
  import { count, t } from './i18n.svelte'
  import { downloadProgress, online, type DownloadJob } from './online.svelte'

  let { collapsed, onExpand }: { collapsed: boolean; onExpand: () => void } = $props()

  function isActive(job: DownloadJob): boolean {
    return job.state === 'queued' || job.state === 'downloading'
  }
</script>

{#if online.downloads.length > 0}
  {#if collapsed}
    <button
      type="button"
      class="relative flex items-center justify-center rounded-lg px-2 py-1.5 text-zinc-600 transition-colors hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800/70"
      title={online.activeDownloadCount > 0
        ? `${t('Downloads')} · ${count(online.activeDownloadCount, '{n} active', '{n} active')}`
        : t('Downloads')}
      aria-label={t('Downloads')}
      onclick={onExpand}
    >
      <Icon
        name="download"
        class={online.activeDownloadCount > 0
          ? 'size-[18px] animate-pulse text-accent-600 dark:text-accent-400'
          : 'size-[18px] opacity-85'}
      />
      {#if online.activeDownloadCount > 0}
        <span class="absolute top-0.5 right-1 flex size-3.5 items-center justify-center rounded-full bg-accent-600 text-[9px] font-semibold text-white">
          {online.activeDownloadCount}
        </span>
      {/if}
    </button>
  {:else}
    <section
      aria-label={t('Downloads')}
      class="flex flex-col gap-2 rounded-xl border border-zinc-200/80 bg-white/80 p-2.5 shadow-xs dark:border-zinc-800 dark:bg-zinc-900/80"
    >
      <div class="flex items-center justify-between gap-1.5">
        <div class="flex min-w-0 items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-zinc-500 dark:text-zinc-400">
          <Icon
            name="download"
            class={online.activeDownloadCount > 0
              ? 'size-3.5 shrink-0 animate-pulse text-accent-600 dark:text-accent-400'
              : 'size-3.5 shrink-0'}
          />
          <span class="truncate">{t('Downloads')}</span>
          {#if online.activeDownloadCount > 0}
            <span class="shrink-0 rounded-full bg-accent-500/15 px-1.5 py-0.5 text-[10px] font-medium normal-case text-accent-700 dark:text-accent-300">
              {online.activeDownloadCount}
            </span>
          {/if}
        </div>
        {#if online.hasFinishedDownloads}
          <button
            type="button"
            class="shrink-0 rounded px-1 py-0.5 text-[11px] text-zinc-400 transition hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
            title={t('Clear finished')}
            aria-label={t('Clear finished')}
            onclick={() => void online.clearFinished()}
          >
            {t('Clear finished')}
          </button>
        {/if}
      </div>

      <ul class="flex max-h-36 flex-col gap-1.5 overflow-y-auto pr-0.5">
        {#each online.downloads as job (job.id)}
          {@const active = isActive(job)}
          <li class="flex flex-col gap-1 rounded-lg bg-zinc-100/70 px-2 py-1.5 text-xs dark:bg-zinc-950/60">
            <div class="flex items-center justify-between gap-1.5">
              <div class="min-w-0 flex-1">
                <p class="truncate text-xs font-medium leading-tight">{job.title}</p>
                {#if job.state === 'downloading' && job.currentTrack && job.currentTrack !== job.title}
                  <p class="truncate text-[11px] text-zinc-500 dark:text-zinc-400">{job.currentTrack}</p>
                {:else if job.state === 'failed' && job.error}
                  <p class="truncate text-[11px] text-red-500" title={job.error}>{job.error}</p>
                {/if}
              </div>
              <div class="flex shrink-0 items-center gap-1">
                <span
                  class={[
                    'text-[11px] font-medium tabular-nums',
                    job.state === 'done'
                      ? 'text-accent-600 dark:text-accent-400'
                      : job.state === 'failed'
                        ? 'text-red-500'
                        : 'text-zinc-500 dark:text-zinc-400',
                  ]}
                >
                  {#if job.state === 'done'}
                    {t('Done')}
                  {:else if job.state === 'failed'}
                    {t('Failed')}
                  {:else if job.state === 'cancelled'}
                    {t('Cancelled')}
                  {:else if job.state === 'queued'}
                    {t('Queued')}
                  {:else}
                    {job.doneTracks}/{job.totalTracks}
                  {/if}
                </span>
                {#if active}
                  <button
                    type="button"
                    class="rounded-full p-0.5 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
                    title={t('Cancel download')}
                    aria-label={t('Cancel download')}
                    onclick={() => void online.cancelDownload(job.id)}
                  >
                    <Icon name="close" class="size-3" />
                  </button>
                {/if}
              </div>
            </div>

            {#if active}
              <div class="h-1 w-full overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
                {#if job.state === 'downloading' && job.doneTracks === 0}
                  <div class="h-full w-2/5 animate-pulse rounded-full bg-accent-500"></div>
                {:else}
                  <div
                    class="h-full w-full origin-left bg-accent-500 transition-transform duration-300 ease-out"
                    style:transform="scaleX({downloadProgress(job) / 100})"
                  ></div>
                {/if}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}
{/if}
