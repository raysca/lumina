mod rules;
mod system;

pub use rules::{
    parse_rule_date, split_date_range, MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbum,
    SmartAlbumRule,
};
pub use system::SystemAlbum;

#[cfg(test)]
mod tests {
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
        assert!(sql_eq.contains("m.id IN (SELECT media_id FROM media_tags WHERE tag = ?)"));
        assert_eq!(
            params_eq[0],
            rusqlite::types::Value::Text("nature".to_string())
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
        assert!(sql_neq.contains("m.id NOT IN (SELECT media_id FROM media_tags WHERE tag = ?)"));
        assert_eq!(
            params_neq[0],
            rusqlite::types::Value::Text("work".to_string())
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
}
