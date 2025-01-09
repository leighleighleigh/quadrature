use embassy_executor::Executor;
use embassy_time::Timer;
use embedded_hal_mock::eh1::digital::Edge;
use embedded_hal_mock::eh1::digital::{
    Mock as PinMock, State as PinState, Transaction as PinTransaction,
};
use log::*;
use quadrature_encoder::{RotaryEncoder, RotaryMovement};
use static_cell::StaticCell;

#[embassy_executor::task]
async fn ticker() {
    loop {
        info!("tick");
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn encoder_task() {
    let mut clk_states: Vec<PinTransaction> = Vec::new();
    let mut dat_states: Vec<PinTransaction> = Vec::new();
    clk_states.push(PinTransaction::get(PinState::High));
    dat_states.push(PinTransaction::get(PinState::High));

    for i in 0..100 {
        match i % 4 {
            0 => {
                clk_states.push(PinTransaction::wait_for_edge(Edge::Falling));
                // dat_states.push(PinTransaction::wait_for_edge(Edge::Falling));
            }
            1 => {
                clk_states.push(PinTransaction::wait_for_edge_forever(Edge::Rising));
                dat_states.push(PinTransaction::wait_for_edge(Edge::Falling));
            }
            2 => {
                clk_states.push(PinTransaction::wait_for_edge(Edge::Rising));
                // dat_states.push(PinTransaction::wait_for_edge(Edge::Falling));
            }
            3 => {
                clk_states.push(PinTransaction::wait_for_edge_forever(Edge::Falling));
                dat_states.push(PinTransaction::wait_for_edge(Edge::Rising));
            }
            _ => {}
        }
    }

    let pin_clk = PinMock::new(&clk_states);
    let pin_dt = PinMock::new(&dat_states);

    let mut encoder = RotaryEncoder::<_, _>::new(pin_clk, pin_dt).into_async();

    loop {
        match encoder.poll().await {
            Ok(Some(movement)) => {
                let direction = match movement {
                    RotaryMovement::Clockwise => "clockwise",
                    RotaryMovement::CounterClockwise => "counter-clockwise",
                };
                println!(
                    "Movement detected in {:?} direction, state: {:02b}",
                    direction,
                    encoder.binary_state()
                )
            }
            Ok(_) => println!(
                "No movement detected, state: {:02b}",
                encoder.binary_state()
            ),
            Err(error) => println!("Error detected: {:?}.", error),
        }

        println!("Encoder is at position: {:?}.", encoder.position());
        Timer::after_millis(100).await;

        if encoder.position() == 25 {
            break;
        }
    }

    let (mut pin_clk, mut pin_dt) = encoder.release();
    pin_clk.done();
    pin_dt.done();
}

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .format_timestamp_nanos()
        .init();

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(ticker()).unwrap();
        spawner.spawn(encoder_task()).unwrap();
    });
}
