<script lang="ts">
  import Icon from './Icon.svelte'
  import { player } from './player.svelte'

  // Plain list for now; the virtualised list arrives with the library view
  // (roadmap step 4).
  const current = $derived(player.snapshot.currentIndex)
  const status = $derived(player.snapshot.status)
</script>

<ol class="divide-y divide-zinc-100 dark:divide-zinc-800/70">
  {#each player.queue as track, i (track.id)}
    <li
      class={[
        'group flex h-11 items-center gap-3 px-5 text-sm',
        i === current
          ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
          : 'text-zinc-700 hover:bg-zinc-50 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
      ]}
      ondblclick={() => player.playIndex(i)}
    >
      <span class="w-8 shrink-0 text-right text-xs tabular-nums text-zinc-400">
        {#if i === current && status === 'playing'}
          <span class="text-teal-600 dark:text-teal-400">▶</span>
        {:else}
          {i + 1}
        {/if}
      </span>
      <button class="min-w-0 flex-1 truncate text-left" onclick={() => player.playIndex(i)} title={track.path}>
        <span class="font-medium">{track.title}</span>
        <span class="ml-2 text-xs text-zinc-400">{track.artist || track.path}</span>
      </button>
      <button
        class="invisible rounded p-1 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700 group-hover:visible dark:hover:bg-zinc-700 dark:hover:text-zinc-100"
        onclick={() => player.removeIndex(i)}
        aria-label="Remove from queue"
      >
        <Icon name="close" class="size-4" />
      </button>
    </li>
  {/each}
</ol>
