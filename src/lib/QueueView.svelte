<script lang="ts">
  import QueueList from './QueueList.svelte'
  import { player } from './player.svelte'
  import { playlists } from './playlists.svelte'

  let { onOpenFiles, onSaved }: { onOpenFiles: (append: boolean) => void; onSaved?: (id: number) => void } = $props()

  let saving = $state(false)
  let name = $state('')

  async function save() {
    const trimmed = name.trim()
    if (trimmed === '') return
    const made = await playlists.saveQueue(trimmed)
    if (!made) return
    saving = false
    name = ''
    onSaved?.(made.id)
  }
</script>

<div class="flex h-full flex-col">
  <div class="flex items-center justify-between gap-3 px-5 pb-2">
    <h2 class="text-lg font-semibold tracking-tight">Queue</h2>
    <div class="flex gap-2">
      {#if player.snapshot.queueLength > 0}
        {#if saving}
          <form
            class="flex items-center gap-2"
            onsubmit={(e) => {
              e.preventDefault()
              void save()
            }}
          >
            <input
              class="w-48 rounded-md border border-zinc-200 bg-white px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500 dark:border-zinc-700 dark:bg-zinc-900"
              placeholder="Playlist name"
              aria-label="Playlist name"
              bind:value={name}
              onkeydown={(e) => e.key === 'Escape' && (saving = false)}
              {@attach (el) => el.focus()}
            />
            <button
              type="submit"
              class="rounded-lg bg-teal-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-teal-500 disabled:opacity-50"
              disabled={name.trim() === ''}
            >
              Save
            </button>
            <button
              type="button"
              class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
              onclick={() => (saving = false)}
            >
              Cancel
            </button>
          </form>
        {:else}
          <button
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
            onclick={() => (saving = true)}
          >
            Save as playlist
          </button>
        {/if}
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
