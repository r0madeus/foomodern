// Desktop theme preference ('system', 'dark', or 'light'). Phones stay dark.
export type ThemeChoice = 'system' | 'dark' | 'light'

const KEY = 'neomodern.theme'

function loadTheme(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY)
    if (v === 'system' || v === 'dark' || v === 'light') return v
  } catch {
    // storage unavailable
  }
  return 'system'
}

const media = typeof window !== 'undefined' ? window.matchMedia('(prefers-color-scheme: dark)') : null

class ThemeStore {
  choice = $state<ThemeChoice>(loadTheme())
  prefersDark = $state(media?.matches ?? true)
  isDark = $derived(this.choice === 'system' ? this.prefersDark : this.choice === 'dark')

  constructor() {
    media?.addEventListener('change', (e) => {
      this.prefersDark = e.matches
    })
  }

  setChoice(next: ThemeChoice): void {
    this.choice = next
    try {
      localStorage.setItem(KEY, next)
    } catch {
      // ignore
    }
  }
}

export const theme = new ThemeStore()
