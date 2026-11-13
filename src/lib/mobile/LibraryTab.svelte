<script lang="ts">
  import Icon from '../Icon.svelte'
  import Cover from '../Cover.svelte'
  import PageHeader from './PageHeader.svelte'
  import { playlists } from '../playlists.svelte'
  import { library } from '../library.svelte'

  let {
    onOpenPlaylist,
    onOpenPage,
  }: { onOpenPlaylist: (id: number) => void; onOpenPage: (page: 'folders' | 'settings') => void } = $props()

  let creating = $state(false)
  let name = $state('')

  async function create() {
    const trimmed = name.trim()
    if (trimmed === '') return
    const made = await playlists.create(trimmed)
    creating = false
    name = ''
    if (made) onOpenPlaylist(made.id)
  }
</script>

<div class="flex h-full flex-col">
  <PageHeader title="Your Library">
    {#snippet actions()}
      <button class="-mr-2 rounded-full p-2.5 active:bg-white/10" aria-label="New playlist" onclick={() => (creating = !creating)}>
        <Icon name="plus" class="size-6" />
      </button>
    {/snippet}
  </PageHeader>

  <div class="min-h-0 flex-1 overflow-y-auto pb-4">
    {#if creating}
      <form
        class="flex gap-2 px-4 pb-3"
        onsubmit={(e) => {
          e.preventDefault()
          void create()
        }}
      >
        <input
          class="min-w-0 flex-1 rounded-lg bg-zinc-800 px-3 py-2.5 text-[15px] outline-none focus:ring-2 focus:ring-teal-500"
          placeholder="Playlist name"
          aria-label="New playlist name"
          bind:value={name}
          {@attach (el) => el.focus()}
        />
        <button class="rounded-lg bg-teal-500 px-4 text-sm font-semibold text-zinc-950 disabled:opacity-40" disabled={name.trim() === ''}>
          Create
        </button>
      </form>
    {/if}

    {#if library.scanning}
      <p class="flex items-center gap-2 px-4 pb-3 text-sm text-teal-400" role="status">
        <span class="size-2 animate-pulse rounded-full bg-teal-400"></span>
        Scanning{library.scan ? ` · ${library.scan.scanned.toLocaleString()} files` : '…'}
      </p>
    {/if}

    {#each playlists.list as p (p.id)}
      <button class="flex h-[72px] w-full items-center gap-3 px-4 text-left active:bg-white/5" onclick={() => onOpenPlaylist(p.id)}>
        <Cover cover={null} title={p.name} class="size-14 shrink-0 rounded text-lg" />
        <span class="min-w-0 flex-1">
          <span class="block truncate text-[15px]">{p.name}</span>
          <span class="mt-0.5 block text-[13px] text-zinc-400">
            Playlist · {p.trackCount.toLocaleString()} {p.trackCount === 1 ? 'song' : 'songs'}
          </span>
        </span>
      </button>
    {:else}
      {#if !creating}
        <button class="flex h-[72px] w-full items-center gap-3 px-4 text-left active:bg-white/5" onclick={() => (creating = true)}>
          <span class="flex size-14 shrink-0 items-center justify-center rounded bg-zinc-800 text-zinc-300">
            <Icon name="plus" class="size-7" />
          </span>
          <span class="text-[15px]">Create a playlist</span>
        </button>
      {/if}
    {/each}

    <div class="mt-4 border-t border-zinc-800 pt-2">
      <button class="flex h-14 w-full items-center gap-4 px-4 text-left active:bg-white/5" onclick={() => onOpenPage('folders')}>
        <Icon name="folder" class="size-6 text-zinc-400" />
        <span class="min-w-0 flex-1">
          <span class="block text-[15px]">Music folders</span>
          <span class="block text-[13px] text-zinc-400">
            {library.stats.tracks.toLocaleString()} songs · {library.stats.albums.toLocaleString()} albums
          </span>
        </span>
        <Icon name="chevronRight" class="size-5 text-zinc-500" />
      </button>
      <button class="flex h-14 w-full items-center gap-4 px-4 text-left active:bg-white/5" onclick={() => onOpenPage('settings')}>
        <Icon name="settings" class="size-6 text-zinc-400" />
        <span class="flex-1 text-[15px]">Settings</span>
        <Icon name="chevronRight" class="size-5 text-zinc-500" />
      </button>
    </div>
  </div>
</div>
