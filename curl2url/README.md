# curl2url

A simple service to convert a web request into a `curl` command for inspection and debugging. It also acts as a proxy, handling small responses directly and offloading large file downloads to a `temp-file-host` service.

## Features

- Converts incoming `GET` requests into `curl` command strings.
- Executes the `curl` command and returns the response if it's within a configured size limit.
- For responses exceeding the size limit, it triggers an asynchronous download on a configured `temp-file-host` instance.
- Returns a job ID and a status URL for polling the progress of the asynchronous download.

## Configuration

The service is configured via a `config.toml` file in the root directory.

```toml
[server]
# The address and port the service will listen on.
listen_addr = "0.0.0.0:3000"

[proxy]
# The base URL of the temp-file-host service.
temp_file_host_url = "http://localhost:3001"
# The maximum response size (in bytes) to handle directly.
# Responses larger than this will be offloaded to temp-file-host.
max_response_size_bytes = 1048576 # 1MB
# The bearer token for authenticating with the temp-file-host's proxy API.
# This must match one of the tokens in temp-file-host's configuration.
auth_token = "insecure-token-for-internal-use-only"

[curl]
# Connection timeout for the curl command in seconds.
timeout_seconds = 30
# Whether curl should follow HTTP redirects.
follow_redirects = true
# Whether to include response headers in the direct response body.
include_headers = true
```

## API Usage

### Endpoint: `GET /curl`

**Query Parameter:**

- `url` (required): The URL of the target resource to fetch.

All headers from the incoming request to `/curl` are forwarded in the `curl` command.

#### Example 1: Small Response

Request:
```bash
curl "http://localhost:3000/curl?url=https://httpbin.org/get" -H "X-Custom-Header: MyValue"
```

Successful Response (if content is < `max_response_size_bytes`):
```json
{
  "curl_command": "curl -H 'x-custom-header: MyValue' -H 'host: localhost:3000' -i -s -L --connect-timeout 30 --max-filesize 1048576 'https://httpbin.org/get'",
  "response_body": "{\n  \"args\": {}, \n  \"headers\": {\n ... \n  },\n ... \n}\n",
  "response_headers": {
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "*",
    "connection": "keep-alive",
    "content-length": "304",
    "content-type": "application/json",
    "date": "...",
    "server": "gunicorn/19.9.0",
    "status": "HTTP/1.1 200 OK"
  },
  "triggered_async_download": false
}
```

#### Example 2: Large Response (Triggering Async Download)

Request to a URL that returns a file larger than `max_response_size_bytes`.

Request:
```bash
curl "http://localhost:3000/curl?url=http://speedtest.tele2.net/10MB.zip"
```

Successful Response:
```json
{
  "curl_command": "curl -L 'http://speedtest.tele2.net/10MB.zip'",
  "triggered_async_download": true,
  "job_id": "a1b2c3d4-e5f6-7890-1234-567890abcdef",
  "status_url": "http://localhost:3001/proxy/status/a1b2c3d4-e5f6-7890-1234-567890abcdef"
}
```

The client can then use the `status_url` to poll the `temp-file-host` service for the download status.

## Running the Service

```bash
cargo run
```

Ensure that the `temp-file-host` service is also running and that the URLs and auth tokens are configured correctly. 