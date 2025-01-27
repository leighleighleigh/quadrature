use embassy_executor::Executor;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Subscriber};
use embedded_hal_async::digital::Wait;
use embedded_hal_compat::eh1_0::digital::{ErrorType, InputPin};
use getch_rs::{Getch, Key};
use log::*;
use quadrature_encoder::{Blocking, FullStep, IncrementalEncoder, Rotary, RotaryMovement};
use static_cell::StaticCell;
use std::convert::Infallible;

// type alias for the encoder - allows easy switching between different step modes
type Encoder<Clk, Dt, Steps = FullStep, T = i32, PM = Blocking> =
    IncrementalEncoder<Rotary, Clk, Dt, Steps, T, PM>;

type GetchPubSub = PubSubChannel<CriticalSectionRawMutex, Key, 64, 2, 1>;
type GetchSub<'a> = Subscriber<'a, CriticalSectionRawMutex, Key, 64, 2, 1>;
static GETCH_CHARS: GetchPubSub = PubSubChannel::new();

// Custom Pin type which we will impliment both InputPin and Wait on,
// using GetCh readings from stdinput, to toggle the state of the pins.
struct PinGetch<'a> {
    key: Key,
    state: bool,
    // the subscriber to the getch channel
    getch_sub: GetchSub<'a>,
}

impl<'a> PinGetch<'a> {
    fn new(key: Key, initial_state: bool, getch_stream: &'a GetchPubSub) -> Self {
        Self {
            key,
            state: initial_state,
            getch_sub: getch_stream
                .subscriber()
                .expect("got GETCH_CHARS subscriber"),
        }
    }

    fn check_for_key(&mut self) -> Result<(), Infallible> {
        loop {
            let k = self.getch_sub.try_next_message_pure();

            if let Some(k) = k {
                if self.key == k {
                    self.state = !self.state;
                }
            } else {
                // no more keys to check!
                break;
            }
        }
        Ok(())
    }

    async fn wait_for_key(&mut self) -> Result<(), Infallible> {
        loop {
            let k = self.getch_sub.next_message_pure().await;
            // toggle when the key is pressed
            if k == self.key {
                self.state = !self.state;
                break;
            }
        }
        Ok(())
    }
}

impl<'a> ErrorType for PinGetch<'a> {
    type Error = Infallible;
}

impl<'a> InputPin for PinGetch<'a> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.check_for_key()?;
        Ok(self.state)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.check_for_key()?;
        Ok(!self.state)
    }
}

impl<'a> Wait for PinGetch<'a> {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        // if we are already high, we don't need to wait
        if self.state {
            return Ok(());
        }
        // otherwise, wait for the key to be pressed
        self.wait_for_key().await
    }

    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        // if we are already low, we don't need to wait
        if !self.state {
            return Ok(());
        }
        // otherwise, wait for the key to be pressed
        self.wait_for_key().await
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        // waits for an actual rising edge, even if we are already high
        self.wait_for_low().await?;
        self.wait_for_high().await
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        // waits for an actual falling edge, even if we are already low
        self.wait_for_high().await?;
        self.wait_for_low().await
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        // waits for any change
        self.wait_for_key().await
    }
}

#[embassy_executor::task]
async fn getch_task() {
    let pub0 = GETCH_CHARS.publisher().expect("got GETCH_CHARS publisher");
    let getch = Getch::new();

    info!("Press 'j' / 'k' to toggle CLOCK / DATA.");
    info!("Press 'q' or <Ctrl-C> to quit.");

    loop {
        // yield before and after every blocking call,
        // to allow the encoder task to run
        embassy_futures::yield_now().await;

        match getch.getch() {
            Ok(Key::Char('q')) => break,
            Ok(Key::Ctrl(_)) => break, // any Ctrl- combination will quit (such as Ctrl-C,Ctrl-Z)
            Ok(key) => pub0.publish(key).await,
            Err(e) => error!("Error getting key: {:?}", e),
        }

        embassy_futures::yield_now().await;
    }

    // quit
    info!("Quitting.");

    // manually drop the getch to restore the terminal
    drop(getch);

    std::process::exit(0);
}

#[embassy_executor::task]
async fn encoder_task() {
    let pin_clk = PinGetch::new(Key::Char('j'), true, &GETCH_CHARS);
    let pin_dt = PinGetch::new(Key::Char('k'), true, &GETCH_CHARS);

    let mut encoder = Encoder::<_, _>::new(pin_clk, pin_dt).into_async();

    loop {
        match encoder.poll().await {
            Ok(Some(movement)) => {
                let direction = match movement {
                    RotaryMovement::Clockwise => "⟳ CW",
                    RotaryMovement::CounterClockwise => "⟲ CCW",
                };
                info!(
                    "Encoder state: {:02b}, position: {} {}",
                    encoder.raw_state(),
                    encoder.position(),
                    direction,
                );
            }
            Ok(_) => {
                info!(
                    "Encoder state: {:02b}, position: {}",
                    encoder.raw_state(),
                    encoder.position(),
                )
            }
            Err(error) => error!("Encoder error: {:?}.", error),
        }
    }
}

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .format_timestamp_nanos()
        .init();

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(encoder_task()).unwrap();
        spawner.spawn(getch_task()).unwrap();
    });
}
