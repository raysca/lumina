mod rules;
mod system;

pub use rules::{
    from_portable_json, parse_rule_date, split_date_range, to_portable_json, MatchMode,
    PortableSmartAlbum, RuleField, RuleOperator, RuleValue, SmartAlbum, SmartAlbumRule,
    MAX_PORTABLE_BYTES,
};
pub use system::SystemAlbum;

#[cfg(test)]
mod tests {
    use super::rules::{MAX_PORTABLE_BYTES, MAX_PORTABLE_RULES, MAX_PORTABLE_TEXT_CHARS};
    use super::*;

    #[test]
    fn test_system_albums_sql_generation() {
        for album in SystemAlbum::all() {
            let (query_sql, params) = album.synthesize_query(50, 0);
            assert!(query_sql.contains("LIMIT ?1 OFFSET ?2"));
            assert_eq!(params.len(), 2);

            let (count_sql, count_params) = album.synthesize_count_query();
            assert!(count_sql.contains("SELECT COUNT(*)"));
            assert_eq!(count_params.len(), 0);
        }
    }

    #[test]
    fn test_smart_album_rule_compilation() {
        let rules = vec![
            SmartAlbumRule {
                field: RuleField::IsFavorite,
                operator: RuleOperator::Equals,
                value: RuleValue::Bool(true),
            },
            SmartAlbumRule {
                field: RuleField::CameraMake,
                operator: RuleOperator::Contains,
                value: RuleValue::Text("Sony".to_string()),
            },
            SmartAlbumRule {
                field: RuleField::Iso,
                operator: RuleOperator::GreaterThan,
                value: RuleValue::Integer(800),
            },
        ];

        let album = SmartAlbum::new(
            "High ISO Sony Favorites".to_string(),
            "heart".to_string(),
            MatchMode::All,
            rules,
        );

        let (sql, params) = album.synthesize_query(20, 0).unwrap();
        assert!(sql.contains("m.is_favorite = ?"));
        assert!(sql.contains("m.make LIKE ?"));
        assert!(sql.contains("m.iso > ?"));
        assert_eq!(params.len(), 5); // 3 rule params + limit + offset
    }

    #[test]
    fn test_smart_album_in_the_last_operator() {
        let rule = SmartAlbumRule {
            field: RuleField::Date,
            operator: RuleOperator::InTheLast,
            value: RuleValue::Text("30d".to_string()),
        };

        let album = SmartAlbum::new(
            "Recent 30 Days".to_string(),
            "clock".to_string(),
            MatchMode::All,
            vec![rule],
        );

        let (where_clause, params) = album.build_sql().unwrap();
        assert!(where_clause.contains("m.timestamp_epoch_ms >= ?"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn in_the_last_accepts_weeks_and_months() {
        let cutoff = |text: &str| {
            let album = SmartAlbum::new(
                String::new(),
                String::new(),
                MatchMode::All,
                vec![SmartAlbumRule {
                    field: RuleField::Date,
                    operator: RuleOperator::InTheLast,
                    value: RuleValue::Text(text.to_string()),
                }],
            );
            match album.build_sql().expect("compiles").1[0] {
                rusqlite::types::Value::Integer(ms) => ms,
                ref other => panic!("cutoff is an integer, got {other:?}"),
            }
        };
        let day = 86_400_000;
        let near = |a: i64, b: i64| (a - b).abs() < 60_000;
        assert!(near(cutoff("2d") - cutoff("2w"), 12 * day));
        assert!(
            near(cutoff("1w") - cutoff("1mo"), 23 * day),
            "mo is months (30 days), not minutes"
        );
    }

    #[test]
    fn test_smart_album_tag_rules() {
        // Tag Equals
        let rule_eq = SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::Equals,
            value: RuleValue::Text("Nature".to_string()),
        };
        let album_eq = SmartAlbum::new(
            "Nature Photos".to_string(),
            "tag".to_string(),
            MatchMode::All,
            vec![rule_eq],
        );
        let (sql_eq, params_eq) = album_eq.build_sql().unwrap();
        assert!(
            sql_eq.contains("m.id IN (SELECT media_id FROM media_tags WHERE (tag >= ?"),
            "Tag Equals is a subtree test: {sql_eq}"
        );
        assert_eq!(
            params_eq,
            vec![rusqlite::types::Value::Text("nature".to_string()); 4]
        );

        // Tag NotEquals
        let rule_neq = SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::NotEquals,
            value: RuleValue::Text("Work".to_string()),
        };
        let album_neq = SmartAlbum::new(
            "Non-Work Photos".to_string(),
            "tag".to_string(),
            MatchMode::All,
            vec![rule_neq],
        );
        let (sql_neq, params_neq) = album_neq.build_sql().unwrap();
        assert!(
            sql_neq.contains("m.id NOT IN (SELECT media_id FROM media_tags WHERE (tag >= ?"),
            "Tag NotEquals is a subtree test: {sql_neq}"
        );
        assert_eq!(
            params_neq,
            vec![rusqlite::types::Value::Text("work".to_string()); 4]
        );

        // Tag Contains
        let rule_contains = SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::Contains,
            value: RuleValue::Text("Trip".to_string()),
        };
        let album_contains = SmartAlbum::new(
            "Trip Photos".to_string(),
            "tag".to_string(),
            MatchMode::All,
            vec![rule_contains],
        );
        let (sql_contains, params_contains) = album_contains.build_sql().unwrap();
        assert!(sql_contains
            .contains("m.id IN (SELECT media_id FROM media_tags WHERE tag LIKE ? ESCAPE '\\')"));
        assert_eq!(
            params_contains[0],
            rusqlite::types::Value::Text("%trip%".to_string())
        );

        // Tag IsEmpty (Untagged media)
        let rule_empty = SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::IsEmpty,
            value: RuleValue::Text("".to_string()),
        };
        let album_empty = SmartAlbum::new(
            "Untagged Photos".to_string(),
            "tag".to_string(),
            MatchMode::All,
            vec![rule_empty],
        );
        let (sql_empty, params_empty) = album_empty.build_sql().unwrap();
        assert!(sql_empty.contains("m.id NOT IN (SELECT media_id FROM media_tags)"));
        assert_eq!(params_empty.len(), 0);

