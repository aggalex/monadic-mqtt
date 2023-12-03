use crate::mqtt::error::PublishError;
use crate::mqtt::Connection;
use rumqttc::v5::mqttbytes::QoS;
use serde::Serialize;
use std::marker::PhantomData;

pub struct Sender<T: Serialize + Send + Sync> {
    topic: String,
    conn: Connection,
    p: PhantomData<T>,
}

async fn send(topic: &str, payload: String, conn: &Connection) -> Result<(), PublishError> {
    conn.client
        .publish(topic, QoS::ExactlyOnce, false, payload)
        .await?;
    Ok(())
}

impl<T: Serialize + Send + Sync> Drop for Sender<T> {
    fn drop(&mut self) {
        let topic = std::mem::take(&mut self.topic);
        let conn = self.conn.clone();
        tokio::spawn(async move {
            send(&topic, "".to_string(), &conn).await.unwrap();
        });
    }
}

impl<T: Serialize + Send + Sync> Sender<T> {
    pub fn new(topic: &str, conn: Connection) -> Sender<T> {
        Sender {
            topic: topic.to_string(),
            conn,
            p: PhantomData,
        }
    }

    pub async fn send(&self, item: T) -> Result<(), PublishError> {
        send(&self.topic, serde_json::to_string(&item)?, &self.conn).await?;
        Ok(())
    }
}
