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
  Sync: 'Senkronize et',
  Retry: 'Tekrar dene',
  'Embedded tag': 'Gömülü etiket',
  NetEase: 'NetEase',
  'Save lyrics': 'Sözleri kaydet',
  'Next to the song (.lrc file)': 'Şarkının yanına (.lrc dosyası)',
  'Inside the song file (tag)': 'Şarkı dosyasının içine (etiket)',
  "Only in the app's cache": 'Yalnız uygulamanın önbelleğinde',
  "Lyrics found online are also saved permanently, so they work offline and in other players. An existing .lrc file or lyrics tag is never replaced. Lyrics in a song's tag are written once the song is no longer the one playing.":
    'Çevrimiçi bulunan sözler kalıcı olarak da kaydedilir; böylece çevrimdışı ve başka oynatıcılarda da çalışır. Var olan bir .lrc dosyası ya da söz etiketi asla değiştirilmez. Etikete yazma, şarkı çalan şarkı olmaktan çıkınca yapılır.',
  'NetEase Cloud Music': 'NetEase Cloud Music',
  'Also search NetEase Cloud Music (unofficial; used when LRCLIB has nothing)':
    'NetEase Cloud Music’te de ara (resmî olmayan; LRCLIB’de bir şey yoksa kullanılır)',
  'Save cached lyrics to songs': 'Önbellekteki sözleri şarkılara kaydet',
  'Writes the lyrics already found for your library with the setting above. Songs that already have lyrics are skipped.':
    'Kütüphanen için daha önce bulunan sözleri yukarıdaki ayara göre yazar. Sözü olan şarkılar atlanır.',
  '{saved} saved, {skipped} skipped': '{saved} kaydedildi, {skipped} atlandı',
  'Lyrics coverage': 'Söz kapsamı',
  'How many songs of your library have lyrics.': 'Kütüphanendeki kaç şarkının sözü var.',
  '{with} with lyrics · {without} checked, none found · {unchecked} not looked up yet':
    '{with} şarkının sözü var · {without} arandı, bulunamadı · {unchecked} henüz aranmadı',
  'Counting…': 'Sayılıyor…',
  Count: 'Say',
  'Share on LRCLIB': 'LRCLIB’de paylaş',
  'Your lyrics will be published publicly on lrclib.net. Sending takes a few minutes of processor work (LRCLIB requires it against spam).':
    'Sözlerin lrclib.net’te herkese açık olarak yayınlanacak. Gönderim birkaç dakika işlemci kullanır (LRCLIB istenmeyen gönderilere karşı bunu şart koşuyor).',
  Publish: 'Yayınla',
  'Sharing… (this can take a few minutes)': 'Paylaşılıyor… (birkaç dakika sürebilir)',
  'Shared on LRCLIB. Thank you!': 'LRCLIB’de paylaşıldı. Teşekkürler!',
  'Sharing cancelled.': 'Paylaşım iptal edildi.',
  'Could not share: {error}': 'Paylaşılamadı: {error}',
  'No lyrics of your own to share.': 'Paylaşılacak kendi sözün yok.',
  'Needs title, artist, album and duration in the tags.': 'Etiketlerde başlık, sanatçı, albüm ve süre gerekir.',
  'LRCLIB already has these lyrics.': 'LRCLIB’de bu sözler zaten var.',
  'These lyrics came from another service.': 'Bu sözler başka bir servisten geldi.',
  'Could not reach LRCLIB.': 'LRCLIB’ye ulaşılamadı.',
  'Another share is running.': 'Başka bir paylaşım sürüyor.',

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

  // Find info (MusicBrainz)
  'Find info': 'Bilgileri bul',
  'Find info for this track': 'Bu parça için bilgileri bul',
  Recommended: 'Önerilen',
  Check: 'Kontrol et',
  Apply: 'Uygula',
  'Applying…': 'Uygulanıyor…',
  Undo: 'Geri al',
  'Undoing…': 'Geri alınıyor…',
  'Searching MusicBrainz…': 'MusicBrainz aranıyor…',
  'MusicBrainz could not be reached': 'MusicBrainz\'e ulaşılamadı',
  'No matching release was found on MusicBrainz.': 'MusicBrainz\'te eşleşen bir sürüm bulunamadı.',
  'Matches from MusicBrainz. Nothing is written until you apply.':
    'MusicBrainz eşleşmeleri. Sen uygulamadan hiçbir şey yazılmaz.',
  Matches: 'Eşleşmeler',
  Changes: 'Değişiklikler',
  'Match score {score}': 'Eşleşme puanı {score}',
  'No matching track on this release; left unchanged.': 'Bu sürümde eşleşen parça yok; değiştirilmedi.',
  'No visible changes': 'Görünür değişiklik yok',
  'MusicBrainz ids are saved in the tags too, and a cover is added if the album has none.':
    'MusicBrainz kimlikleri de etiketlere yazılır; albümün kapağı yoksa kapak eklenir.',
  'The changes were undone.': 'Değişiklikler geri alındı.',
  'Cover added': 'Kapak eklendi',
  'Save or discard your edits first.': 'Önce düzenlemelerini kaydet ya da vazgeç.',

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
  Cancelled: 'İptal edildi',
  'Cancel download': 'İndirmeyi iptal et',
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
  // Offline mode and internet switches
  Internet: 'İnternet',
  'Offline mode': 'Çevrimdışı mod',
  'Nothing connects to the internet': 'Hiçbir şey internete bağlanmaz',
  'Offline mode is on, so the switches above are off.':
    'Çevrimdışı mod açık, bu yüzden yukarıdaki anahtarlar kapalı.',
  'Turned off in Settings': 'Ayarlar’dan kapalı',
  'Not checked': 'Denetlenmedi',
  'Lyrics (LRCLIB, NetEase)': 'Şarkı sözleri (LRCLIB, NetEase)',
  'Looks up lyrics online and lets you share your own on LRCLIB. Lyrics next to your songs and in their tags keep working.':
    'Sözleri internetten arar ve kendi sözlerini LRCLIB’de paylaşmanı sağlar. Şarkıların yanındaki ve etiketlerindeki sözler çalışmaya devam eder.',
  'Online search and downloads (YouTube, iTunes)': 'Çevrimiçi arama ve indirme (YouTube, iTunes)',
  'Online search, streaming and downloads, including album search and covers through iTunes.':
    'Çevrimiçi arama, dinleme ve indirme; iTunes üzerinden albüm arama ve kapaklar dahil.',
  'Artist info (Deezer, Wikipedia, iTunes)': 'Sanatçı bilgisi (Deezer, Wikipedia, iTunes)',
  'Artist portraits, biographies and missing albums. Anything already saved is still shown.':
    'Sanatçı fotoğrafları, biyografiler ve eksik albümler. Daha önce kaydedilenler yine gösterilir.',
  'Find info (MusicBrainz, Cover Art Archive)': 'Bilgi bul (MusicBrainz, Cover Art Archive)',
  'Finds album and track tags and covers on MusicBrainz.':
    'Albüm ve şarkı etiketlerini ve kapaklarını MusicBrainz’de bulur.',
  'Update check (GitHub)': 'Güncelleme denetimi (GitHub)',
  'Checks once a day whether a new version of FooModern is available.':
    'FooModern’in yeni bir sürümü olup olmadığını günde bir kez denetler.',
  'Search your library…': 'Kütüphanende ara…',
  'Online search is turned off in Settings.': 'Çevrimiçi arama Ayarlar’dan kapalı.',
  'Type to search your local library.': 'Yerel kütüphanende aramak için yaz.',
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

  // Unified search & filters
  'Filter…': 'Filtrele…',
  'Filter library': 'Kütüphaneyi filtrele',
  'Filter songs, artists, albums…': 'Şarkı, sanatçı, albüm filtrele…',
  'Filter albums, artists, songs…': 'Albüm, sanatçı, şarkı filtrele…',
  'Search your library & online (Enter for online)…':
    'Kütüphanede ve internette ara (çevrimiçi için Enter)…',
  'Search library and online': 'Kütüphanede ve internette ara',
  'Online search': 'İnternette ara',
  'Type to search your local library immediately, or press Enter to search, stream, and download online.':
    'Kütüphanende anında aramak için yaz; internette arama, dinleme ve indirme için Enter’a bas.',
  'In your library': 'Kütüphanende',
  'No matches in your library.': 'Kütüphanende eşleşme yok.',
  Online: 'Çevrimiçi',
  'Searching online…': 'İnternette aranıyor…',
  'No online results for “{q}”.': '“{q}” için çevrimiçi sonuç yok.',
  'In library': 'Kütüphanede',
  'Press Enter to search online for “{q}”.': '“{q}” için internette aramak üzere Enter’a bas.',

  // Favorites
  Favorites: 'Favoriler',
  'Add “{title}” to favorites': '“{title}” parçasını favorilere ekle',
  'Remove “{title}” from favorites': '“{title}” parçasını favorilerden kaldır',
  'No favorite tracks yet. Click the star on any song to save it here.':
    'Henüz favori şarkı yok. Buraya eklemek için herhangi bir şarkının yıldızına tıkla.',

  // Playlist covers
  'Change cover': 'Kapağı değiştir',
  Cover: 'Kapak',
  'Playlist cover': 'Çalma listesi kapağı',
  'Choose photo…': 'Fotoğraf seç…',
  'Choose playlist cover': 'Çalma listesi kapağını seç',
  'Use automatic cover': 'Otomatik kapak kullan',
  Gradient: 'Gradyan',
  Icon: 'İkon',
  'Apply gradient & icon': 'Gradyan ve ikonu uygula',

  // Listening history & stats
  'Listening stats': 'Dinleme istatistikleri',
  'Your Wrapped': 'Dinleme Özeti',
  'Last 7 days': 'Son 7 gün',
  'Last 30 days': 'Son 30 gün',
  'All time': 'Tüm zamanlar',
  Custom: 'Özel aralık',
  From: 'Başlangıç',
  To: 'Bitiş',
  'Listening time': 'Dinleme süresi',
  Plays: 'Dinlenme',
  'Unique tracks': 'Farklı şarkı',
  'Unique artists': 'Farklı sanatçı',
  'Longest streak': 'En uzun seri',
  'Current streak': 'Aktif seri',
  '{n} day': '{n} gün',
  '{n} days': '{n} gün',
  'Completion rate': 'Tamamlama oranı',
  'Peak hour': 'En aktif saat',
  'Favorite day': 'Favori gün',
  'Morning listener': 'Sabah dinleyicisi',
  'Afternoon listener': 'Öğlen dinleyicisi',
  'Evening listener': 'Akşam dinleyicisi',
  'Night owl': 'Gece kuşu',
  '{done} finished · {skipped} skipped': '{done} tamamlandı · {skipped} atlandı',
  'Top tracks': 'En çok dinlenen şarkılar',
  'By time': 'Süreye göre',
  'By plays': 'Sayıya göre',
  'Top tracks · {period}': 'En çok dinlenenler · {period}',
  'Playlist “{name}” created': '“{name}” çalma listesi oluşturuldu',
  'Top artists': 'En çok dinlenen sanatçılar',
  'Top albums': 'En çok dinlenen albümler',
  'Top genres': 'En çok dinlenen türler',
  'Listening rhythm': 'Dinleme ritmin',
  'Hours of the day': 'Günün saatleri',
  'Days of the week': 'Haftanın günleri',
  'Monthly trend': 'Aylık gidişat',
  'Recent activity': 'Son dinlenenler',
  Completed: 'Tamamlandı',
  Skipped: 'Atlandı',
  Partial: 'Yarım kaldı',
  'Clear history': 'Geçmişi temizle',
  'Clear all listening history?': 'Tüm dinleme geçmişi silinsin mi?',
  'No listening history yet': 'Henüz dinleme geçmişi yok',
  'Play any song for a few seconds and FooModern will build your listening summary here automatically.':
    'Herhangi bir şarkıyı birkaç saniye dinle; FooModern dinleme özetini burada otomatik oluşturur.',
  'No tracks found in this period.': 'Bu aralıkta kayıtlı dinleme yok.',
  Mon: 'Pzt',
  Tue: 'Sal',
  Wed: 'Çar',
  Thu: 'Per',
  Fri: 'Cum',
  Sat: 'Cmt',
  Sun: 'Paz',
  Monday: 'Pazartesi',
  Tuesday: 'Salı',
  Wednesday: 'Çarşamba',
  Thursday: 'Perşembe',
  Friday: 'Cuma',
  Saturday: 'Cumartesi',
  Sunday: 'Pazar',
  '{h} hr {m} min': '{h} sa {m} dk',
  '{m} min': '{m} dk',
  '< 1 min': '< 1 dk',
  '{n} play': '{n} çalma',
  '{n} plays': '{n} çalma',
  Artists: 'Sanatçılar',
  '← Artists': '← Sanatçılar',
  'Filter artists…': 'Sanatçı filtrele…',
  'No artists match “{q}”.': '“{q}” ile eşleşen sanatçı yok.',
  'Your listens: {time} · {plays}': 'Dinlemen: {time} · {plays}',
  Popular: 'Popüler',
  'Albums in library': 'Kütüphanendeki albümler',
  'More albums (Online)': 'Diğer albümler (Çevrimiçi)',
  'All tracks': 'Tüm şarkılar',
  Home: 'Ana Menü',
  'Daily Mixes': 'Senin İçin Karışımlar',
  'Weekly Favorites': 'Haftanın Favorileri',
  'Most played and finished over the last 7 days':
    'Son 7 günde en çok dinlenen ve tamamlananlar',
  'On Repeat': 'Tekrar Tekrar',
  'Songs you kept coming back to this month': 'Bu ay dönüp dönüp çaldığın şarkılar',
  Rediscover: 'Yeniden Keşfet',
  "Favorites and past listens you haven't played lately":
    'Son zamanlarda çalmadığın favoriler ve eski dinlemelerin',
  '{name} Mix': '{name} Karışımı',
  'Inspired by what you played most lately': 'Son günlerde en çok dinlediklerinden esinlendi',
  'Recently played': 'Son çalınanlar',
  'Recently played tracks': 'Son çalınan şarkılar',
  'See all': 'Tümünü gör',
  'Mix saved as playlist “{name}”': 'Karışım “{name}” adıyla kaydedildi',
  'Mix could not be loaded.': 'Karışım yüklenemedi.',
  'Start playing some songs to unlock personalized mixes and recent history.':
    'Kişisel karışımların ve son dinlediklerinin oluşması için birkaç şarkı çal.',
  'Quick access': 'Hızlı erişim',
  'Dynamic mix': 'Dinamik karışım',
  'Good morning': 'Günaydın',
  'Good afternoon': 'İyi günler',
  'Good evening': 'İyi akşamlar',
  Appearance: 'Görünüm',
  Theme: 'Tema',
  Dark: 'Koyu',
  Light: 'Açık',
  'Choose whether FooModern follows your system theme or stays dark or light.':
    'FooModern’in sistem temasını mı izleyeceğini yoksa hep koyu ya da açık mı kalacağını seç.',
  All: 'Tümü',
  General: 'Genel',
  'Collapse sidebar': 'Kenar çubuğunu daralt',
  'Expand sidebar': 'Kenar çubuğunu genişlet',
}
