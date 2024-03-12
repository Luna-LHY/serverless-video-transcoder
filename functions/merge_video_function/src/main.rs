use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client as S3Client};
use aws_sdk_s3::primitives::ByteStream;
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde_json::{json, Value};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct SegmentData {
    job_id: String,
    transcoded_segment: String,
    segment_order: f64,
    s3_bucket: String,
    s3_prefix: String,
    object_name: String
}

fn generate_m3u8file(m3u8_filepath: &str, event: LambdaEvent<Value>) -> i32 {
    let mut m3u8_file = File::create(m3u8_filepath).unwrap();
    writeln!(m3u8_file, "#EXTM3U").unwrap();
    writeln!(m3u8_file, "#EXT-X-VERSION:3").unwrap();
    writeln!(m3u8_file, "#EXT-X-TARGETDURATION:10").unwrap();
    writeln!(m3u8_file, "#EXT-X-MEDIA-SEQUENCE:0").unwrap();
    writeln!(m3u8_file, "#EXT-X-PLAYLIST-TYPE:VOD").unwrap();

    let mut segment_count: i32 = 0;
    for segment_group in event.payload.as_array().unwrap() {
        for segment in segment_group.as_array().unwrap() {
            segment_count += 1;
            let t4_filename = segment.get("transcoded_segment").unwrap();
            writeln!(m3u8_file, "#EXTINF:20.0").unwrap();
            writeln!(m3u8_file, "{}", t4_filename).unwrap();
        }
    }

    writeln!(m3u8_file, "#EXT-X-ENDLIST").unwrap();
    return segment_count
}


async fn handler(event: LambdaEvent<Value>) -> Result<Value, Error> {

    let input_data = event.payload.as_array().unwrap();
    let segment: SegmentData = serde_json::from_value(input_data.get(0).unwrap().get(0).unwrap().clone()).unwrap();

    let job_id = segment.job_id;
    let object_name = segment.object_name;
    let m3u8_filename = format!("{}.m3u8", object_name.split('.').next().unwrap());
    let m3u8_filepath = format!("/tmp/{}", m3u8_filename);
    println!("job_id: {}", job_id);
    println!("object_name: {}", object_name);
    println!("m3u8_filename: {}", m3u8_filename);

    let segment_count = generate_m3u8file(&m3u8_filepath, event);

    let bucket = env::var("MEDIA_BUCKET").unwrap();
    let key = format!("output/{}/{}", job_id, m3u8_filename);
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = S3Client::new(&config);

    s3_client.put_object()
        .bucket(&bucket)
        .key(&key)
        .body(ByteStream::from_path(Path::new(&m3u8_filepath)).await.unwrap())
        .send()
        .await
        .expect("Failed to put object to s3 bucket");

    let output_data = json!({
        "input_segments": &segment_count,
        "m3u8_file": &m3u8_filename,
        "create_hls": 0,
        "output_bucket": &bucket,
        "output_key": &key
    });

    Ok(output_data)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    print!("Hello, World!");
    lambda_runtime::run(service_fn(handler)).await
}
