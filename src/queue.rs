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
    let sql = "WITH rows AS (
                SELECT id FROM grpc.call_queue ORDER BY id LIMIT $1
            )
            DELETE FROM grpc.call_queue q
            USING rows
            WHERE q.id = rows.id
            RETURNING q.id, q.endpoint, q.method, q.request,
                    q.metadata, q.options, q.timeout_ms";

    // DELETE ... RETURNING atomically consumes the rows
    Spi::connect_mut(|client| {
        client
            .update(sql, None, &[batch_size.into()])
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
    if results.is_empty() {
        return;
    }

    let mut ids: Vec<i64> = Vec::with_capacity(results.len());
    let mut statuses: Vec<String> = Vec::with_capacity(results.len());
    let mut responses: Vec<Option<pgrx::JsonB>> = Vec::with_capacity(results.len());
    let mut error_msgs: Vec<Option<String>> = Vec::with_capacity(results.len());

    for r in results {
        ids.push(r.id);
        match r.outcome {
            CallOutcome::Success(v) => {
                statuses.push("SUCCESS".to_string());
                responses.push(Some(pgrx::JsonB(v)));
                error_msgs.push(None);
            }
            CallOutcome::Error(msg) => {
                statuses.push("ERROR".to_string());
                responses.push(None);
                error_msgs.push(Some(msg));
            }
        }
    }

    Spi::connect_mut(|client| {
        client
            .update(
                "INSERT INTO grpc._call_result (id, status, response, error_msg)
                 SELECT * FROM UNNEST($1::bigint[], $2::text[], $3::jsonb[], $4::text[])",
                None,
                &[
                    ids.into(),
                    statuses.into(),
                    responses.into(),
                    error_msgs.into(),
                ],
            )
            .unwrap();
    });
}

pub fn ttl_cleanup(ttl: &str) {
    Spi::connect_mut(|client| {
        if let Err(e) = client.update(
            "DELETE FROM grpc._call_result WHERE created < now() - $1::interval",
            None,
            &[ttl.into()],
        ) {
            pgrx::warning!("TTL cleanup skipped (pg_grpc.ttl = {:?}): {}", ttl, e);
        }
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
