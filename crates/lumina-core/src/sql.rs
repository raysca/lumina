//! SQL fragments shared across query builders.
//!
//! A neutral home (rather than `catalog`) so `search` and `smart_albums` can
//! share the media column list without importing the catalog.

/// The media SELECT list `map_media_item_row` reads, in its positional order.
/// Shared by every builder (catalog, search, smart albums) so a schema change
/// cannot silently misalign one copy against the mapper.
pub(crate) fn media_columns(prefix: &str) -> String {
    format!(
        "{p}id, {p}file_path, {p}parent_dir, {p}file_name, {p}file_size, {p}mtime_epoch_ms, \
         {p}media_type, {p}width, {p}height, {p}duration_ms, \
         {p}timestamp_epoch_ms, {p}year, {p}month, {p}day, \
         {p}latitude, {p}longitude, {p}altitude, \
         {p}country_code, {p}admin1_region, {p}city, \
         {p}make, {p}model, {p}lens, {p}focal_length, {p}f_number, {p}iso, {p}exposure_time, \
         {p}is_favorite, {p}is_hidden, {p}is_trashed, {p}title",
        p = prefix
    )
}
