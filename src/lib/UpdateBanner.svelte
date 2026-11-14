<script lang="ts">
  import { appSettings } from './appSettings.svelte'
  import { t } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import { updates } from './updates.svelte'
</script>

{#if updates.available && appSettings.allowUpdates}
  <div
    role="status"
    class="flex items-center gap-3 rounded-lg bg-accent-600 px-3 py-2 text-sm text-white shadow-sm dark:bg-accent-700"
  >
    <span class="min-w-0 flex-1">{t('FooModern {v} is available.', { v: updates.available.version })}</span>
    <button class="shrink-0 rounded-md bg-white/15 px-2.5 py-1 font-medium hover:bg-white/25" onclick={() => void updates.open()}>
      {t("Download")}
    </button>
    <button class="-mr-1 shrink-0 rounded-md p-1 hover:bg-white/15" aria-label={t("Dismiss")} onclick={() => updates.dismiss()}>
      <Icon name="close" class="size-4" />
    </button>
  </div>
{/if}
