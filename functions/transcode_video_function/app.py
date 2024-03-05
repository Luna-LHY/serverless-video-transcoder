import os
import re
import subprocess
import boto3
from botocore.config import Config
from urllib.parse import unquote_plus

s3_client = boto3.client('s3', os.environ['AWS_REGION'], config=Config(
    s3={'addressing_style': 'path'}))


def transcode_segment(presigned_url, start_ts, duration, segment_order):
    output_filename = 'tmp_' + str(segment_order) + '.mp4'
    output_filepath = "/tmp/" + output_filename

    # extract all i-frames as thumbnails
    cmd = ['ffmpeg', '-v', 'error', '-ss', str(start_ts - 1), '-i', presigned_url, '-ss', '1', '-t', str(duration), '-vf', "scale=-1:720", '-x264opts', 'stitchable', '-c:a', 'copy', '-y', output_filepath]

    # create thumbnails
    print("trancoding the segment")
    try:
        subprocess.check_output(cmd)
    except subprocess.CalledProcessError as e:
        print(e.output)

    return output_filepath


def mp4_to_t4(mp4_filepath, segment_order, bucket_name, job_id):
    ts_filename = 'tmp_' + str(segment_order) + '.ts'
    ts_filepath = '/tmp/' + ts_filename
    cmd = ['ffmpeg', '-y', '-i', mp4_filepath, '-vcodec', 'copy', '-acodec', 'copy', '-bsf:v', 'h264_mp4toannexb', ts_filepath]

    print("Transcoding mp4 file to ts.")
    subprocess.check_output(cmd)

    key = 'output/{}/{}'.format(job_id, ts_filename)
    s3_client.upload_file(ts_filepath, bucket_name, key)

    return ts_filename


def lambda_handler(event, context):
    job_id = event['job_id']
    presigned_url = event['presigned_url']
    bucket_name = event['s3_bucket']
    object_name = event['object_name']
    start_ts = event['video_segment']['start_ts']
    duration = event['video_segment']['duration']
    segment_order = event['video_segment']['segment_order']

    output_filepath = transcode_segment(presigned_url, start_ts, duration, segment_order)
    result = mp4_to_t4(output_filepath, segment_order, bucket_name, job_id)

    return {
        'job_id': job_id,
        'transcoded_segment': result,
        'segment_order': segment_order,
        's3_bucket': event['s3_bucket'],
        's3_prefix': event['s3_prefix'],
        'object_name': object_name
    }
