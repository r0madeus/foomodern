// UI translations. The English text is the key; a missing translation
// falls back to English. Placeholders are written {name}.
//
// The language follows the system (Turkish if the system language is
// Turkish, English otherwise) unless chosen in Settings.

export type Lang = 'en' | 'tr'
export type LangChoice = 'system' | Lang
type Params = Record<string, string | number>

const KEY = 'foomodern.lang'

function loadChoice(): LangChoice {
  try {
    const v = localStorage.getItem(KEY)
    if (v === 'en' || v === 'tr' || v === 'system') return v
  } catch {
    // storage unavailable
  }
  return 'system'
}

function systemLang(): Lang {
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language]
  return langs.some((l) => l?.toLowerCase().startsWith('tr')) ? 'tr' : 'en'
}

class I18n {
  choice = $state<LangChoice>(loadChoice())
  lang = $derived<Lang>(this.choice === 'system' ? systemLang() : this.choice)
  /** Locale for numbers and dates. */
  locale = $derived(this.lang === 'tr' ? 'tr-TR' : 'en-US')

  setChoice(next: LangChoice): void {
    this.choice = next
    try {
      localStorage.setItem(KEY, next)
    } catch {
      // ignore
    }
  }
}

export const i18n = new I18n()

/** Translates `en` into the current language and fills in {placeholders}. */
export function t(en: string, params?: Params): string {
  const text = i18n.lang === 'tr' ? (TR[en] ?? en) : en
  if (!params) return text
  return text.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m))
}

/** `n` with the singular or plural key, e.g. count(3, '{n} song', '{n} songs'). */
export function count(n: number, one: string, many: string): string {
  return t(n === 1 ? one : many, { n: n.toLocaleString(i18n.locale) })
}

