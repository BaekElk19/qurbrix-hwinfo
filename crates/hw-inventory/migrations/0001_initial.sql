PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migration (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS hardware_snapshot (
    snapshot_id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    scan_status TEXT NOT NULL CHECK (scan_status IN ('complete', 'partial')),
    schema_version TEXT NOT NULL,
    scanner_version TEXT,
    machine_bind_id TEXT NOT NULL,
    bindid_algorithm TEXT NOT NULL,
    configuration_fingerprint TEXT NOT NULL,
    device_count INTEGER NOT NULL CHECK (device_count >= 0),
    warning_count INTEGER NOT NULL CHECK (warning_count >= 0),
    duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0)
) STRICT;

CREATE TABLE IF NOT EXISTS inventory_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_snapshot_id TEXT REFERENCES hardware_snapshot(snapshot_id),
    current_machine_bind_id TEXT,
    bindid_algorithm TEXT,
    last_configuration_fingerprint TEXT,
    core_identity_count INTEGER CHECK (core_identity_count IS NULL OR core_identity_count >= 0),
    fingerprint_version INTEGER,
    last_quick_probe_at TEXT,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_device (
    snapshot_id TEXT NOT NULL REFERENCES hardware_snapshot(snapshot_id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    vendor TEXT,
    model TEXT,
    serial TEXT,
    bus_kind TEXT,
    bus_address TEXT,
    driver_name TEXT,
    driver_status TEXT,
    parent_device_id TEXT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (snapshot_id, device_id),
    FOREIGN KEY (snapshot_id, parent_device_id)
        REFERENCES snapshot_device(snapshot_id, device_id)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_device_identifier (
    snapshot_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    identifier_kind TEXT NOT NULL,
    identifier_value TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (snapshot_id, device_id, identifier_kind, identifier_value),
    FOREIGN KEY (snapshot_id, device_id)
        REFERENCES snapshot_device(snapshot_id, device_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_device_property (
    snapshot_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    property_key TEXT NOT NULL,
    value_type TEXT NOT NULL CHECK (value_type IN ('text', 'integer', 'real', 'boolean')),
    text_value TEXT,
    integer_value INTEGER,
    real_value REAL,
    boolean_value INTEGER CHECK (boolean_value IS NULL OR boolean_value IN (0, 1)),
    unit TEXT,
    ordinal INTEGER NOT NULL DEFAULT 0 CHECK (ordinal >= 0),
    CHECK (
        (value_type = 'text' AND text_value IS NOT NULL AND integer_value IS NULL AND real_value IS NULL AND boolean_value IS NULL) OR
        (value_type = 'integer' AND text_value IS NULL AND integer_value IS NOT NULL AND real_value IS NULL AND boolean_value IS NULL) OR
        (value_type = 'real' AND text_value IS NULL AND integer_value IS NULL AND real_value IS NOT NULL AND boolean_value IS NULL) OR
        (value_type = 'boolean' AND text_value IS NULL AND integer_value IS NULL AND real_value IS NULL AND boolean_value IS NOT NULL)
    ),
    PRIMARY KEY (snapshot_id, device_id, property_key, ordinal),
    FOREIGN KEY (snapshot_id, device_id)
        REFERENCES snapshot_device(snapshot_id, device_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_device_relation (
    snapshot_id TEXT NOT NULL,
    source_device_id TEXT NOT NULL,
    relation_kind TEXT NOT NULL,
    target_device_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (snapshot_id, source_device_id, relation_kind, target_device_id),
    FOREIGN KEY (snapshot_id, source_device_id)
        REFERENCES snapshot_device(snapshot_id, device_id) ON DELETE CASCADE,
    FOREIGN KEY (snapshot_id, target_device_id)
        REFERENCES snapshot_device(snapshot_id, device_id) ON DELETE CASCADE
        DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_warning (
    warning_id INTEGER PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES hardware_snapshot(snapshot_id) ON DELETE CASCADE,
    device_id TEXT,
    code TEXT NOT NULL,
    message TEXT NOT NULL,
    source TEXT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    FOREIGN KEY (snapshot_id, device_id)
        REFERENCES snapshot_device(snapshot_id, device_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_source (
    source_id INTEGER PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES hardware_snapshot(snapshot_id) ON DELETE CASCADE,
    device_id TEXT,
    source TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    source_status TEXT NOT NULL,
    summary TEXT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    FOREIGN KEY (snapshot_id, device_id)
        REFERENCES snapshot_device(snapshot_id, device_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS probe_history (
    probe_id INTEGER PRIMARY KEY,
    probe_type TEXT NOT NULL CHECK (probe_type IN ('quick', 'full')),
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'partial', 'failed')),
    snapshot_id TEXT REFERENCES hardware_snapshot(snapshot_id),
    previous_snapshot_id TEXT REFERENCES hardware_snapshot(snapshot_id),
    machine_bind_id TEXT,
    configuration_fingerprint TEXT,
    duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
    warning_count INTEGER CHECK (warning_count IS NULL OR warning_count >= 0),
    error_code TEXT,
    error_message TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_artifact (
    snapshot_id TEXT PRIMARY KEY REFERENCES hardware_snapshot(snapshot_id) ON DELETE CASCADE,
    artifact_kind TEXT NOT NULL CHECK (artifact_kind = 'scan_report_json'),
    relative_path TEXT NOT NULL UNIQUE,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    schema_version TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot_lifecycle (
    snapshot_id TEXT PRIMARY KEY REFERENCES hardware_snapshot(snapshot_id) ON DELETE CASCADE,
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    uploaded_at TEXT,
    delete_pending INTEGER NOT NULL DEFAULT 0 CHECK (delete_pending IN (0, 1)),
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS scan_lease (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    owner_id TEXT NOT NULL,
    acquired_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS artifact_delete_queue (
    relative_path TEXT PRIMARY KEY,
    sha256 TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_snapshot_machine_created
    ON hardware_snapshot(machine_bind_id, created_at DESC, snapshot_id DESC);
CREATE INDEX IF NOT EXISTS idx_snapshot_configuration
    ON hardware_snapshot(configuration_fingerprint);
CREATE INDEX IF NOT EXISTS idx_device_snapshot_kind
    ON snapshot_device(snapshot_id, kind, ordinal);
CREATE INDEX IF NOT EXISTS idx_device_identifier_lookup
    ON snapshot_device_identifier(identifier_kind, identifier_value);
CREATE INDEX IF NOT EXISTS idx_device_property_lookup
    ON snapshot_device_property(property_key, text_value, integer_value, real_value);
CREATE INDEX IF NOT EXISTS idx_warning_snapshot
    ON snapshot_warning(snapshot_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_source_snapshot
    ON snapshot_source(snapshot_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_probe_started
    ON probe_history(started_at DESC, probe_id DESC);
CREATE INDEX IF NOT EXISTS idx_lifecycle_retention
    ON snapshot_lifecycle(pinned, uploaded_at, delete_pending);
CREATE INDEX IF NOT EXISTS idx_delete_queue_updated
    ON artifact_delete_queue(updated_at);

CREATE TRIGGER IF NOT EXISTS immutable_hardware_snapshot
BEFORE UPDATE ON hardware_snapshot BEGIN
    SELECT RAISE(ABORT, 'published snapshot is immutable');
END;
CREATE TRIGGER IF NOT EXISTS immutable_snapshot_device
BEFORE UPDATE ON snapshot_device BEGIN
    SELECT RAISE(ABORT, 'published device is immutable');
END;
CREATE TRIGGER IF NOT EXISTS immutable_snapshot_identifier
BEFORE UPDATE ON snapshot_device_identifier BEGIN
    SELECT RAISE(ABORT, 'published identifier is immutable');
END;
CREATE TRIGGER IF NOT EXISTS immutable_snapshot_property
BEFORE UPDATE ON snapshot_device_property BEGIN
    SELECT RAISE(ABORT, 'published property is immutable');
END;
CREATE TRIGGER IF NOT EXISTS immutable_snapshot_relation
BEFORE UPDATE ON snapshot_device_relation BEGIN
    SELECT RAISE(ABORT, 'published relation is immutable');
END;

INSERT OR IGNORE INTO inventory_state(id, updated_at)
VALUES (1, '1970-01-01T00:00:00Z');
