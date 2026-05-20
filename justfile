db := "/tmp/jas-sqlx-prepare.db"
export DATABASE_URL := "sqlite://" + db

# Rebuild the .sqlx offline query cache. Run after changing any sqlx::query! macro.
sqlx-prepare:
    rm -f {{db}}
    cargo sqlx database create
    cargo sqlx migrate run
    cargo sqlx prepare -- --features ssr --no-default-features --bin jas
