use pgrx::prelude::*;
use serde_json::Value as JsonValue;

pub struct QueueRow {
    pub id: i64,
    pub endpoint: String,
    pub method: String,
    pub request: JsonValue,
    pub metadata: Option<JsonValue>,
    pub options: Option<JsonValue>,
    pub timeout_ms: i32,
}

pub enum CallOutcome {
    Success(JsonValue),
    Error(String),
}

pub struct CallResult {
    pub id: i64,
    pub outcome: CallOutcome,
}

pub enum LookupStatus {
    Pending,
    Success(JsonValue),
    Error(String),
}

pub struct LookupResult {
    pub id: i64,
    pub status: LookupStatus,
}

pub fn dequeue(batch_size: i32) -> Vec<QueueRow> {
    // Modifying CTE via select — UPDATE inside a CTE is valid SQL and works with &SpiClient
    let sql = format!(
        "WITH updated AS (
             UPDATE grpc.call_queue SET status = 'processing'
             WHERE id IN (
                 SELECT id FROM grpc.call_queue
                 WHERE status = 'pending'
                 ORDER BY id
                 LIMIT {batch_size}
             )
             RETURNING id, endpoint, method, request, metadata, options, timeout_ms
         )
         SELECT * FROM updated"
    );

    Spi::connect(|client| {
        client
            .select(&sql, None, &[])
            .unwrap()
            .map(|row| QueueRow {
                id: row["id"].value::<i64>().unwrap().unwrap(),
                endpoint: row["endpoint"].value::<String>().unwrap().unwrap(),
                method: row["method"].value::<String>().unwrap().unwrap(),
                request: jsonb_to_value(row["request"].value::<pgrx::JsonB>().unwrap()),
                metadata: row["metadata"]
                    .value::<pgrx::JsonB>()
                    .unwrap()
                    .map(|j| serde_json::from_str(&j.0.to_string()).unwrap()),
                options: row["options"]
                    .value::<pgrx::JsonB>()
                    .unwrap()
                    .map(|j| serde_json::from_str(&j.0.to_string()).unwrap()),
                timeout_ms: row["timeout_ms"].value::<i32>().unwrap().unwrap(),
            })
            .collect()
    })
}

pub fn insert_results(results: Vec<CallResult>) {
    Spi::connect_mut(|client| {
        for r in &results {
            match &r.outcome {
                CallOutcome::Success(response) => {
                    client
                        .update(
                            "INSERT INTO grpc._call_result (id, status, response)
                             VALUES ($1, 'SUCCESS', $2)",
                            None,
                            &[r.id.into(), pgrx::JsonB(response.clone()).into()],
                        )
                        .unwrap();
                }
                CallOutcome::Error(msg) => {
                    client
                        .update(
                            "INSERT INTO grpc._call_result (id, status, error_msg)
                             VALUES ($1, 'ERROR', $2)",
                            None,
                            &[r.id.into(), msg.as_str().into()],
                        )
                        .unwrap();
                }
            }
        }
        let ids: Vec<String> = results.iter().map(|r| r.id.to_string()).collect();
        if !ids.is_empty() {
            let sql = format!(
                "DELETE FROM grpc.call_queue WHERE id IN ({})",
                ids.join(",")
            );
            client.update(&sql, None, &[]).unwrap();
        }
    });
}

pub fn ttl_cleanup(ttl: &str) {
    Spi::connect_mut(|client| {
        client
            .update(
                "DELETE FROM grpc._call_result WHERE created < now() - $1::interval",
                None,
                &[ttl.into()],
            )
            .unwrap_or_else(|e| {
                pgrx::error!("TTL cleanup failed (pg_grpc.ttl = {:?}): {}", ttl, e)
            });
    });
}

pub fn lookup(id: i64) -> LookupResult {
    Spi::connect(|client| {
        // Check _call_result first
        let result_rows: Vec<_> = client
            .select(
                "SELECT status, response, error_msg FROM grpc._call_result WHERE id = $1",
                None,
                &[id.into()],
            )
            .unwrap()
            .collect();

        if let Some(row) = result_rows.into_iter().next() {
            let status = row["status"].value::<&str>().unwrap().unwrap();
            return match status {
                "SUCCESS" => LookupResult {
                    id,
                    status: LookupStatus::Success(jsonb_to_value(
                        row["response"].value::<pgrx::JsonB>().unwrap(),
                    )),
                },
                _ => LookupResult {
                    id,
                    status: LookupStatus::Error(
                        row["error_msg"]
                            .value::<String>()
                            .unwrap()
                            .unwrap_or_default(),
                    ),
                },
            };
        }

        // Check call_queue (pending or processing = still in-flight)
        let in_queue = client
            .select(
                "SELECT count(*) FROM grpc.call_queue WHERE id = $1",
                None,
                &[id.into()],
            )
            .unwrap()
            .first()
            .get_one::<i64>()
            .unwrap()
            .unwrap_or(0);

        if in_queue > 0 {
            return LookupResult {
                id,
                status: LookupStatus::Pending,
            };
        }

        LookupResult {
            id,
            status: LookupStatus::Error("not found — result may have expired".to_string()),
        }
    })
}

fn jsonb_to_value(v: Option<pgrx::JsonB>) -> JsonValue {
    v.map(|j| serde_json::from_str(&j.0.to_string()).unwrap())
        .unwrap_or(JsonValue::Null)
}