        // Tag IsNotEmpty (Any tagged media)
        let rule_not_empty = SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::IsNotEmpty,
            value: RuleValue::Text("".to_string()),
        };
        let album_not_empty = SmartAlbum::new(
            "All Tagged Photos".to_string(),
            "tag".to_string(),
            MatchMode::All,
            vec![rule_not_empty],
        );
        let (sql_not_empty, params_not_empty) = album_not_empty.build_sql().unwrap();
        assert!(sql_not_empty.contains("m.id IN (SELECT media_id FROM media_tags)"));
        assert_eq!(params_not_empty.len(), 0);
    }

    fn query_album(text: &str) -> SmartAlbum {
        SmartAlbum::new(
            String::new(),
            String::new(),
            MatchMode::All,
            vec![SmartAlbumRule {
                field: RuleField::Query,
                operator: RuleOperator::Equals,
                value: RuleValue::Text(text.to_string()),
            }],
        )
    }

    #[test]
    fn query_rule_rejects_trashed_unknown_and_empty() {
        let err = |text: &str| {
            query_album(text)
                .build_sql()
                .expect_err(&format!("{text:?} must not compile"))
                .to_string()
        };
        assert_eq!(
            err("sunset is:trashed"),
            "A smart album cannot show deleted items"
        );
        assert_eq!(
            err("is:fovorite color:pink"),
            "Not understood: is:fovorite, color:pink"
        );
        assert_eq!(err("   "), "A saved search needs some search text");
        let other_operator = SmartAlbum::new(
            String::new(),
            String::new(),
            MatchMode::All,
            vec![SmartAlbumRule {
                field: RuleField::Query,
                operator: RuleOperator::Contains,
                value: RuleValue::Text("sunset".into()),
            }],
        );
        assert!(
            other_operator.build_sql().is_err(),
            "a Query rule supports only Equals"
        );
        let (sql, _) = query_album("sunset rating:>=3")
            .build_sql()
            .expect("a valid query compiles");
        assert!(
            sql.contains(
                "+m.id IN (SELECT rowid FROM media_search_fts WHERE media_search_fts MATCH ?)"
            ),
            "the FTS match is an IN subquery, got {sql}"
        );
    }

    #[test]
    fn query_rule_round_trips_rules_json() {
        let album = query_album("sunset -beach camera:\"Sony A7\" rating:>=3");
        let json = serde_json::to_string(&album.rules).expect("serialize rules");
        assert!(
            json.contains(r#""field":"query""#),
            "the field is stored as snake_case query, got {json}"
        );
        let back: Vec<SmartAlbumRule> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, album.rules, "the raw query text survives rules_json");
    }

    /// One rule per field. The match is exhaustive, so a new `RuleField`
    /// does not compile until it is added here and to the round trip.
    fn rule_for(field: RuleField) -> SmartAlbumRule {
        use RuleOperator as Op;
        use RuleValue as V;
        let (operator, value) = match field {
            RuleField::Tag => (Op::Equals, V::Text("trips/kyoto".into())),
            RuleField::MediaType => (Op::Equals, V::Text("video".into())),
            RuleField::IsFavorite => (Op::Equals, V::Bool(true)),
            RuleField::Year => (Op::Between, V::Range(2019.0, 2021.0)),
            RuleField::Month => (Op::Equals, V::Number(7.0)),
            RuleField::City => (Op::Contains, V::Text("Kyoto".into())),
            RuleField::CountryCode => (Op::NotEquals, V::Text("JP".into())),
            RuleField::CameraMake => (Op::Contains, V::Text("Sony".into())),
            RuleField::CameraModel => (Op::NotContains, V::Text("A7".into())),
            RuleField::Lens => (Op::IsNotEmpty, V::Text(String::new())),
            RuleField::Iso => (Op::GreaterThan, V::Number(800.0)),
            RuleField::FNumber => (Op::LessThan, V::Number(2.8)),
            RuleField::FocalLength => (Op::AtLeast, V::Number(35.0)),
            RuleField::Date => (Op::InTheLast, V::Text("2w".into())),
            RuleField::Filename => (Op::Contains, V::Text("IMG_".into())),
            RuleField::FileSize => (Op::AtMost, V::Number(1_000_000.0)),
            RuleField::DurationMs => (Op::GreaterThan, V::Number(60_000.0)),
            RuleField::Rating => (Op::AtLeast, V::Number(3.0)),
            RuleField::ColorLabel => (Op::Equals, V::Text("red".into())),
            RuleField::Place => (Op::Contains, V::Text("Japan".into())),
            RuleField::Camera => (Op::NotContains, V::Text("iPhone".into())),
            RuleField::Query => (Op::Equals, V::Text("sunset rating:>=3".into())),
            RuleField::Flag => (Op::NotEquals, V::Number(-1.0)),
        };
        SmartAlbumRule {
            field,
            operator,
            value,
        }
    }

    #[test]
    fn portable_json_round_trips_every_rule_field() {
        let fields = [
            RuleField::Tag,
            RuleField::MediaType,
            RuleField::IsFavorite,
            RuleField::Year,
            RuleField::Month,
            RuleField::City,
            RuleField::CountryCode,
            RuleField::CameraMake,
            RuleField::CameraModel,
            RuleField::Lens,
            RuleField::Iso,
            RuleField::FNumber,
            RuleField::FocalLength,
            RuleField::Date,
            RuleField::Filename,
            RuleField::FileSize,
            RuleField::DurationMs,
            RuleField::Rating,
            RuleField::ColorLabel,
            RuleField::Place,
            RuleField::Camera,
            RuleField::Query,
            RuleField::Flag,
        ];
        let mut album = SmartAlbum::new(
            "Kyoto “trip”".into(),
            "folder".into(),
            MatchMode::Any,
            fields.into_iter().map(rule_for).collect(),
        );
        album.id = 42;
        album.pinned = true;
        album
            .build_sql()
            .expect("every sample rule compiles, so the import check passes");
        let json = to_portable_json(&album).expect("export");
        assert!(
            json.contains(r#""lumina_smart_album": 1"#),
            "the file names its format and version: {json}"
        );
        assert!(
            !json.contains("pinned") && !json.contains("\"id\""),
            "the pin and the catalog id stay in this library: {json}"
        );
        let back = from_portable_json(&json).expect("import");
        assert_eq!(back.title, album.title);
        assert_eq!(back.icon_name, album.icon_name);
        assert_eq!(back.match_mode, MatchMode::Any);
        assert_eq!(
            back.rules, album.rules,
            "every rule field survives the file"
        );
    }

    #[test]
    fn portable_json_rejects_unknown_version() {
        let album = SmartAlbum::new(
            "Faves".into(),
            "folder".into(),
            MatchMode::All,
            vec![rule_for(RuleField::IsFavorite)],
        );
        let json = to_portable_json(&album).expect("export");
        let newer = json.replace(r#""lumina_smart_album": 1"#, r#""lumina_smart_album": 2"#);
        let err = from_portable_json(&newer).expect_err("a newer format is refused");
        assert!(
            err.to_string().contains("newer version of Lumina"),
            "the error says why: {err}"
        );
        let err = from_portable_json(r#"{"title": "x"}"#).expect_err("no version, no import");
        assert!(
            err.to_string().contains("not a Lumina smart album"),
            "the error says why: {err}"
        );
        let unknown_field = json.replace(r#""field": "is_favorite""#, r#""field": "sharpness""#);
        assert_ne!(unknown_field, json, "the sample has an is_favorite rule");
        assert!(
            from_portable_json(&unknown_field).is_err(),
            "a rule field this build does not know is refused"
        );
        let extra_key = json.replacen('{', r#"{"pinned": true, "#, 1);
        assert!(
            from_portable_json(&extra_key).is_err(),
            "an unknown top-level setting is refused, not dropped"
        );
        let extra_rule_key = json.replace(
            r#""field": "is_favorite","#,
            r#""field": "is_favorite", "negate": true,"#,
        );
        assert_ne!(extra_rule_key, json, "the sample has an is_favorite rule");
        let err = from_portable_json(&extra_rule_key)
            .expect_err("an unknown key inside a rule is refused, not dropped");
        assert!(
            err.to_string().contains("does not know") && !err.to_string().contains("negate"),
            "a plain sentence, not the parser's detail: {err}"
        );
        let bad_rule = json.replace(r#""field": "is_favorite""#, r#""field": "query""#);
        let err = from_portable_json(&bad_rule).expect_err("rules that do not compile are refused");
        let compile_err = SmartAlbum::new(
            String::new(),
            String::new(),
            MatchMode::All,
            vec![SmartAlbumRule {
                field: RuleField::Query,
                operator: RuleOperator::Equals,
                value: RuleValue::Bool(true),
            }],
        )
        .build_sql()
        .expect_err("a query rule needs text");
        assert_eq!(
            err.to_string(),
            compile_err.to_string(),
            "the import reports the builder's own compile error"
        );
    }

    fn album_with(rules: Vec<SmartAlbumRule>) -> String {
        to_portable_json(&SmartAlbum::new(
            "Big".into(),
            "folder".into(),
            MatchMode::Any,
            rules,
        ))
        .expect("export")
    }

    #[test]
    fn portable_json_refuses_oversized_input() {
        let many = album_with(vec![
            rule_for(RuleField::IsFavorite);
            MAX_PORTABLE_RULES + 1
        ]);
        let err = from_portable_json(&many).expect_err("too many rules");
        assert!(
            err.to_string().contains("more than 100 rules"),
            "the error says why: {err}"
        );
        assert!(
            from_portable_json(&album_with(vec![
                rule_for(RuleField::IsFavorite);
                MAX_PORTABLE_RULES
            ]))
            .is_ok(),
            "exactly the limit is fine"
        );

        let long = SmartAlbumRule {
            field: RuleField::Filename,
            operator: RuleOperator::Contains,
            value: RuleValue::Text("x".repeat(MAX_PORTABLE_TEXT_CHARS + 1)),
        };
        let err = from_portable_json(&album_with(vec![long])).expect_err("too long a value");
        assert!(
            err.to_string().contains("too long"),
            "the error says why: {err}"
        );

        let huge = format!(
            "{}{}",
            album_with(vec![rule_for(RuleField::IsFavorite)]),
            " ".repeat(MAX_PORTABLE_BYTES)
        );
        let err = from_portable_json(&huge).expect_err("too large a file");
        assert!(
            err.to_string().contains("too large"),
            "the error says why: {err}"
        );
    }
}
