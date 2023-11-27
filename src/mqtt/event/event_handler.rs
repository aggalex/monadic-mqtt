use rumqttc::v5::{AsyncClient, mqttbytes::QoS};
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use rumqttc::v5::mqttbytes::v5::PublishProperties;
use crate::mqtt::event::SubscribeEvent;

pub trait Fulfillable: Send + Sync {
    fn fulfill(&self, str: String, response_topic: Option<&PublishProperties>);
    fn topic(&self) -> &str;
}

pub struct Invocation<C: SubscribeEvent> (AsyncClient, PhantomData<C>);

impl<C: SubscribeEvent> Invocation<C> {
    pub fn new(async_client: AsyncClient) -> Self {
        Self(async_client, PhantomData)
    }
}

unsafe impl<C: SubscribeEvent> Send for Invocation<C> {

}

unsafe impl<C: SubscribeEvent> Sync for Invocation<C> {

}

impl<C: SubscribeEvent + Send> Fulfillable for Invocation<C> {
    fn fulfill(&self, content: String, properties: Option<&PublishProperties>) {
        let client = self.0.clone();
        let response_topic = properties
            .and_then(|prop| prop.response_topic.as_deref().map(ToString::to_string));
        tokio::spawn(async move {
            let payload = serde_json::from_str::<C>(&content).map_err(|err| format!("{err:?}"))
                .and_then(|client_event| client_event.invoke().map_err(|err| format!("{err:?}"))) // This is the possibly time consuming action
                .and_then(|res| serde_json::to_string(&res).map_err(|err| format!("{err:?}")))
                .unwrap_or_else(|err| err);
            if let Some(topic) = response_topic {
                if let Err(err) = client.publish(topic.clone(), QoS::AtLeastOnce, false, payload).await
                {
                    eprintln!("===>> Error: {:?}", err);
                }
            }
        });
    }

    fn topic(&self) -> &str {
        C::TOPIC
    }
}

#[derive(Clone)]
pub struct EventHandler {
    map: Arc<RwLock<HashMap<String, Box<dyn Fulfillable>>>>,
}

unsafe impl Send for EventHandler {

}

unsafe impl Sync for EventHandler {

}

impl EventHandler {
    pub(crate) fn new() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    pub(crate) fn add(&self, topic: String, f: impl Fulfillable + 'static) {
        self.map.write().unwrap().insert(topic, Box::new(f));
    }

    fn remove(&self, topic: &str) -> Option<Box<dyn Fulfillable>> {
        self.map.write().unwrap().remove(topic)
    }

    pub(crate) async fn invoke_by_topic(&self, topic: &str, payload: &str, properties: Option<&PublishProperties>) {
        self.map.read().unwrap().get(topic)
            .map(|fulfillable| fulfillable.fulfill(payload.to_string(), properties));
    }

    pub(crate) async fn invoke_by_topic_and_remove(&self, topic: &str, payload: &str, properties: Option<&PublishProperties>) -> Option<Box<dyn Fulfillable>> {
        self.remove(topic).map(|fulfillable| {
            fulfillable.fulfill(payload.to_string(), properties);
            fulfillable
        })
    }
}
