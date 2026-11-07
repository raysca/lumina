# Search

The search field takes free text and `prefix:value` filters, separated by
spaces. Every part must match (AND). The grammar lives in
`crates/lumina-core/src/search/parse.rs`; the SQL in `search/mod.rs`.

## Free text

- `sunset kyoto` matches items whose file name, folder, place, camera, lens,
  year, tags or caption contain words starting with `sunset` and `kyoto`.
  Matching is by word prefix: `beach` also finds "beaches".
- `"two words"` keeps a phrase together.
- Text is always literal. `OR`, `NOT`, `NEAR`, `*`, `^` and stray quotes are
  searched as words, never as operators.
- A word with an unknown prefix, such as `foo:bar` or `12:30`, is searched as
  text, and `-foo:bar` excludes that text.

## Filters

| Filter | Meaning |
| --- | --- |
| `camera:Sony` (`make:`, `model:`) | Camera make or model contains the text |
| `lens:35mm` | Lens contains the text |
| `city:Kyoto`, `country:JP` | Place |
| `year:2026`, `month:sep` (or `9`) | Capture year and month |
| `date:2026-09-01`, `date:2026-09-01..2026-09-30`, `date:>=2026-09-01` | Capture date or range (`>`, `>=`, `<`, `<=`) |
| `iso:>800`, `f:<2.8` | ISO and aperture (`>`, `>=`, `<`, `<=`, or an exact value) |
| `rating:>=4` (`stars:`) | Star rating |
| `color:red` | Color label: `red`, `orange`, `yellow`, `green`, `blue`, `purple` (or `1`-`6`) |
| `tag:travel` (`keyword:`) | Has the tag (case-insensitive). Keywords are paths separated by `/`, and `tag:places` also matches sub-keywords such as `places/europe/paris`, but not `places-old` |
| `is:favorite` (`is:fav`, `is:favourite`) | Favorites |
| `is:video`, `is:photo` (`is:image`), `is:raw` | Media type. `is:photo` means non-RAW stills |
| `is:trashed` | Items in the trash (otherwise the trash is never searched) |
| `is:unrated` | No star rating |
| `is:untagged` | No tags |
| `is:unlabeled`, `color:none` (`color:0`) | No color label |
| `is:pick`, `is:reject`, `is:unflagged` | Culling flag: picked, rejected, or neither |
| `has:location`, `has:tag`, `has:color`, `has:rating` | Has GPS, any tag, any color label, any rating |

## Excluding

A leading `-` excludes:

- `-beach` hides items matching `beach`. Like inclusion this is a prefix
  match, so `-beach` also hides "beaches".
- `-"two words"` hides a phrase.
- `-tag:work` hides items tagged `work` or a sub-keyword such as `work/clients`.
- `-is:favorite` shows only non-favorites.
- `-has:location`, `-has:tag`, `-has:color` show items without GPS, tags or a
  color label.

A lone `-` (or `-""`) is ignored. Other negated filters, such as `-is:video`
or `-camera:Sony`, are not supported.

## Not understood

A filter the search cannot use is reported as not understood and has no
effect: an unknown value (`is:fovorite`, `color:pink`, `color:grey`,
`has:wings`), a value that does not parse (`rating:abc`, `year:20x6`,
`iso:lots`, `month:smarch`, `date:yesterday`), or an unsupported negation
(`-is:video`). These tokens are not searched as text either. In code they are
collected in `StructuredQuery::unrecognized`, and the explanation under the
search field lists them as "Not understood: …".

## Scope

Search always covers the whole library. When a query becomes non-empty,
whether typed, picked from the suggestions, run as a quick filter or set by a
link in Get Info, the sidebar moves to Library. Back (⌘[) returns to the album
you were in, with an empty search.

## Quick filters

The command palette (⌘K) has quick filters that edit the search text, so the
grid shows exactly what the query says. There is no separate filter state and
nothing is dimmed.

| Command | Token |
| --- | --- |
| Show Favorites | `is:favorite` |
| Show Videos | `is:video` |
| Show Rated ★ 1+ … ★ 5+ | `rating:>=1` … `rating:>=5` |
| Show Unrated | `is:unrated` |
| Show Color: Red … Purple | `color:red` … `color:purple` |
| Show Unlabeled | `is:unlabeled` |

- Running a quick filter whose token is already in the query (or one with the
  same effect, such as `is:fav`) removes it.
- Tokens of one kind replace each other: favorites (`is:favorite`,
  `-is:favorite`), ratings (`rating:`, `stars:`,
  `is:unrated`, `has:rating`), color labels (`color:`, `is:unlabeled`,
  `has:color`) and media types (`is:video`, `is:photo`, `is:raw`). Show Rated
  ★ 3+ replaces `is:unrated`.
- Clear Filters removes every filter token, negated and not-understood ones
  included, and keeps free text and `-word` exclusions. It is listed only
  when the query has a filter.
- The palette shows 8 rows, so typing "filter" alone does not list the color
  filters or Clear Filters; type "color red" or "clear filters".

## Explanation

While the search field is focused, the bottom of the suggestion list says in
plain words what the query searches for, for example
`Favorites · Camera contains “Sony” · ISO 801 or higher`. It stays visible
after Escape hides the suggestions, until the field loses focus. The phrases
are English only, and a country code or tag is shown the way it is matched
(country upper case, tags lower case), not as typed.

## Recent searches

Pressing Enter in the search field records the query as a recent search (up
to 10, newest first, duplicates merged regardless of case). They are kept per
machine in the app settings and shown first in the suggestion list, marked
with a history icon. Command palette (⌘K) > Clear Recent Searches forgets
them; it is listed only when there are some.

## Saved searches

Command palette (⌘K) > Save Search as Smart Album… opens the smart album
builder with one rule that holds the search text exactly as typed, titled with
the query (cut to 60 characters). The preview counts the same items the search
shows; OK saves the album and opens it. The command is listed only while the
search field has text.

The album runs the search again each time it is opened, so new items that
match appear in it. The builder refuses a search it cannot save and says why
in its footer:

- `is:trashed`: a smart album cannot show deleted items.
- Anything listed under "Not understood", such as a misspelled filter.
- Text with nothing to search for.

Limitation: renaming a tag updates smart album Tag rules, but not a `tag:`
filter inside a saved search's text. Edit the saved search after renaming a
tag it uses.

Limitation: an older Lumina build does not know the saved-search rule. It
shows such an album as the whole library, and editing the album there
replaces the saved search.

Limitation: a saved search for a rare word finds its first page by walking the
timeline, so it slows with library size (under 1 ms at 25,000 items, about
8 ms near 240,000). Counts are not affected.
