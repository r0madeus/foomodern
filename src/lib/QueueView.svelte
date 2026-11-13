<script lang="ts">
  import QueueList from './QueueList.svelte'
  import { player } from './player.svelte'

  let { onOpenFiles }: { onOpenFiles: (append: boolean) => void } = $props()
</script>

<div class="flex h-full flex-col">
  <div class="flex items-center justify-between gap-3 px-5 pb-2">
    <h2 class="text-lg font-semibold tracking-tight">Queue</h2>
    <div class="flex gap-2">
      {#if player.snapshot.queueLength > 0}
        <button
          class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
          onclick={() => onOpenFiles(true)}
        >
          Add to queue
        </button>
      {/if}
      <button
        class="rounded-lg bg-zinc-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white"
        onclick={() => onOpenFiles(false)}
      >
        Open files
      </button>
    </div>
  </div>
  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if player.snapshot.queueLength === 0}
      <div class="flex h-full items-center justify-center text-zinc-500">The queue is empty.</div>
    {:else}
      <QueueList />
    {/if}
  </div>
</div>
