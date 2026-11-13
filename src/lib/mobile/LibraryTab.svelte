<script lang="ts">
  import { count, i18n, t } from '../i18n.svelte'
  import Icon from '../Icon.svelte'
  import PlaylistCover from '../PlaylistCover.svelte'
  import PageHeader from './PageHeader.svelte'
  import { favorites } from '../favorites.svelte'
  import { playlists } from '../playlists.svelte'
  import { library } from '../library.svelte'

  let {
    onOpenPlaylist,
    onOpenFavorites,
    onOpenArtists,
    onOpenStats,
    onOpenPage,
  }: {
    onOpenPlaylist: (id: number) => void
    onOpenFavorites: () => void
    onOpenArtists: () => void
    onOpenStats: () => void
    onOpenPage: (page: 'folders' | 'settings') => void
  } = $props()

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
  <PageHeader title={t("Your Library")}>
    {#snippet actions()}
      <button class="-mr-2 rounded-full p-2.5 active:bg-white/10" aria-label={t("New playlist")} onclick={() => (creating = !creating)}>
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
          placeholder={t("Playlist name")}
          aria-label={t("New playlist name")}
          bind:value={name}
          {@attach (el) => el.focus()}
        />
        <button class="rounded-lg bg-teal-500 px-4 text-sm font-semibold text-zinc-950 disabled:opacity-40" disabled={name.trim() === ''}>
          {t("Create")}
        </button>
      </form>
    {/if}

    {#if library.scanning}
      <p class="flex items-center gap-2 px-4 pb-3 text-sm text-teal-400" role="status">
        <span class="size-2 animate-pulse rounded-full bg-teal-400"></span>
        {library.scan ? t('Scanning · {n} files', { n: library.scan.scanned.toLocaleString(i18n.locale) }) : t('Scanning…')}
      </p>
    {/if}

    <button
      type="button"
      class="flex h-[72px] w-full items-center gap-3 px-4 text-left active:bg-white/5"
      onclick={onOpenFavorites}
    >
      <span class="flex size-14 shrink-0 items-center justify-center rounded bg-gradient-to-br from-teal-400 to-indigo-600 text-white">
        <Icon name="star" class="size-7" />
      </span>
      <span class="min-w-0 flex-1">
        <span class="block truncate text-[15px] font-medium">{t('Favorites')}</span>
        <span class="mt-0.5 block text-[13px] text-zinc-400">
          {t('Playlist · {count}', { count: count(favorites.ids.size, '{n} song', '{n} songs') })}
        </span>
      </span>
    </button>

    <button
      type="button"
      class="flex h-14 w-full items-center gap-4 px-4 text-left active:bg-white/5"
      onclick={onOpenArtists}
    >
      <Icon name="artist" class="size-6 text-teal-400" />
      <span class="min-w-0 flex-1 text-[15px]">{t('Artists')}</span>
      <Icon name="chevronRight" class="size-5 text-zinc-500" />
    </button>

    <button
      type="button"
      class="flex h-14 w-full items-center gap-4 px-4 text-left active:bg-white/5"
      onclick={onOpenStats}
    >
      <Icon name="chart" class="size-6 text-teal-400" />
      <span class="min-w-0 flex-1 text-[15px]">{t('Listening stats')}</span>
      <Icon name="chevronRight" class="size-5 text-zinc-500" />
    </button>

    {#each playlists.list as p (p.id)}
      <button class="flex h-[72px] w-full items-center gap-3 px-4 text-left active:bg-white/5" onclick={() => onOpenPlaylist(p.id)}>
        <PlaylistCover
          cover={p.cover}
          albumCovers={p.albumCovers}
          name={p.name}
          class="size-14 shrink-0 rounded text-lg"
          iconClass="size-6"
        />
        <span class="min-w-0 flex-1">
          <span class="block truncate text-[15px]">{p.name}</span>
          <span class="mt-0.5 block text-[13px] text-zinc-400">
            {t('Playlist · {count}', { count: count(p.trackCount, '{n} song', '{n} songs') })}
          </span>
        </span>
      </button>
    {:else}
      {#if !creating}
        <button class="flex h-[72px] w-full items-center gap-3 px-4 text-left active:bg-white/5" onclick={() => (creating = true)}>
          <span class="flex size-14 shrink-0 items-center justify-center rounded bg-zinc-800 text-zinc-300">
            <Icon name="plus" class="size-7" />
          </span>
          <span class="text-[15px]">{t("Create a playlist")}</span>
        </button>
      {/if}
    {/each}

    <button
      class="flex h-14 w-full items-center gap-4 px-4 text-left active:bg-white/5"
      onclick={async () => {
        const made = await playlists.importM3u()
        if (made) onOpenPlaylist(made.id)
      }}
    >
      <Icon name="upload" class="size-6 text-zinc-400" />
      <span class="min-w-0 flex-1">
        <span class="block text-[15px]">{t("Import playlist")}</span>
        <span class="block text-[13px] text-zinc-400">{t("An M3U file exported from FooModern on another device")}</span>
      </span>
    </button>

    <div class="mt-4 border-t border-zinc-800 pt-2">
      <button class="flex h-14 w-full items-center gap-4 px-4 text-left active:bg-white/5" onclick={() => onOpenPage('folders')}>
        <Icon name="folder" class="size-6 text-zinc-400" />
        <span class="min-w-0 flex-1">
          <span class="block text-[15px]">{t("Music folders")}</span>
          <span class="block text-[13px] text-zinc-400">
            {count(library.stats.tracks, '{n} song', '{n} songs')} · {count(library.stats.albums, '{n} album', '{n} albums')}
          </span>
        </span>
        <Icon name="chevronRight" class="size-5 text-zinc-500" />
      </button>
      <button class="flex h-14 w-full items-center gap-4 px-4 text-left active:bg-white/5" onclick={() => onOpenPage('settings')}>
        <Icon name="settings" class="size-6 text-zinc-400" />
        <span class="flex-1 text-[15px]">{t("Settings")}</span>
        <Icon name="chevronRight" class="size-5 text-zinc-500" />
      </button>
    </div>
  </div>
</div>
