use serde::{Deserialize};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::marker::PhantomData;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use lazy_static::lazy_static;
use rumqttc::v5::ClientError;
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::mqttbytes::v5::PublishProperties;
use crate::mqtt::Connection;
use crate::mqtt::event::event_handler::Fulfillable;

struct ResponseContent {
    payload: Poll<String>,
    waker: Option<Waker>,
    publish_properties: Option<PublishProperties>,
}

impl Clone for ResponseContent {
    fn clone(&self) -> Self {
        Self {
            payload: self.payload.clone(),
            waker: self.waker.clone(),
            publish_properties: self.publish_properties.clone()
        }
    }
}

unsafe impl Send for ResponseContent {

}

impl Unpin for ResponseContent {}

impl ResponseContent {
    fn new() -> Arc<Mutex<ResponseContent>> {
        Arc::new(Mutex::new(
            ResponseContent {
                payload: Poll::Pending,
                waker: None,
                publish_properties: None
            }
        ))
    }
}

pub struct Response<T: for<'a> Deserialize<'a>> {
    content: Arc<Mutex<ResponseContent>>,
    topic: String,
    p: PhantomData<T>
}

impl<T: for<'a> Deserialize<'a>> Clone for Response<T> {
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            topic: self.topic.clone(),
            p: PhantomData
        }
    }
}

unsafe impl<T: for<'a> Deserialize<'a>> Send for Response<T> {

}

unsafe impl<T: for<'a> Deserialize<'a>> Sync for Response<T> {

}

lazy_static! {
    static ref next_response_id: AtomicUsize = AtomicUsize::new(0usize);
}

impl<T: for<'a> Deserialize<'a>> Response<T> {
    pub fn new_without_id(topic: &str) -> Response<T> {
        Response {
            content: ResponseContent::new(),
            topic: topic.to_string(),
            p: PhantomData
        }
    }

    pub fn new(topic: &str) -> Response<T> {
        Response {
            content: ResponseContent::new(),
            topic: format!("{}/__res_{}", topic, next_response_id
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst,
                              |x| Some(x + 1)).unwrap()),
            p: PhantomData
        }
    }

    pub(crate) fn publish_properties(&self) -> Option<PublishProperties> {
        self.content.lock().unwrap().publish_properties.clone()
    }
}

impl<T: for<'a> Deserialize<'a> + Unpin + Send + 'static> Response<T> {
    pub async fn subscribe(&self, conn: &Connection) -> Result<(), ClientError> {
        let awaited_responses = conn.awaited_responses.clone();
        conn.client.subscribe(self.topic.clone(), QoS::AtLeastOnce).await?;
        awaited_responses.add(self.topic.to_string(), self.clone());
        Ok(())
    }
}

impl<T: for<'a> Deserialize<'a>> Future for Response<T> {
    type Output = Result<T, serde_json::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut content = self.content.lock().unwrap();
        content.waker = Some(cx.waker().clone());
        content.payload.clone()
            .map(|str| { cx.waker().wake_by_ref(); str })
            .map(|str| serde_json::from_str(&str))
    }
}

impl<T: for<'a> Deserialize<'a> + Send + Unpin + 'static> Fulfillable for Response<T> {
    fn fulfill(&self, str: String, properties: Option<&PublishProperties>) {
        let mut content = self.content.lock().unwrap();
        content.payload = Poll::Ready(str);
        content.publish_properties = properties.cloned();
        if let Some(ref waker) = content.waker {
            waker.wake_by_ref()
        }
    }

    fn topic(&self) -> &str {
        &self.topic
    }
}
