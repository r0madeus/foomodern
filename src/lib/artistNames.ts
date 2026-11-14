// Splits compound artist strings for the clickable artist links. Mirrors
// `split_artists` in src-tauri/src/library/db.rs; both are checked against
// fixtures/artist-split-cases.json (`npm test` and `cargo test`).

const FEATURE_RE = /\(featuring |\[featuring |\(feat\.|\[feat\.|\(ft\.|\[ft\.| featuring | feat\. | feat | ft\. | ft /gi

function stripUnmatchedClose(part: string): string {
  let p = part.trim()
  if (p.endsWith(')') && !p.slice(0, -1).includes('(')) p = p.slice(0, -1).trim()
  if (p.endsWith(']') && !p.slice(0, -1).includes('[')) p = p.slice(0, -1).trim()
  return p
}

function isSingleWord(w: string): boolean {
  const t = w.trim()
  return t !== '' && !/\s/.test(t)
}

// "Earth, Wind & Fire", "Peter, Paul and Mary": three or more single words,
// the last two joined by & or "and".
function isSingleWordBand(parts: string[]): boolean {
  const last = parts[parts.length - 1].toLowerCase()
  let i = last.indexOf(' & ')
  let sep = 3
  if (i < 0) {
    i = last.indexOf(' and ')
    sep = 5
  }
  if (i < 0) return false
  const rest = parts.slice(0, -1)
  return (
    rest.length + 2 >= 3 &&
    rest.every(isSingleWord) &&
    isSingleWord(last.slice(0, i)) &&
    isSingleWord(last.slice(i + sep))
  )
}

function splitNameList(list: string): string[] {
  const parts: string[] = []
  for (const raw of list.split(',')) {
    const part = stripUnmatchedClose(raw)
    if (!part) continue
    if (parts.length > 0 && part.slice(0, 4).toLowerCase() === 'the ') {
      parts[parts.length - 1] += `, ${part}`
    } else {
      parts.push(part)
    }
  }
  if (parts.length > 1 && isSingleWordBand(parts)) return [parts.join(', ')]
  return parts
}

export function splitArtistNames(raw: string): string[] {
  const out: string[] = []
  const seen = new Set<string>()
  for (const segment of raw.trim().split(FEATURE_RE)) {
    for (const piece of segment.split(';')) {
      for (const name of splitNameList(piece)) {
        const key = name.toLowerCase()
        if (!seen.has(key)) {
          seen.add(key)
          out.push(name)
        }
      }
    }
  }
  return out
}
