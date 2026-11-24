//! Verify the library actually compiled into Lumi, not just Cargo advisory ranges.
//! SQLCipher may bundle an older SQLite than libsqlite3-sys's plain SQLite build.

#[test]
fn bundled_sqlcipher_uses_patched_sqlite() -> Result<(), Box<dyn std::error::Error>> {
    let connection = rusqlite::Connection::open_in_memory()?;
    let sqlite: String = connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
    let cipher: String = connection.query_row("PRAGMA cipher_version", [], |row| row.get(0))?;
    assert!(
        rusqlite::version_number() >= 3_054_000,
        "SQLite {sqlite} is below the CVE-2026-119375 security floor"
    );
    assert!(
        cipher.starts_with("4.20."),
        "Expected the reviewed SQLCipher 4.20 runtime, got {cipher}"
    );
    Ok(())
}
