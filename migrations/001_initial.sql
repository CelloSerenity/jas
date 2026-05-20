CREATE TABLE IF NOT EXISTS devices (
    id            TEXT NOT NULL PRIMARY KEY,
    name          TEXT NOT NULL,
    udid          TEXT NOT NULL UNIQUE,
    ip            TEXT NOT NULL,
    port          INTEGER NOT NULL DEFAULT 62078,
    pairing_blob  BLOB NOT NULL,
    last_seen     INTEGER,
    discovery     TEXT NOT NULL DEFAULT 'static',
    mdns_ip       TEXT,
    mdns_seen_at  INTEGER
);

CREATE TABLE IF NOT EXISTS apple_accounts (
    id           TEXT NOT NULL PRIMARY KEY,
    apple_id     TEXT NOT NULL UNIQUE,
    session_blob BLOB NOT NULL,
    team_id      TEXT,
    team_name    TEXT
);

CREATE TABLE IF NOT EXISTS apps (
    id              TEXT NOT NULL PRIMARY KEY,
    device_id       TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    bundle_id       TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    version         TEXT,
    ipa_path        TEXT,
    account_id      TEXT REFERENCES apple_accounts(id) ON DELETE SET NULL,
    installed_at    INTEGER,
    last_refreshed  INTEGER,
    refresh_enabled INTEGER NOT NULL DEFAULT 1,
    UNIQUE(device_id, bundle_id)
);

CREATE TABLE IF NOT EXISTS jobs (
    id          TEXT NOT NULL PRIMARY KEY,
    app_id      TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'queued',
    error       TEXT,
    started_at  INTEGER,
    finished_at INTEGER,
    progress    INTEGER NOT NULL DEFAULT 0,
    stage       TEXT
);

CREATE TABLE IF NOT EXISTS sideload_storage (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);
