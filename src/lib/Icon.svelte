<script lang="ts">
  type Name =
    | 'play'
    | 'pause'
    | 'prev'
    | 'next'
    | 'volume'
    | 'mute'
    | 'close'
    | 'folder'
    | 'lyrics'
    | 'refresh'
    | 'shuffle'
    | 'repeat'
    | 'repeatOne'
    | 'search'
    | 'upload'
    | 'plus'
    | 'more'
    | 'up'
    | 'down'
    | 'trash'
    | 'album'
    | 'music'
    | 'library'
    | 'back'
    | 'chevronRight'
    | 'queue'
    | 'settings'
    | 'expand'
    | 'download'
    | 'check'
    | 'pin'
  let { name, class: className = 'size-5' }: { name: Name; class?: string } = $props()

  const paths: Record<Name, string> = {
    expand: 'M14 3h7v7h-2V6.41l-5.29 5.3-1.42-1.42L17.59 5H14V3ZM10 21H3v-7h2v3.59l5.29-5.3 1.42 1.42L6.41 19H10v2Z',
    album: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 14.5a4.5 4.5 0 1 1 0-9 4.5 4.5 0 0 1 0 9Zm0-5.5a1 1 0 1 0 0 2 1 1 0 0 0 0-2Z',
    music: 'M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6Z',
    library: 'M20 2H8a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2Zm-2 5h-3v5.5a2.5 2.5 0 1 1-2.5-2.5c.57 0 1.08.19 1.5.51V5h4v2ZM4 6H2v14a2 2 0 0 0 2 2h14v-2H4V6Z',
    back: 'M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12l4.58-4.59Z',
    chevronRight: 'M10 6 8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6-6-6Z',
    queue: 'M15 6H3v2h12V6Zm0 4H3v2h12v-2ZM3 16h8v-2H3v2ZM17 6v8.18A3 3 0 1 0 19 17V8h3V6h-5Z',
    settings: 'M3 17v2h6v-2H3ZM3 5v2h10V5H3Zm10 16v-2h8v-2h-8v-2h-2v6h2ZM7 9v2H3v2h4v2h2V9H7Zm14 4v-2H11v2h10Zm-6-4h2V7h4V5h-4V3h-2v6Z',
    play: 'M8 5.14v13.72a1 1 0 0 0 1.5.86l11-6.86a1 1 0 0 0 0-1.72l-11-6.86A1 1 0 0 0 8 5.14Z',
    pause: 'M7 5h3.5v14H7zM13.5 5H17v14h-3.5z',
    prev: 'M6 5h2v14H6zM19 6.2v11.6a.8.8 0 0 1-1.24.67L9.5 13a1.2 1.2 0 0 1 0-2l8.26-5.47A.8.8 0 0 1 19 6.2Z',
    next: 'M16 5h2v14h-2zM5 6.2v11.6a.8.8 0 0 0 1.24.67L14.5 13a1.2 1.2 0 0 0 0-2L6.24 5.53A.8.8 0 0 0 5 6.2Z',
    volume: 'M4 9.5v5h3.5L12 19V5L7.5 9.5H4Zm11.5 2.5a4 4 0 0 0-2-3.46v6.92a4 4 0 0 0 2-3.46ZM13.5 4.2v2.06a6 6 0 0 1 0 11.48v2.06a8 8 0 0 0 0-15.6Z',
    mute: 'M4 9.5v5h3.5L12 19V5L7.5 9.5H4Zm12.3-1.2L15 9.6l2.4 2.4-2.4 2.4 1.3 1.3 2.4-2.4 2.4 2.4 1.3-1.3-2.4-2.4 2.4-2.4-1.3-1.3-2.4 2.4-2.4-2.4Z',
    close: 'M6.4 5 5 6.4 10.6 12 5 17.6 6.4 19l5.6-5.6 5.6 5.6 1.4-1.4-5.6-5.6L19 6.4 17.6 5 12 10.6 6.4 5Z',
    folder: 'M3 6.5A1.5 1.5 0 0 1 4.5 5h4.38l2 2H19.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5v-11Z',
    lyrics: 'M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v9a1.5 1.5 0 0 1-1.5 1.5H9l-4.5 3.5V16H5.5A1.5 1.5 0 0 1 4 14.5v-9Zm3 2v1.5h10V7.5H7Zm0 3.5v1.5h7V11H7Z',
    refresh: 'M17.65 6.35A7.96 7.96 0 0 0 12 4a8 8 0 1 0 7.75 10h-2.08A6 6 0 1 1 12 6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35Z',
    shuffle: 'M10.59 9.17 5.41 4 4 5.41l5.17 5.17 1.42-1.41ZM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5Zm.33 9.41-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13Z',
    repeat: 'M7 7h10v3l4-4-4-4v3H5v6h2V7Zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4Z',
    repeatOne: 'M7 7h10v3l4-4-4-4v3H5v6h2V7Zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4Zm-4-2V9h-1l-2 1v1h1.5v4H13Z',
    search: 'M15.5 14h-.79l-.28-.27a6.5 6.5 0 1 0-.7.7l.27.28v.79l5 4.99L20.49 19l-4.99-5Zm-6 0a4.5 4.5 0 1 1 0-9 4.5 4.5 0 0 1 0 9Z',
    upload: 'M12 4 7 9h3v6h4V9h3l-5-5Zm-7 13v2h14v-2H5Z',
    plus: 'M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2Z',
    more: 'M6 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4Zm6 0a2 2 0 1 0 0 4 2 2 0 0 0 0-4Zm6 0a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z',
    up: 'M7.41 15.41 12 10.83l4.59 4.58L18 14l-6-6-6 6 1.41 1.41Z',
    down: 'M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41Z',
    trash: 'M6 19a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7H6v12ZM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4Z',
    download:
      'M5 20h14v-2H5v2Zm7-16v8.17l3.59-3.58L17 10l-5 5-5-5 1.41-1.41L11 12.17V4h1Z',
    check: 'M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41L9 16.17Z',
    pin: 'M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2Z',
  }
</script>

<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" class={className}>
  <path d={paths[name]} />
</svg>