const TR: Record<string, string> = {
  // Navigation and views
  Songs: 'Şarkılar',
  Albums: 'Albümler',
  Queue: 'Sıra',
  Folders: 'Klasörler',
  Playlists: 'Çalma listeleri',
  Settings: 'Ayarlar',
  Library: 'Kütüphane',
  'Your Library': 'Kütüphanen',
  Search: 'Ara',
  Main: 'Ana menü',
  Lyrics: 'Şarkı sözleri',
  'Now playing': 'Şimdi çalıyor',
  '← Albums': '← Albümler',
  Back: 'Geri',

  // Player
  Play: 'Çal',
  Pause: 'Duraklat',
  Next: 'Sonraki',
  Previous: 'Önceki',
  Seek: 'Konum',
  Volume: 'Ses',
  Mute: 'Sesi kapat',
  Unmute: 'Sesi aç',
  Shuffle: 'Karıştır',
  'Shuffle play': 'Karışık çal',
  'Shuffle on': 'Karıştırma açık',
  'Shuffle off': 'Karıştırma kapalı',
  'Shuffle: On': 'Karıştır: Açık',
  'Shuffle: Off': 'Karıştır: Kapalı',
  'Repeat: One': 'Tekrarla: Tek şarkı',
  'Repeat: All': 'Tekrarla: Tümü',
  'Repeat: Off': 'Tekrarla: Kapalı',
  'Repeat one': 'Tek şarkıyı tekrarla',
  'Repeat all': 'Tümünü tekrarla',
  'Repeat off': 'Tekrar kapalı',
  'Nothing playing': 'Çalan bir şey yok',
  'Nothing playing.': 'Çalan bir şey yok.',
  Playing: 'Çalıyor',
  'Unknown artist': 'Bilinmeyen sanatçı',
  'Open now playing': '"Şimdi çalıyor" ekranını aç',
  'Close now playing': '"Şimdi çalıyor" ekranını kapat',
  'Close (Esc)': 'Kapat (Esc)',
  'Open player': 'Oynatıcıyı aç',
  'Close player': 'Oynatıcıyı kapat',
  '{i} of {n}': '{i} / {n}',

  // Lyrics
  'Full-screen lyrics': 'Tam ekran sözler',
  'Close lyrics': 'Sözleri kapat',
  'Refresh lyrics': 'Sözleri yenile',
  'Load .lrc file': '.lrc dosyası yükle',
  'Load .lrc': '.lrc yükle',
  'Local .lrc': 'Yerel .lrc',
  'Looking for lyrics…': 'Sözler aranıyor…',
  'No lyrics for this song.': 'Bu şarkının sözleri yok.',
  'No lyrics found.': 'Söz bulunamadı.',
  '♪ Instrumental': '♪ Enstrümantal',
  'Play a track to see lyrics.': 'Sözleri görmek için bir şarkı çal.',
  'Back to current line': 'Çalan satıra dön',
  Retry: 'Tekrar dene',

  // Lists and sorting
  'Sort by': 'Sırala',
  Artist: 'Sanatçı',
  Title: 'Başlık',
  Album: 'Albüm',
  Time: 'Süre',
  'Recently added': 'Son eklenenler',
  Recent: 'Yeni',
  '{n} matching': '{n} sonuç',
  'No songs match “{q}”.': '“{q}” ile eşleşen şarkı yok.',
  'No albums match “{q}”.': '“{q}” ile eşleşen albüm yok.',
  'Clear search': 'Aramayı temizle',
  Clear: 'Temizle',
  'Search…': 'Ara…',
  'Search library': 'Kütüphanede ara',
  'Search songs, artists, albums…': 'Şarkı, sanatçı, albüm ara…',
  'Search albums, artists, songs…': 'Albüm, sanatçı, şarkı ara…',
  'Songs, artists or albums': 'Şarkı, sanatçı veya albüm',
  'Search your library': 'Kütüphanende ara',
  'Find songs, artists and albums by name.': 'Şarkıları, sanatçıları ve albümleri adıyla bul.',
  'No results for “{q}”': '“{q}” için sonuç yok',
  'Check the spelling or try fewer words.': 'Yazımı kontrol et ya da daha az kelime dene.',
  '{n} song': '{n} şarkı',
  '{n} songs': '{n} şarkı',
  '{n} track': '{n} parça',
  '{n} tracks': '{n} parça',
  '{n} album': '{n} albüm',
  '{n} albums': '{n} albüm',
  'Album · {details}': 'Albüm · {details}',
  'Playlist · {count}': 'Çalma listesi · {count}',
  Loading: 'Yükleniyor',
  'Loading…': 'Yükleniyor…',

  // Library and folders
  'Your library is empty.': 'Kütüphanen boş.',
  'Add a music folder': 'Müzik klasörü ekle',
  'Add folder': 'Klasör ekle',
  'Music folders': 'Müzik klasörleri',
  'No folders yet. Add a folder to build your library.': 'Henüz klasör yok. Kütüphaneni oluşturmak için bir klasör ekle.',
  Rescan: 'Yeniden tara',
  Remove: 'Kaldır',
  'Scanning your music folders…': 'Müzik klasörlerin taranıyor…',
  'Scanning…': 'Taranıyor…',
  'Scanning · {n}': 'Taranıyor · {n}',
  'Scanning · {n} files': 'Taranıyor · {n} dosya',
  'Reading tags…': 'Etiketler okunuyor…',

  // Queue
  'The queue is empty.': 'Sıra boş.',
  'Open files': 'Dosya aç',
  'Add to queue': 'Sıraya ekle',
  'Save as playlist': 'Çalma listesi olarak kaydet',
  'Remove from queue': 'Sıradan kaldır',
  'Remove “{title}” from queue': '“{title}” sıradan kaldırılsın',
  'Close queue': 'Sırayı kapat',
  'Next up': 'Sıradaki',
  'Nothing after this song.': 'Bu şarkıdan sonra bir şey yok.',
  Audio: 'Ses',

  // Playlists
  Playlist: 'Çalma listesi',
  'New playlist': 'Yeni çalma listesi',
  'New playlist…': 'Yeni çalma listesi…',
  'New playlist name': 'Yeni çalma listesinin adı',
  'Playlist name': 'Çalma listesi adı',
  'No playlists yet.': 'Henüz çalma listesi yok.',
  'Create a playlist': 'Çalma listesi oluştur',
  Create: 'Oluştur',
  'Add to playlist': 'Çalma listesine ekle',
  'Add album to playlist': 'Albümü çalma listesine ekle',
  Rename: 'Yeniden adlandır',
  Delete: 'Sil',
  'Delete this playlist?': 'Bu çalma listesi silinsin mi?',
  Cancel: 'Vazgeç',
  Save: 'Kaydet',
  'Saving…': 'Kaydediliyor…',
  Close: 'Kapat',
  Export: 'Dışa aktar',
  'Export…': 'Dışa aktar…',
  'Save as an M3U file, e.g. to open on another device': 'M3U dosyası olarak kaydet (örneğin başka bir cihazda açmak için)',
  'Import playlist': 'Çalma listesi içe aktar',
  'Import playlist (M3U)': 'Çalma listesi içe aktar (M3U)',
  'An M3U file exported from FooModern on another device': 'Başka bir cihazdaki FooModern’den dışa aktarılmış M3U dosyası',
  'Playlist not found.': 'Çalma listesi bulunamadı.',
  'This playlist is empty. Use the “…” button on a song or album to add tracks.':
    'Bu çalma listesi boş. Şarkı eklemek için bir şarkının ya da albümün “…” düğmesini kullan.',
  'This playlist is empty. Use the ⋯ menu on a song or album to add tracks.':
    'Bu çalma listesi boş. Şarkı eklemek için bir şarkının ya da albümün ⋯ menüsünü kullan.',
  'More actions for “{title}”': '“{title}” için diğer işlemler',
  'Remove “{title}” from playlist': '“{title}” çalma listesinden kaldırılsın',
  'Move “{title}” up': '“{title}” yukarı taşınsın',
  'Move “{title}” down': '“{title}” aşağı taşınsın',
  '{path}\nDouble-click or Enter to play. Alt+Up/Down moves, Delete removes.':
    '{path}\nÇalmak için çift tıkla ya da Enter. Alt+Yukarı/Aşağı taşır, Delete kaldırır.',
  'Added {count} to “{name}”': '“{name}” listesine {count} eklendi',
  'Added album to “{name}”': 'Albüm “{name}” listesine eklendi',
  'Added {count} to new playlist “{name}”': 'Yeni “{name}” listesine {count} eklendi',
  'Saved “{name}” with {count}': '“{name}” {count} ile kaydedildi',
  ' ({n} not in the library, skipped)': ' ({n} tanesi kütüphanede yok, atlandı)',
  'Exported “{name}”': '“{name}” dışa aktarıldı',
  'Imported “{name}” with {count}': '“{name}” {count} ile içe aktarıldı',
  ' ({n} not found in this library)': ' ({n} tanesi bu kütüphanede bulunamadı)',
  Imported: 'İçe aktarılan',

  // Tags
  'Edit tags': 'Etiketleri düzenle',
  'Edit tags…': 'Etiketleri düzenle…',
  'Edit album tags': 'Albüm etiketlerini düzenle',
  'Edit tags of {n} tracks': '{n} parçanın etiketlerini düzenle',
  'Album artist': 'Albüm sanatçısı',
  'Track number': 'Parça no',
  'Disc number': 'Disk no',
  Year: 'Yıl',
  Genre: 'Tür',
  '(multiple values)': '(farklı değerler)',
  'These tracks are no longer in the library.': 'Bu parçalar artık kütüphanede değil.',
  'Year must be between 1 and 9999.': 'Yıl 1 ile 9999 arasında olmalı.',
  'Updated tags of {count}': '{count} parçanın etiketleri güncellendi',
  '{n} file': '{n} dosya',
  '{n} files': '{n} dosya',
  'Only the fields you change are written.': 'Yalnızca değiştirdiğin alanlar yazılır.',
  '(will be cleared)': '(silinecek)',
  '{label} must be a whole number.': '{label} tam sayı olmalı.',
  'Clear {label} on all files': 'Tüm dosyalarda {label} alanını temizle',
  '{n} updated, ': '{n} güncellendi, ',
  '{count} could not be edited:': '{count} düzenlenemedi:',

  // Settings
  Playback: 'Çalma',
  ReplayGain: 'ReplayGain',
  'Evens out loudness using ReplayGain tags. Track levels every song; Album keeps the loudness differences inside an album.':
    'ReplayGain etiketleriyle ses yüksekliğini dengeler. "Parça" her şarkıyı eşitler; "Albüm" albüm içindeki ses farklarını korur.',
  Off: 'Kapalı',
  Track: 'Parça',
  'Show file extensions': 'Dosya uzantılarını göster',
  'Adds a small format badge (FLAC, MP3…) to song rows and album track lists.':
    'Şarkı satırlarına ve albüm parça listelerine küçük bir biçim rozeti (FLAC, MP3…) ekler.',
  Window: 'Pencere',
  'Keep playing in the tray when the window is closed': 'Pencere kapatılınca sistem tepsisinde çalmaya devam et',
  'Closing the window hides FooModern to the system tray and playback continues. Use the tray menu to show the window again or to quit.':
    'Pencereyi kapatmak FooModern’i sistem tepsisine gizler ve çalma sürer. Pencereyi geri getirmek ya da çıkmak için tepsi menüsünü kullan.',
  'No system tray is available here, so closing the window always quits.':
    'Burada sistem tepsisi yok, bu yüzden pencereyi kapatmak her zaman uygulamadan çıkar.',
  Storage: 'Depolama',
  'Access to all files': 'Tüm dosyalara erişim',
  'Needed to edit tags and to attach .lrc lyrics next to your songs. Playing music works without it.':
    'Etiket düzenlemek ve şarkıların yanına .lrc sözleri eklemek için gerekir. Müzik çalmak için gerekmez.',
  Allow: 'İzin ver',
  Allowed: 'İzin verildi',
  Language: 'Dil',
  'Language of the app. "System default" follows the device language.': 'Uygulamanın dili. "Sistem dili" cihazın dilini izler.',
  'System default': 'Sistem dili',
  About: 'Hakkında',
  ' · version {v}': ' · sürüm {v}',
  'Check for updates': 'Güncellemeleri denetle',
  'Checking…': 'Denetleniyor…',
  'You have the latest version.': 'En son sürümü kullanıyorsun.',
  'Could not reach GitHub.': 'GitHub’a ulaşılamadı.',
  'Version {v} is available.': '{v} sürümü çıktı.',
  'Log files': 'Log dosyaları',
  'Attach these when reporting a problem.': 'Bir sorun bildirirken bunları ekle.',
  Copy: 'Kopyala',
  Copied: 'Kopyalandı',

  // Updates
  'FooModern {v} is available.': 'FooModern {v} çıktı.',
  Download: 'İndir',
  Dismiss: 'Kapat',

  // Online search & downloads
  '← Search': '← Ara',
  'Download album': 'Albümü indir',
  'Download track': 'Parçayı indir',
  'Download “{title}”': '“{title}” parçasını indir',
  Downloaded: 'İndirildi',
  'Downloading…': 'İndiriliyor…',
  'Downloading {done}/{total}': 'İndiriliyor {done}/{total}',
  Queued: 'Sırada',
  Done: 'Bitti',
  Failed: 'Başarısız',
  'No tracks found for this album.': 'Bu albüm için parça bulunamadı.',
  'Search songs, albums, or paste a link…': 'Şarkı, albüm ara veya bağlantı yapıştır…',
  'Search music online': 'Çevrimiçi müzik ara',
  'Searching…': 'Aranıyor…',
  Refresh: 'Yenile',
  'yt-dlp was not found. You can still browse albums; install yt-dlp or set its path in Settings to stream and download.':
    'yt-dlp bulunamadı. Albümlere göz atmaya devam edebilirsin; dinlemek ve indirmek için yt-dlp kur veya Ayarlar’dan yolunu seç.',
  'Find albums and tracks, stream them right away, or download them straight into your library.':
    'Albüm ve şarkıları ara, hemen dinle ya da doğrudan kütüphanene indir.',
  Downloads: 'İndirilenler',
  '{n} active': '{n} etkin',
  'Clear finished': 'Bitenleri temizle',
  'Search & Downloads': 'Ara ve İndir',
  'Download folder': 'İndirme klasörü',
  'Choose download folder': 'İndirme klasörünü seç',
  'Locate yt-dlp executable': 'yt-dlp dosyasını seç',
  'Choose…': 'Seç…',
  Reset: 'Sıfırla',
  Ready: 'Hazır',
  'Not found': 'Bulunamadı',
  'ffmpeg available': 'ffmpeg hazır',
  'ffmpeg missing': 'ffmpeg yok',

  // Pinned sidebar items
  Pin: 'Sabitle',
  Pinned: 'Sabitlendi',
  'Pinned items': 'Sabitlenenler',
  Unpin: 'Sabitlemeyi kaldır',
  'Pin “{title}”': '“{title}” öğesini sabitle',
  'Unpin “{title}”': '“{title}” sabitlemesini kaldır',
}
