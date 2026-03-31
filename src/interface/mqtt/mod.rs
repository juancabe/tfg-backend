use crate::dao::DbPool;
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::{AsyncClient, ClientError, Event, Incoming, MqttOptions};
use std::env;
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    MqttClientError(ClientError),
}

pub struct MqttClient {
    pub client: AsyncClient,
}

impl MqttClient {
    /// Creates the local client that connects to the embedded broker
    pub async fn new() -> (Self, rumqttc::v5::EventLoop) {
        let host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("MQTT_PORT")
            .unwrap_or_else(|_| "1884".to_string()) // Default to MQTT 5 port
            .parse::<u16>()
            .expect("MQTT_PORT must be a number");
        let client_id =
            env::var("MQTT_CLIENT_ID").unwrap_or_else(|_| "backend-internal-client".to_string());

        // Use the admin credentials defined in rumqttd.toml
        let user = env::var("MQTT_USER").expect("MQTT_USER must be set");
        let password = env::var("MQTT_PASSWORD").expect("MQTT_PASSWORD must be set");

        let mut mqttoptions = MqttOptions::new(client_id, host, port);
        mqttoptions.set_credentials(user, password);
        mqttoptions.set_keep_alive(Duration::from_secs(60));

        let (client, eventloop) = AsyncClient::new(mqttoptions, 10);
        (Self { client }, eventloop)
    }

    pub async fn run(
        self,
        mut eventloop: rumqttc::v5::EventLoop,
        pool: DbPool,
    ) -> Result<(), Error> {
        // Subscribe to relevant topics
        self.client
            .subscribe("sensors/+/data", QoS::AtMostOnce)
            .await
            .map_err(Error::MqttClientError)?;

        // Subscribe to test ping topic for connectivity verification
        self.client
            .subscribe("/v1/test/ping", QoS::AtMostOnce)
            .await
            .map_err(Error::MqttClientError)?;

        log::info!("Backend MQTT 5 Client started and subscribed to topics");

        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    if let Event::Incoming(Incoming::Publish(publish)) = notification {
                        Self::handle_message(publish, &self.client, &pool).await;
                    }
                }
                Err(e) => {
                    log::error!("MQTT 5 Client error: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn handle_message(
        publish: rumqttc::v5::mqttbytes::v5::Publish,
        client: &AsyncClient,
        _pool: &DbPool,
    ) {
        let topic = std::str::from_utf8(&publish.topic).unwrap_or("");
        log::info!("Backend received message on topic: {}", topic);

        match topic {
            "/v1/test/ping" => {
                log::info!("Ping received. Sending pong...");
                if let Err(e) = client
                    .publish("test/pong", QoS::AtMostOnce, false, "pong")
                    .await
                {
                    log::error!("Failed to send pong: {:?}", e);
                }
            }
            t if t.starts_with("sensors/") => {
                log::info!("Handling sensor data: {:?}", publish.payload);
                // Boilerplate: Call your service here
                // let service = ProducerService::new(pool.clone());
            }
            _ => {
                log::warn!("Unknown topic: {}", topic);
            }
        }
    }
}
