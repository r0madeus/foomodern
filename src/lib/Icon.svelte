<script lang="ts">
  type Name = 'play' | 'pause' | 'prev' | 'next' | 'volume' | 'mute' | 'close' | 'folder'
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
  }
</script>

<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" class={className}>
  <path d={paths[name]} />
</svg>
