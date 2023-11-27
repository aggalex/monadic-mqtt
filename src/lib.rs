#![feature(associated_type_defaults)]
#![feature(async_closure)]
#![feature(noop_waker)]
#![feature(return_position_impl_trait_in_trait)]

extern crate serde;
extern crate serde_json;
extern crate tokio;
extern crate futures;
extern crate bytes;

use serde::{Deserialize, Serialize};
use mqtt::event::{PublishEvent, SubscribeEvent};

pub mod mqtt;

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use rumqttc::v5::MqttOptions;
    use tokio::{task, time};
    use crate::mqtt::Listener;
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Sum {
        a: i32,
        b: i32
    }

    // TODO attribute
    impl PublishEvent for Sum {
        type Response = i32;
        const TOPIC: &'static str = "event/sum";
    }

    impl SubscribeEvent for Sum {
        fn invoke(&self) -> Result<Self::Response, Self::Error> {
            println!("summing {} + {}", self.a, self.b);
            Ok(self.a + self.b)
        }
    }

    #[derive(Serialize, Deserialize)]
    struct Factorial (i32);

    impl PublishEvent for Factorial {
        type Response = i64;
        const TOPIC: &'static str = "event/factorial";
    }

    impl SubscribeEvent for Factorial {
        type Error = String;
        fn invoke(&self) -> Result<Self::Response, Self::Error> {
            if self.0 <= 0 {
                return Err("Non-positive factorials not supported".to_string())
            } else if self.0 == 1 {
                return Ok(1)
            }
            let mut product = 1i64;
            Ok((2..=self.0).map(i64::from).fold(1i64, |a, b| a * b))
        }
    }

    fn connect() -> Listener {
        let mut mqttoptions = MqttOptions::new("rumqtt-async", "test.mosquitto.org", 1883);
        mqttoptions.set_keep_alive(Duration::from_secs(5));

        let listener = Listener::new(mqttoptions, 10);
        listener
    }

    #[tokio::test]
    async fn test_serial() {

        let mut listener = connect();
        let con = listener.connection().clone();

        let task = task::spawn(async move {
            listener
                .subscribe::<Sum>().await
                .listen().await;
        });

        time::sleep(Duration::from_millis(1000)).await;

        for i in 0..10 {
            let result = Sum { a: 1, b: i }.publish(con.clone()).await.unwrap();
            println!("{result}");
            time::sleep(Duration::from_millis(100)).await;
        }

        task.abort();
    }

    #[tokio::test]
    async fn test_parallel() {

        let mut listener = connect();
        let con = listener.connection().clone();

        let task = task::spawn(async move {
            listener
                .subscribe::<Factorial>().await
                .listen().await;
        });

        time::sleep(Duration::from_millis(1000)).await;

        let handles = (0..10).map(|i| {
            let con = con.clone();
            async move {
                task::spawn(Factorial(i).publish(con)).await
                    .into_iter()
                    .flatten()
                    .map(|res| (i, res))
                    .next()
            }
        });

        let results = futures::future::join_all(handles).await
            .into_iter()
            .flat_map(|opt: Option<_>| opt.into_iter())
            .map(|(i, res)| format!("{i}! = {res}"))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(9, results.lines().count());

        println!("Factorials:\n{results}");

        task.abort();
    }
}
