use aws_sdk_dynamodb::{Client as DynamoDbClient, types::AttributeValue};
use aws_lambda_events::event::s3::{S3Event};
use aws_sdk_sfn::{Client as SFNClient};
use serde_json::{json};
use lambda_runtime::{service_fn, LambdaEvent, Error};
use std::env;
use aws_config::BehaviorVersion;
use urlparse::unquote_plus;
use uuid::Uuid;
use chrono::Utc;
use lambda_runtime::tower::ServiceExt;
use lambda_runtime::tracing::Event;

async fn handler(event: LambdaEvent<S3Event>) -> Result<(), Error> {
    let job_table = env::var("JOB_TABLE")?;
    let segment_time = env::var("DEFAULT_SEGMENT_TIME")?;
    let sfn_arn = env::var("SFN_ARN")?;
    let create_hls = env::var("ENABLE_HLS")?;

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = DynamoDbClient::new(&config);
    let sfn_client = SFNClient::new(&config);

    let s3_event: S3Event = event.payload;

    for record in s3_event.records {
        let bucket = record.s3.bucket.name.unwrap_or_default();
        let key = unquote_plus(record.s3.object.key.as_deref().unwrap_or_default())?;
        let object_prefix = key[..key.rfind('/').unwrap() + 1].to_owned();
        let object_name = key[key.rfind('/').unwrap() + 1..].to_owned();

        // create a job item in dynamodb
        let job_id = Uuid::new_v4().to_string();
        let request = dynamodb_client
            .put_item()
            .table_name(&job_table)
            .item("id", AttributeValue::S(job_id.clone()), )
            .item("bucket", AttributeValue::S(bucket.clone()), )
            .item("key", AttributeValue::S(key.clone()), )
            .item("object_prefix", AttributeValue::S(object_prefix.clone()), )
            .item("object_name", AttributeValue::S(object_name.clone()), )
            .item("created_at", AttributeValue::S(Utc::now().to_rfc3339()), );

        request.send().await?;

        let input = json!({
            "job_id": job_id.clone(),
            "bucket": bucket.clone(),
            "key": key.clone(),
            "object_prefix": object_prefix.clone(),
            "object_name": object_name.clone(),
            "segment_time": segment_time.clone(),
            "create_hls": create_hls.clone()
        });

        sfn_client
            .start_execution()
            .state_machine_arn(sfn_arn.as_str())
            .input(serde_json::to_string(&input)?)
            .send()
            .await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handler)).await
}
