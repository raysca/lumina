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
| `tag:travel` (`keyword:`) | Has the tag (case-insensitive) |
| `is:favorite` (`is:fav`, `is:favourite`) | Favorites |
| `is:video`, `is:photo` (`is:image`), `is:raw` | Media type. `is:photo` means non-RAW stills |
| `is:trashed` | Items in the trash (otherwise the trash is never searched) |
| `is:unrated` | No star rating |
| `is:untagged` | No tags |
| `is:unlabeled`, `color:none` (`color:0`) | No color label |
| `has:location`, `has:tag`, `has:color`, `has:rating` | Has GPS, any tag, any color label, any rating |

## Excluding

A leading `-` excludes:

- `-beach` hides items matching `beach`. Like inclusion this is a prefix
  match, so `-beach` also hides "beaches".
- `-"two words"` hides a phrase.
- `-tag:work` hides items tagged `work`.
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

## Explanation

While the search field is focused, the bottom of the suggestion list says in
plain words what the query searches for, for example
`Favorites · Camera contains “Sony” · ISO 801 or higher`. It stays visible
after Escape hides the suggestions, until the field loses focus.

## Recent searches

Pressing Enter in the search field records the query as a recent search (up
to 10, newest first, duplicates merged regardless of case). They are kept per
machine in the app settings and shown first in the suggestion list, marked
with a history icon. Command palette (⌘K) > Clear Recent Searches forgets
them; it is listed only when there are some.
