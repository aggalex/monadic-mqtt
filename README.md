# Monadic MQTT

A proof of concept for an MQTT library for rust that uses Rust's `Future` monads 
and tokio to simplify communication between IoT devices simpler than ever.

## How to use

### Static events

Make a serializable/deserializable topic by defining its content and configuring
how it gets published and processed:

```rust
#[derive(Serialize, Deserialize)]
struct Sum {
    a: i32,
    b: i32,
}

impl PublishEvent for Sum {
    type Response = i32;
    const TOPIC: &'static str = "event/sum";
}

impl SubscribeEvent for Sum {
    fn invoke(&self) -> Result<Self::Response, Self::Error> {
        Ok(self.a + self.b)
    }
}
```

And publish the event using the `publish` method:

```rust
async fn main() -> Result<(), Box<dyn Error>> {
    let mut listener = connect();
    let con = listener.connection().clone();

    // ...

    let should_be_seven = Sum { a: 2, b: 5 }.publish(&con).await?;
    println!("2 + 5 = {}", should_be_seven);
}
```

All the receiver needs to do is subscribe to the published event

```rust
async fn main() -> Result<(), Box<dyn Error>> {
    listener
        .subscribe::<Sum>().await
        .listen().await;
}
```

## How it works

This library uses [rumqtt](https://github.com/bytebeamio/rumqtt) in the background. 
Every publish means the struct gets serialized into JSON and sent to the receiver, 
while the publisher also subscribes to the respective response topic, and awaits for the response (blocks tokio thread).
The response topic is shared with the receiver through MQTT 5 publish properties.
The receiver then deserializes, calculates the result, serializes it and publishes the response back using QoS 1 on the topic it received.
Once the original publisher receives the response back it wakes the thread the published the event, and continues the execution,
with the deserialized value sent from the receiver used as a return value of the `publish` method. The return value is
wrapped in a `Result` monad, that switches to its `Err` variant whenever there's an error in execution.
