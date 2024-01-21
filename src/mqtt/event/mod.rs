use std::any::TypeId;
use rumqttc::v5::ClientError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::future::Future;

use crate::mqtt::error::PublishError;
use crate::mqtt::event::event_handler::Fulfillable;
use crate::mqtt::event::response::Response;
use crate::mqtt::Connection;
use rumqttc::v5::mqttbytes::v5::PublishProperties;
use rumqttc::v5::mqttbytes::QoS;

pub mod event_handler;
pub(super) mod response;

pub trait PublishEvent: Serialize + Sized {
    type Response: Serialize + for<'a> Deserialize<'a> + Send + Unpin + 'static;

    const TOPIC: &'static str;

    const QUALITY_OF_SERVICE: QoS = QoS::AtMostOnce;

    fn publish_and_subscribe_response(self, conn: Connection) -> impl Future<Output = Result<Self::Response, PublishError>> {
        async move {
            let response = Response::<Self::Response>::new(Self::TOPIC);

            response.subscribe(&conn).await?;

            conn.client
                .publish_with_properties(
                    Self::TOPIC,
                    Self::QUALITY_OF_SERVICE,
                    false,
                    serde_json::to_string(&self)?,
                    PublishProperties {
                        response_topic: Some(response.topic().to_string()),
                        ..Default::default()
                    },
                )
                .await
                .map_err(Into::<PublishError>::into)?;

            Ok(response.await?)
        }
    }

    fn publish(self, conn: Connection) -> impl Future<Output = Result<(), PublishError>> {
        async move {
            conn.client
                .publish(
                    Self::TOPIC,
                    Self::QUALITY_OF_SERVICE,
                    false,
                    serde_json::to_string(&self)?,
                )
                .await
                .map_err(Into::<PublishError>::into)?;

            Ok(())
        }
    }
}

impl PublishEvent for () {
    type Response = ();
    const TOPIC: &'static str = "";

    fn publish_and_subscribe_response(
        self,
        _conn: Connection,
    ) -> impl Future<Output = Result<Self::Response, PublishError>> {
        async {
            let response = Response::new("");
            response.fulfill("".to_string(), None);
            Ok(response.await?)
        }
    }
}

impl PublishEvent for i32 {
    type Response = ();
    const TOPIC: &'static str = "number";
}

impl SubscribeEvent for i32 {
    fn invoke(&self) -> impl Future<Output = Result<Self::Response, Self::Error>> {
        async { Ok(()) }
    }
}

pub trait SubscribeEvent: for<'a> Deserialize<'a> + PublishEvent {
    type Error: Debug + Serialize = ();

    fn invoke(&self) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send;

    fn subscribe(conn: &Connection) -> impl Future<Output = Result<(), ClientError>> {
        conn.client.subscribe(Self::TOPIC, Self::QUALITY_OF_SERVICE)
    }
}
