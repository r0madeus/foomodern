<script lang="ts">
  import { t } from './i18n.svelte'

  // Three GPU-animated equalizer bars replacing the static play triangle.
  let {
    class: className = '',
    label,
  }: {
    class?: string
    label?: string
  } = $props()
</script>

<span
  class={['inline-flex h-3.5 items-end justify-center gap-[2px]', className]}
  role="img"
  aria-label={label ?? t('Playing')}
>
  <span class="eq-bar w-[2px] rounded-full bg-current" style:animation-delay="0ms"></span>
  <span class="eq-bar w-[2px] rounded-full bg-current" style:animation-delay="-320ms"></span>
  <span class="eq-bar w-[2px] rounded-full bg-current" style:animation-delay="-620ms"></span>
</span>
