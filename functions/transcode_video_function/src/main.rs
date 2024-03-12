use std::process::Command;
use serde_json::{json, Value};
use aws_sdk_s3::{Client as S3Client};
use std::path::Path;
use std::str::FromStr;
use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct TranscodeOutput {
    job_id: String,
    transcoded_segment: String,
    segment_order: i32,
    s3_bucket: String,
    s3_prefix: String,
    object_name: String
}

#[derive(Deserialize, Debug)]
struct InputData {
    job_id: String,
    object_name: String,
    presigned_url: String,
    s3_bucket: String,
    video_segment: VideoSegment,
    s3_prefix: String,
}

#[derive(Deserialize, Debug)]
struct VideoSegment {
    duration: f64,
    segment_order: f64,
    start_ts: f64,
}

fn transcode_segment(presigned_url: &str, start_ts: i32, duration: i32, segment_order: i32) -> String {
    let output_filename = format!("tmp_{}.mp4", segment_order);
    let output_filepath = format!("/tmp/{}", output_filename);

    let cmd = Command::new("ffmpeg")
        .arg("-v")
        .arg("error")
        .arg("-ss")
        .arg((start_ts - 1).to_string())
        .arg("-i")
        .arg(presigned_url)
        .arg("-ss")
        .arg("1")
        .arg("-t")
        .arg(duration.to_string())
        .arg("-vf")
        .arg("scale=-1:720")
        .arg("-x264opts")
        .arg("stitchable")
        .arg("-c:a")
        .arg("copy")
        .arg("-y")
        .arg(&output_filepath)
        .output()
        .expect("Failed to execute ffmpeg");

    return output_filepath;
}

async fn mp4_to_t4(mp4_filepath: &str, segment_order: i32, bucket_name: &str, job_id: &str) -> String {
    let ts_filename = format!("tmp_{}.ts", segment_order);
    let ts_filepath = format!("/tmp/{}", ts_filename);

    println!("Transcoding mp4 file to ts.");

    let cmd = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(mp4_filepath)
        .arg("-vcodec")
        .arg("copy")
        .arg("-acodec")
        .arg("copy")
        .arg("-bsf:v")
        .arg("h264_mp4toannexb")
        .arg(&ts_filepath)
        .output()
        .expect("Failed to convert mp4 to t4 file type");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = S3Client::new(&config);
    let body = ByteStream::from_path(Path::new(&ts_filepath)).await.unwrap();
    let key = format!("output/{}/{}",job_id, ts_filename);

    s3_client.put_object()
        .bucket(bucket_name)
        .key(key)
        .body(body)
        .send()
        .await
        .expect("Failed to put object to S3 bucket.");

    return ts_filename
}

async fn handler(event: LambdaEvent<Value>) -> Result<TranscodeOutput, Error> {
    let input_data:InputData = serde_json::from_value(event.payload).unwrap();

    let job_id = input_data.job_id;
    let presigned_url = input_data.presigned_url;
    let bucket_name = input_data.s3_bucket;
    let object_name = input_data.object_name;

    let video_segment = input_data.video_segment;
    let start_ts = video_segment.start_ts as i32;
    let duration = video_segment.duration as i32;
    let segment_order = video_segment.segment_order as i32;

    let output_filepath = transcode_segment(&presigned_url, start_ts, duration, segment_order);
    let mp4_to_t4_result = mp4_to_t4(&output_filepath, segment_order, &bucket_name, &job_id).await;

    Ok(TranscodeOutput {
        job_id: job_id.to_string(),
        transcoded_segment: mp4_to_t4_result,
        segment_order: segment_order,
        s3_bucket: bucket_name.to_string(),
        s3_prefix: input_data.s3_prefix,
        object_name: object_name.to_string()
    })
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    print!("Hello, World!");
    lambda_runtime::run(service_fn(handler)).await
}

