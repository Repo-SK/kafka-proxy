## What is it
This is a simple web application to proxy HTTP traffic to a Kafka cluster.
It exposes a ``/publish`` route
that you can ``POST`` a Kafka message to.

The payload to ``POST`` is:
```typescript
interface PublishData {
    topic: string;
    message: string; // a JSON stringified string of your message
}
```

## Authentication
An ``Authentication`` header must be present otherwise all requests will be rejected.
The auth token can be set via the ``AUTH_TOKEN`` key in the env file.

## Compilation instructions:
* install cross
* run ``cross build --release --target x86_64-unknown-linux-gnu``

## Deployment instructions:
* Upload compiled binary to the internet
* ``cd /home``
* ``curl -o kafka-proxy <uploaded_file_url>``
* ``chmod +x kafka-proxy``
* make a systemd service and start it :)