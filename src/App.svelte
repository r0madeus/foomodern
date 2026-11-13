<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import Icon from './lib/Icon.svelte'
  import PlayerBar from './lib/PlayerBar.svelte'
  import QueueList from './lib/QueueList.svelte'
  import { player } from './lib/player.svelte'

  const AUDIO_EXTENSIONS = [
    'mp3', 'flac', 'ogg', 'oga', 'opus', 'm4a', 'm4b', 'aac', 'wav', 'wv', 'ape',
    'wma', 'aif', 'aiff', 'dsf', 'dff', 'mka', 'mpc', 'tta', 'alac',
  ]

  onMount(() => {
    player.init().catch((e) => (player.error = String(e)))
  })

  async function pickFiles(append: boolean) {
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [{ name: 'Audio', extensions: AUDIO_EXTENSIONS }],
    })
    if (!picked || picked.length === 0) return
    if (append) await player.appendFiles(picked)
    else await player.openFiles(picked)
  }
</script>

<div class="flex h-screen flex-col bg-zinc-50 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
  <header class="flex items-center justify-between gap-4 px-5 py-3">
    <h1 class="text-lg font-semibold tracking-tight">FooModern</h1>
    <div class="flex gap-2">
      {#if player.snapshot.queueLength > 0}
        <button
          class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
          onclick={() => pickFiles(true)}
        >
          Add to queue
        </button>
      {/if}
      <button
        class="flex items-center gap-2 rounded-lg bg-zinc-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white"
        onclick={() => pickFiles(false)}
      >
        <Icon name="folder" class="size-4" /> Open files
      </button>
    </div>
  </header>

  {#if player.error}
    <p class="mx-5 mb-2 rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300">
      {player.error}
    </p>
  {/if}

  <main class="min-h-0 flex-1 overflow-y-auto">
    {#if player.snapshot.queueLength === 0}
      <div class="flex h-full flex-col items-center justify-center gap-3 text-center text-zinc-500">
        <p>The queue is empty.</p>
        <button class="text-sm font-medium text-teal-700 hover:underline dark:text-teal-400" onclick={() => pickFiles(false)}>
          Open audio files to start
        </button>
      </div>
    {:else}
      <QueueList />
    {/if}
  </main>

  <PlayerBar />
</div>
