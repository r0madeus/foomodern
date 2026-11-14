<script lang="ts">
  let {
    checked = $bindable(false),
    disabled = false,
    ariaLabelledby,
    ariaDescribedby,
    ariaLabel,
    onchange,
  }: {
    checked?: boolean
    disabled?: boolean
    ariaLabelledby?: string
    ariaDescribedby?: string
    ariaLabel?: string
    onchange?: (next: boolean) => void
  } = $props()

  function toggle() {
    if (disabled) return
    const next = !checked
    checked = next
    onchange?.(next)
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-labelledby={ariaLabelledby}
  aria-describedby={ariaDescribedby}
  aria-label={ariaLabel}
  {disabled}
  class={[
    'relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full p-0.5 transition-colors duration-200 outline-none focus-visible:ring-2 focus-visible:ring-accent-500 focus-visible:ring-offset-2 focus-visible:ring-offset-white dark:focus-visible:ring-offset-zinc-900 disabled:cursor-not-allowed disabled:opacity-45',
    checked ? 'bg-accent-600 dark:bg-accent-500' : 'bg-zinc-300 dark:bg-zinc-700',
  ]}
  onclick={toggle}
>
  <span
    class={[
      'pointer-events-none block size-5 rounded-full bg-white shadow-sm transition-transform duration-200',
      checked ? 'translate-x-5' : 'translate-x-0',
    ]}
  ></span>
</button>
