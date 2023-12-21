pub mod sender;

use crate::mqtt::error::PublishError;
use crate::mqtt::event::response::Response;
use crate::mqtt::Connection;
use rumqttc::qos;
use rumqttc::tokio_rustls::rustls::internal::msgs::base::Payload;
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::ClientError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{channel, Receiver, Sender};

const IS_DONE: &'static str = "isDone";

#[derive(Serialize, Deserialize, Clone)]
pub struct Stream<T> {
    pub topic: String,
    pub buffer_size: usize,
    p: PhantomData<T>,
}

impl<T> Stream<T> {
    pub fn new(topic: &str) -> Stream<T> {
        Self::new_with_size(topic, 8)
    }

    pub fn new_with_size(topic: &str, buffer_size: usize) -> Stream<T> {
        Self {
            topic: topic.to_string(),
            buffer_size,
            p: PhantomData,
        }
    }
}

async fn get_next_response<T: DeserializeOwned + Unpin + Send + 'static>(
    topic: &str,
    conn: &Connection,
) -> Result<T, PublishError> {
    let response = Response::<T>::with_exact_topic(&topic);
    response.subscribe(&conn).await?;
    Ok(response.await?)
}

impl<T: DeserializeOwned + Unpin + Send + 'static> Stream<T> {
    pub fn receiver(&self, conn: Connection) -> Receiver<Result<T, PublishError>> {
        let (sender, receiver) = channel(self.buffer_size);
        let topic = self.topic.clone();

        tokio::spawn(async move {
            loop {
                let result = get_next_response(&topic, &conn);
                match result.await {
                    Err(PublishError::EmptyPayload) => break,
                    res => {
                        if sender.send(res).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        receiver
    }
}

impl<T: Serialize + Unpin + Send + Sync + 'static> Stream<T> {
    pub fn sender(&self, conn: Connection) -> sender::Sender<T> {
        sender::Sender::<T>::new(&self.topic, conn)
    }
}
