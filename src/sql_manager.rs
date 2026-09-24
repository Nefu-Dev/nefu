//! SQL management system
//!
//! Provides access and control over SQLite databases.
//! Supports creating/connecting to databases, executing SQL statements, transaction management, etc.
//! Exposed to frontend JS through the bridge API.

use anyhow::{Context, Result};
use rusqlite::{Connection, params, types::ValueRef};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Mutex;

/// Database connection type
#[derive(Debug, Clone, PartialEq)]
pub enum DbType {
    /// SQLite database file
    SQLite,
}

/// Database connection manager
pub struct SqlManager {
    /// The currently active database connection
    current: Mutex<Option<Connection>>,
    /// Current database path/identifier
    current_path: Mutex<Option<String>>,
    /// Connection pool (caches multiple connections by name)
    connections: Mutex<HashMap<String, Connection>>,
}

impl SqlManager {
    /// Create a new SQL manager
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
            current_path: Mutex::new(None),
            connections: Mutex::new(HashMap::new()),
        }
    }

    /// Open/create a SQLite database file
    ///
    /// # Parameters
    /// - `path`: Database file path
    ///
    /// # Returns
    /// Success message
    pub fn open(&self, path: &str) -> Result<String> {
        let conn = Connection::open(path)
            .with_context(|| format!("Unable to open database: {}", path))?;
        
        // Enable WAL mode to improve concurrency performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .context("Failed to set WAL mode")?;
        
        // Enable foreign key constraints
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .context("Failed to enable foreign key constraints")?;

        let mut current = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        *current = Some(conn);
        
        let mut current_path = self.current_path.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        *current_path = Some(path.to_string());

        Ok(format!("Connected to database: {}", path))
    }

    /// Create an in-memory database
    pub fn open_memory(&self) -> Result<String> {
        let conn = Connection::open_in_memory()
            .context("Unable to create in-memory database")?;
        
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .context("Failed to enable foreign key constraints")?;

        let mut current = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        *current = Some(conn);
        
        let mut current_path = self.current_path.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        *current_path = Some(":memory:".to_string());

        Ok("In-memory database created".to_string())
    }

    /// Close the current database connection
    pub fn close(&self) -> Result<String> {
        let mut current = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        let mut path = self.current_path.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        *current = None;
        let db_path = path.take().unwrap_or_default();
        
        Ok(format!("Closed database connection: {}", db_path))
    }

    /// Execute a SQL query (SELECT) and return the result set
    ///
    /// # Parameters
    /// - `sql`: SQL query statement
    ///
    /// # Returns
    /// A JSON array, each row is an object { column: value }
    pub fn query(&self, sql: &str) -> Result<JsonValue> {
        let conn = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let conn = conn.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Not connected to a database, please call open() first")
        })?;

        let mut stmt = conn.prepare(sql)
            .with_context(|| format!("SQL prepare failed: {}", sql))?;

        let column_count = stmt.column_count();
        let column_names: Vec<String> = (0..column_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let rows = stmt.query_map([], |row| {
            let mut map = serde_json::Map::new();
            for i in 0..column_count {
                let name = &column_names[i];
                let value = row_to_json_value(row, i);
                map.insert(name.clone(), value);
            }
            Ok(JsonValue::Object(map))
        })?;

        let mut results = Vec::new();
        for row in rows {
            match row {
                Ok(val) => results.push(val),
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to read row data: {}", e));
                }
            }
        }

        Ok(JsonValue::Array(results))
    }

    /// Execute a SQL command (INSERT/UPDATE/DELETE/CREATE, etc.)
    ///
    /// # Parameters
    /// - `sql`: SQL command statement
    ///
    /// # Returns
    /// The number of affected rows
    pub fn execute(&self, sql: &str) -> Result<JsonValue> {
        let conn = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let conn = conn.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Not connected to a database, please call open() first")
        })?;

        let sql_upper = sql.trim().to_uppercase();
        let is_insert = sql_upper.starts_with("INSERT");

        let affected = conn.execute(sql, [])
            .with_context(|| format!("SQL execution failed: {}", sql))?;

        let mut result = serde_json::Map::new();
        result.insert("affected_rows".to_string(), JsonValue::Number(affected.into()));

        if is_insert {
            let last_id = conn.last_insert_rowid();
            result.insert("last_insert_rowid".to_string(), JsonValue::Number(last_id.into()));
        }

        Ok(JsonValue::Object(result))
    }

    /// Execute multiple SQL statements (separated by semicolons)
    pub fn execute_batch(&self, sql: &str) -> Result<JsonValue> {
        let conn = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let conn = conn.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Not connected to a database, please call open() first")
        })?;

        conn.execute_batch(sql)
            .with_context(|| format!("Batch SQL execution failed"))?;

        Ok(JsonValue::String("Batch execution succeeded".to_string()))
    }

    /// Begin a transaction
    pub fn begin_transaction(&self) -> Result<String> {
        let conn = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let conn = conn.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Not connected to a database")
        })?;

        conn.execute_batch("BEGIN TRANSACTION;")
            .context("Failed to begin transaction")?;

        Ok("Transaction started".to_string())
    }

    /// Commit a transaction
    pub fn commit(&self) -> Result<String> {
        let conn = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let conn = conn.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Not connected to a database")
        })?;

        conn.execute_batch("COMMIT;")
            .context("Failed to commit transaction")?;

        Ok("Transaction committed".to_string())
    }

    /// Roll back a transaction
    pub fn rollback(&self) -> Result<String> {
        let conn = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let conn = conn.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Not connected to a database")
        })?;

        conn.execute_batch("ROLLBACK;")
            .context("Failed to roll back transaction")?;

        Ok("Transaction rolled back".to_string())
    }

    /// Get database information (table list, version, etc.)
    pub fn get_info(&self) -> Result<JsonValue> {
        let conn = self.current.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let conn = match conn.as_ref() {
            Some(c) => c,
            None => {
                return Ok(serde_json::json!({
                    "connected": false,
                    "message": "Not connected to a database"
                }));
            }
        };

        // Get the SQLite version
        let version: String = conn.query_row(
            "SELECT sqlite_version()",
            [],
            |row| row.get(0)
        ).unwrap_or_default();

        // Get all tables
        let tables = self.query(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
        ).unwrap_or(JsonValue::Array(vec![]));

        let path = self.current_path.lock().ok()
            .and_then(|p| p.clone())
            .unwrap_or_default();

        Ok(serde_json::json!({
            "connected": true,
            "path": path,
            "version": version,
            "tables": tables
        }))
    }

    /// Get column information for a table
    pub fn get_table_info(&self, table_name: &str) -> Result<JsonValue> {
        let sql = format!("PRAGMA table_info({})", table_name);
        self.query(&sql)
    }

    /// Get the database list (all saved connections)
    pub fn list_connections(&self) -> Result<JsonValue> {
        let conns = self.connections.lock().map_err(|e| {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        })?;
        
        let names: Vec<JsonValue> = conns.keys()
            .map(|k| JsonValue::String(k.clone()))
            .collect();

        let current = self.current_path.lock().ok()
            .and_then(|p| p.clone())
            .unwrap_or_default();

        Ok(serde_json::json!({
            "connections": names,
            "current": current
        }))
    }
}

/// Convert a value from a rusqlite row into a JSON value
fn row_to_json_value(row: &rusqlite::Row, i: usize) -> JsonValue {
    match row.get_ref(i) {
        Ok(val) => match val {
            ValueRef::Null => JsonValue::Null,
            ValueRef::Integer(n) => JsonValue::Number(n.into()),
            ValueRef::Real(f) => {
                if let Some(n) = serde_json::Number::from_f64(f) {
                    JsonValue::Number(n)
                } else {
                    JsonValue::String(f.to_string())
                }
            }
            ValueRef::Text(s) => {
                match std::str::from_utf8(s) {
                    Ok(text) => JsonValue::String(text.to_string()),
                    Err(_) => JsonValue::String(format!("[non-UTF-8 text, {} bytes]", s.len())),
                }
            }
            ValueRef::Blob(b) => JsonValue::String(format!("[BLOB {} bytes]", b.len())),
        },
        Err(e) => JsonValue::String(format!("<error: {}>", e)),
    }
}
