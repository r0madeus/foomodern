import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { splitArtistNames } from '../src/lib/artistNames.ts'

const fixture = JSON.parse(
  readFileSync(new URL('../fixtures/artist-split-cases.json', import.meta.url), 'utf8'),
) as { cases: [string, string[]][] }

test('UI artist splitting matches the shared cases', () => {
  assert.ok(fixture.cases.length > 0)
  for (const [input, expected] of fixture.cases) {
    assert.deepEqual(splitArtistNames(input), expected, `input: ${JSON.stringify(input)}`)
  }
})
