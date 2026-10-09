#[test]
fn bundled_sqlite_has_fts5() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE VIRTUAL TABLE t USING fts5(body);
         INSERT INTO t(body) VALUES ('fixed the login button'), ('ran npm test');",
    )
    .unwrap();
    let hit: String = db
        .query_row("SELECT body FROM t WHERE t MATCH 'login'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(hit, "fixed the login button");
    let snippet: String = db
        .query_row(
            "SELECT snippet(t, 0, '[', ']', '…', 4) FROM t WHERE t MATCH 'npm'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(snippet.contains("[npm]"));
}
