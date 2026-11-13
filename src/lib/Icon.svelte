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
  let { name, class: className = 'size-5' }: { name: Name; class?: string } = $props()

  const paths: Record<Name, string> = {
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
  }
</script>

<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" class={className}>
  <path d={paths[name]} />
</svg>
