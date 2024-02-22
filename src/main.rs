use std::env;
use std::time::Duration;
use std::process::exit;

use dotenvy::dotenv;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use warp::{Filter, http::StatusCode};
use warp::reject::Reject;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PublishData {
    topic: String,
    message: String,
}

#[derive(Debug)]
struct AuthenticationError;
impl Reject for AuthenticationError {}

fn with_auth() -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    warp::header::header::<String>("Authorization")
        .and_then(|token: String| async move {
            if let Ok(expected_token) = env::var("AUTH_TOKEN") {
                if token == expected_token {
                    Ok(())
                } else {
                    Err(warp::reject::custom(AuthenticationError))
                }
            } else {
                Err(warp::reject::custom(AuthenticationError))
            }
        })
}

fn json_body() -> impl Filter<Extract=(PublishData, ), Error=warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

async fn publish_message(
    data: PublishData,
    producer: FutureProducer,
) -> Result<impl warp::Reply, warp::Rejection> {
    let result = producer.send(
        FutureRecord::to(&data.topic)
            .key("")
            .payload(&data.message),
        Duration::from_secs(0),
    ).await;

    match result {
        Ok(_) => Ok(StatusCode::OK),
        Err((e, _)) => {
            let error = format!("Failed to send message: {:?}", e);
            eprintln!("{}", error);

            // force restart of the service to reconnect to kafka
            tokio::spawn(async {
                println!("shutting down in 60 seconds...");
                tokio::time::sleep(Duration::from_secs(60)).await;
                exit(0);
            });

            Err(warp::reject::reject())
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let brokers = env::var("KAFKA_HOST").expect("KAFKA_HOST must be set");
    let protocol = env::var("KAFKA_PROTOCOL").expect("KAFKA_PROTOCOL must be set");
    let mechanism = env::var("KAFKA_MECHANISM").expect("KAFKA_MECHANISM must be set");
    let username = env::var("KAFKA_USERNAME").expect("KAFKA_USERNAME must be set");
    let password = env::var("KAFKA_PASSWORD").expect("KAFKA_PASSWORD must be set");

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("security.protocol", &protocol)
        .set("sasl.mechanisms", &mechanism)
        .set("sasl.username", &username)
        .set("sasl.password", &password)
        .set("queue.buffering.max.ms", "5000") // buffer up to 5 seconds
        .set("message.timeout.ms", "120000") // 2 minutes
        .set("retries", "5") // attempt to send messages up to 5 times
        .set("retry.backoff.ms", "3000") // wait for 3 seconds before retrying
        .set("retry.backoff.max.ms", "5000") // wait for 3 seconds before retrying
        .set("reconnect.backoff.ms", "1000") // delay between reconnection attempts
        .set("reconnect.backoff.max.ms", "120000") // maximum delay between reconnection attempts
        .create()
        .expect("Producer creation error");

    let producer_filter = warp::any().map(move || producer.clone());

    let publish_route = warp::post()
        .and(warp::path("publish"))
        .and(warp::path::end())
        .and(with_auth())
        .untuple_one()
        .and(json_body())
        .and(producer_filter.clone())
        .and_then(publish_message);

    let health_route = warp::path!("health").map(|| warp::reply::json(&"alive"));

    let routes = publish_route.or(health_route);

    warp::serve(routes)
        .run(([0, 0, 0, 0], 80))
        .await;
}
