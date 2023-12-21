use std::collections::HashMap;
use event::event_handler::{EventHandler, Invocation};
use event::SubscribeEvent;
use rumqttc::v5::{mqttbytes::QoS, AsyncClient, Event, EventLoop, Incoming, MqttOptions};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt::Debug;
use std::future::Future;
use uuid::Uuid;

pub mod error;
pub mod event;
pub mod stream;

#[derive(Clone)]
pub struct Connection {
    client: AsyncClient,
    uuid: Uuid,
    awaited_responses: EventHandler<(String, usize)>,
    events: EventHandler<String>,
}

unsafe impl Send for Connection {}

unsafe impl Sync for Connection {}

pub struct Listener {
    connection: Connection,
    event_loop: EventLoop,
}

impl Listener {
    pub fn new(opts: MqttOptions, cap: usize) -> Self {
        let (client, event_loop) = AsyncClient::new(opts, cap);
        Listener {
            connection: Connection {
                client,
                uuid: Uuid::new_v4(),
                awaited_responses: EventHandler::new(),
                events: EventHandler::new(),
            },
            event_loop,
        }
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub async fn subscribe<C: SubscribeEvent + Send + 'static>(&mut self) -> &mut Listener {
        let client = self.connection.client.clone();
        self.connection
            .events
            .add(C::TOPIC.to_string(), Invocation::<C>::new(client));
        C::subscribe(&self.connection).await.unwrap();
        self
    }

    pub async fn listen(&mut self) {
        while let Ok(notification) = self.event_loop.poll().await {
            println!("Received = {:?}", notification);
            let Event::Incoming(Incoming::Publish(event)) = notification else {
                continue;
            };

            let Ok(topic) = std::str::from_utf8(event.topic.as_ref()) else {
                eprintln!("Unparsable topic");
                continue;
            };

            let topic = topic.to_string();

            let Ok(payload) = std::str::from_utf8(event.payload.as_ref()) else {
                eprintln!("Unparsable payload");
                continue;
            };


            let user_props = event.properties.as_ref()
                .map(|props| props.user_properties.into_iter().collect::<HashMap<String, String>>())
                .unwrap_or_else(|| HashMap::new());

            if let Some(id) = user_props.get("uuid")
                .filter(|uuid| self.connection.uuid.to_string() == uuid)
                .and_then(|_| user_props.get("id"))
                .map(|str| usize::from(str))
            {
                self.connection
                    .awaited_responses
                    .invoke_by_topic_and_remove(&(topic, id), payload, event.properties.as_ref())
                    .await
                    .map(|response_topic| {
                        self.connection
                            .client
                            .unsubscribe(response_topic.topic().to_string())
                    });
            }

            self.connection
                .events
                .invoke_by_topic(&topic, payload, event.properties.as_ref())
                .await;
        }
    }
}
