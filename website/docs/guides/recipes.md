---
title: Recipes
sidebar_position: 4
---

# Recipes

Patterns for the three places `pg_grpc` shines: triggers, scheduled jobs, and batch enrichment. Each recipe names the gotcha you need to know before cargo-culting it.

## Calling from a trigger (outbox pattern)

**Don't** put `grpc_call` directly inside a synchronous trigger. The call runs **inside the originating transaction** — a slow service blocks every writer, and a failure aborts the insert.

Instead, **enqueue** in the trigger and **drain** asynchronously. This is the standard outbox pattern.

```sql
-- 1. Outbox table
CREATE TABLE event_outbox (
  id          BIGSERIAL PRIMARY KEY,
  endpoint    TEXT NOT NULL,
  method      TEXT NOT NULL,
  request     JSONB NOT NULL,
  enqueued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  delivered_at TIMESTAMPTZ,
  last_error  TEXT
);

CREATE INDEX event_outbox_undelivered_idx
  ON event_outbox (id) WHERE delivered_at IS NULL;

-- 2. Trigger writes to the outbox only — fast and never blocks on the network
CREATE OR REPLACE FUNCTION trg_user_change_outbox() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO event_outbox(endpoint, method, request)
  VALUES (
    'hooks.internal:50051',
    'webhooks.Hook/Notify',
    jsonb_build_object('user_id', NEW.id, 'op', TG_OP)
  );
  RETURN NEW;
END;
$$;

CREATE TRIGGER user_change_outbox
  AFTER INSERT OR UPDATE ON users
  FOR EACH ROW EXECUTE FUNCTION trg_user_change_outbox();
```

Drain in a separate transaction — typically via [pg_cron](https://github.com/citusdata/pg_cron) — using `FOR UPDATE SKIP LOCKED` so multiple workers can process the queue without conflicts:

```sql
SELECT cron.schedule('drain-outbox', '* * * * *', $$
  WITH batch AS (
    SELECT id, endpoint, method, request
    FROM event_outbox
    WHERE delivered_at IS NULL
    ORDER BY id
    LIMIT 100
    FOR UPDATE SKIP LOCKED
  )
  UPDATE event_outbox e
  SET delivered_at = NOW()
  FROM batch b
  WHERE e.id = b.id
    AND grpc_call(b.endpoint, b.method, b.request) IS NOT NULL;
$$);
```

:::caution On RPC failures the whole batch aborts

If any one `grpc_call` fails inside the batch, the entire `UPDATE` rolls back — the next cron tick will pick up the same rows again. For independent retry, wrap `grpc_call` in a SECURITY DEFINER function with `EXCEPTION WHEN OTHERS` and store the error into `last_error`.

:::

## Polling a service from `pg_cron`

For services that produce data on demand — health checks, lookups, batch jobs — schedule a `grpc_call` directly:

```sql
SELECT cron.schedule(
  'health-check',
  '*/5 * * * *',
  $$ INSERT INTO health_log (checked_at, response)
     SELECT NOW(),
            grpc_call(
              'health.internal:50051',
              'health.Health/Check',
              '{"service": "core"}'::jsonb,
              options => '{"timeout_ms": 2000}'::jsonb
            ); $$
);
```

:::note `pg_cron` is a separate extension

Install it independently (`apt install postgresql-NN-cron`, then `CREATE EXTENSION pg_cron;`). Without `pg_cron`, fall back to an external scheduler.

:::

## Enriching a query with batched calls

Use `grpc_call` inline as a function call — one call per row.

```sql
INSERT INTO user_profile_enrichment (user_id, profile)
SELECT u.id,
       grpc_call(
         'profiles.internal:50051',
         'profiles.Profiles/Get',
         jsonb_build_object('user_id', u.id)
       )
FROM users u
WHERE u.profile_fetched_at IS NULL
ORDER BY u.id
LIMIT 1000;
```

:::caution Per-row gRPC cost

Every row does a full RPC round-trip. For 1000 rows × 10 ms latency, that's 10 seconds of wall time, all inside one transaction. Two ways to scale:

1. **Bounded `LIMIT`** keeps each batch's transaction short — the `LIMIT 1000` above is doing exactly this.
2. **Batched RPC** — if your service exposes a `GetMany(ids[])`-style endpoint, prefer that. One call returning many results beats N calls returning one each.

:::

## Combining patterns

The outbox above is the most production-shaped of the three. Any time you'd think "this trigger should fire a webhook," think outbox first. Any time you'd think "this query should call a service per row," ask whether the service has a batched method first.
