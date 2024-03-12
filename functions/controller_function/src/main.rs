use std::env;
use std::process::Command;
use serde_json::Value;
use serde_json::{json};
use std::f64;
use std::str::FromStr;
use aws_sdk_s3::{Client as S3Client};
use aws_sdk_s3::presigning::PresigningConfig;
use std::time::Duration;
use aws_config::{BehaviorVersion};
use lambda_runtime::{Error, LambdaEvent, service_fn};

async fn handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let payload = event.payload;

    let job_id = payload.get("job_id").unwrap().as_str().unwrap();
    let bucket = payload.get("bucket").unwrap().as_str().unwrap();
    let key = payload.get("key").unwrap().as_str().unwrap();
    let bucket_prefix = payload.get("object_prefix").unwrap().as_str().unwrap();
    let object_name = payload.get("object_name").unwrap().as_str().unwrap();
    let default_segment_time= env::var("DEFAULT_SEGMENT_TIME")?;
    let segment_time: i32 = payload.get("segment_time").unwrap().as_str().unwrap_or(&default_segment_time).parse().unwrap();

    let video_details = analyze_video(bucket, key).await;

    let control_data = generate_control_data(&video_details, &job_id, segment_time as f64, &bucket, &bucket_prefix, &object_name);

    println!("control data is: {:?}", control_data);

    Ok(control_data)
}

async fn analyze_video(bucket: &str, key: &str) -> Value {
    println!("Analyze video!!");
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = S3Client::new(&config);
    let expires_in = Duration::from_secs(600);
    let presigned_request = s3_client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(PresigningConfig::expires_in(expires_in).unwrap())
        .await
        .unwrap();

    let video_file_presigned_url = presigned_request.uri();

    let output = Command::new("ffprobe")
        .args(&[
            "-loglevel",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
            video_file_presigned_url,
        ])
        .output()
        .expect("failed to execute ffprobe");


    if !output.status.success() {
        panic!(
            "Could not run ffprobe."
        );
    }

    let video_details_string = String::from_utf8(output.stdout)
        .expect("Failed to convert to string");

    return serde_json::from_str(&video_details_string).unwrap();
}

fn generate_control_data(
    video_details: &Value,
    job_id: &str,
    segment_time: f64,
    s3_bucket: &str,
    s3_prefix: &str,
    object_name: &str,
) -> Value {

    let mut control_data = json!({
        "video_details": video_details,
        "job_id": job_id,
        "s3_bucket": s3_bucket,
        "s3_prefix": s3_prefix,
        "object_name": object_name,
        "video_groups": []
    });

    println!("generate_control_data!!!!!");

    let mut video_stream = None;
    let t = video_details.get("streams");
    for stream in video_details.get("streams").unwrap().as_array().unwrap() {
        println!("stream is {}", stream);
        println!("codec_type is {}", stream.get("codec_type").unwrap());
        if stream.get("codec_type").unwrap() == "video" {
            println!("stream equals!!!");
            video_stream = Some(stream);
            break;
        }
    }

    println!("final video_stream is {}", video_stream.unwrap());


    if video_stream != None {
        let t = f64::from_str(video_stream.unwrap().get("duration").unwrap().as_str().unwrap()).unwrap();
        println!("t: {}", t);
        let video_duration = t;
        let mut segment_count = (video_duration / segment_time).ceil() as i32;

        println!(
            "video duration: {}, segment_time: {}, segment_count: {}",
            video_duration, segment_time, segment_count
        );
        let group_count: i32 = env::var("PARALLEL_GROUPS").unwrap().as_str().parse().unwrap();
        let group_segment_count = (segment_count as f64 / group_count as f64).ceil() as i32;
        let mut video_groups = Vec::new();

        for group_index in 0..group_count {
            let mut video_segments = Vec::new();
            for segment_index in 0..group_segment_count {
                if segment_count <= 0 {
                    break;
                }
                video_segments.push(json!({
                    "start_ts": segment_time * (group_index * group_segment_count + segment_index) as f64,
                    "duration": segment_time,
                    "segment_order": group_index * group_segment_count + segment_index
                }));
                segment_count -= 1;
            }
            video_groups.push(json!(video_segments));
        }

        control_data["video_groups"] = json!(video_groups);
    }

    control_data
}


#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handler)).await
}

