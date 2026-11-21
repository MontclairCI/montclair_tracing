use montclair_tracing::WideEventRow;
use serde::Serialize;
use time::OffsetDateTime;

#[tokio::main]
async fn main() {
    eprintln!("Creating client");

    let server_url = "http://localhost:8123";
    let client = clickhouse::Client::default()
        .with_url(server_url)
        .with_user("default")
        .with_password("password")
        .with_database("default")
        // Enable inserting JSON columns as a string
        .with_setting("input_format_binary_read_json_as_string", "1")
        // Enable selecting JSON columns as a string
        .with_setting("output_format_binary_write_json_as_string", "1");

    eprintln!("Creating database");
    client
        .query(
            r#"
            CREATE DATABASE IF NOT EXISTS
                "montclair_tracing";
            "#,
        )
        .execute()
        .await
        .unwrap();

    eprintln!("Creating table");
    client
        .query(
            r#"
            CREATE TABLE IF NOT EXISTS
                "montclair_tracing"."event" (
                    "timestamp" DateTime64(3),
                    "level" LowCardinality(String),
                    "target" String,
                    "message" String,
                    "error_span_trace" String,
                    "attr" JSON
                )
            ENGINE = MergeTree()
            ORDER BY (
                "timestamp",
                "level"
            );
            "#,
        )
        .execute()
        .await
        .unwrap();

    eprintln!("Creating user");
    client
        .query(
            r#"
            CREATE USER IF NOT EXISTS
                "montclair_tracing_writer"
            IDENTIFIED WITH
                sha256_password
            BY
                'montclair_tracing'
            ;
            "#,
        )
        .execute()
        .await
        .unwrap();

    eprintln!("Granting privileges");
    client
        .query(
            r#"
            GRANT
                INSERT,
                SELECT
            ON
                "montclair_tracing"."event"
            TO
                "montclair_tracing_writer"
            ;
            "#,
        )
        .execute()
        .await
        .unwrap();

    eprintln!("Starting insert");
    let client = client.with_database("montclair_tracing");
    let mut inserter = client.insert::<WideEventRow>("event").await.unwrap();

    #[derive(Serialize)]
    struct ExampleAttrs {
        bool_field: bool,
        string_field: String,
        int_field: i64,
        float_field: f64,
    }

    eprintln!("Writing row");
    inserter
        .write(&WideEventRow {
            timestamp: OffsetDateTime::now_utc(),
            level: "info".to_string(),
            target: "target0".to_string(),
            message: "message0".to_string(),
            error_span_trace: "error0".to_string(),
            attr: serde_json::to_string(&ExampleAttrs {
                bool_field: false,
                string_field: "".to_string(),
                int_field: 0,
                float_field: 0.0,
            })
            .unwrap(),
        })
        .await
        .unwrap();

    eprintln!("Committing insert");
    inserter.end().await.unwrap();

    eprintln!("Done.");
}
