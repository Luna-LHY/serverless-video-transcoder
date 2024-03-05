import boto3
import os
import subprocess
import shutil
import re
from urllib.parse import unquote_plus
from botocore.config import Config

s3_client = boto3.client('s3', os.environ['AWS_REGION'], config=Config(s3={'addressing_style': 'path'}))


def generate_m3u8file(m3u8_filepath, event):
    m3u8_file = open(m3u8_filepath, 'w')
    m3u8_file.write('#EXTM3U\n')
    m3u8_file.write('#EXT-X-VERSION:3\n')
    m3u8_file.write('#EXT-X-MEDIA-SEQUENCE:0\n')
    m3u8_file.write('#EXT-X-ALLOW-CACHE:YES\n')
    m3u8_file.write('#EXT-X-TARGETDURATION:21\n')

    segment_count = 0

    for segment_group in event:
        for segment in segment_group:
            segment_count = segment_count + 1
            t4_filename = segment['transcoded_segment']
            # TODO：Determine how we get file duration (fixed 20 or ffprobe)
            m3u8_file.write('#EXTINF:20.0\n')
            m3u8_file.write(t4_filename + "\n")

    m3u8_file.write('#EXT-X-ENDLIST\n')
    m3u8_file.close()

    return segment_count


def lambda_handler(event, context):

    if len(event) == 0:
        return {}

    # upload merged media to S3
    job_id = event[0][0]['job_id']
    object_name = event[0][0]['object_name']
    m3u8_filename = object_name.split('.')[0] + '.m3u8'
    m3u8_filepath = "/tmp/" + m3u8_filename

    segment_count = generate_m3u8file(m3u8_filepath, event)

    bucket = os.environ['MEDIA_BUCKET']
    key = 'output/{}/{}'.format(job_id, m3u8_filename)
    s3_client.upload_file(m3u8_filepath, bucket, key)

    return {
        'input_segments': segment_count,
        'm3u8_file': m3u8_filename,
        'create_hls': 0,
        'output_bucket': bucket,
        'output_key': key
    }
