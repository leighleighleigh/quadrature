use embassy_executor::Executor;
use embassy_time::Timer;
use embedded_hal_mock::eh1::digital::Edge;
use embedded_hal_mock::eh1::digital::{
    Mock as PinMock, State as PinState, Transaction as PinTransaction,
};
use log::*;
use quadrature_encoder::{Blocking, IncrementalEncoder, QuadStep, Rotary, RotaryMovement};
use static_cell::StaticCell;

// helper macros to generate a clockwise/counter-clockwise mock PinTransaction sequences
macro_rules! cw {
    ($clk:ident, $dat:ident) => {
        $clk.push(PinTransaction::wait_for_edge(Edge::Falling));
        $clk.push(PinTransaction::wait_for_edge_forever(Edge::Rising));
        $dat.push(PinTransaction::wait_for_edge(Edge::Falling));
        $clk.push(PinTransaction::wait_for_edge(Edge::Rising));
        $clk.push(PinTransaction::wait_for_edge_forever(Edge::Falling));
        $dat.push(PinTransaction::wait_for_edge(Edge::Rising));
    };
}
macro_rules! ccw {
    ($clk:ident, $dat:ident) => {
        $clk.push(PinTransaction::wait_for_edge_forever(Edge::Falling));
        $dat.push(PinTransaction::wait_for_edge(Edge::Falling));
        $clk.push(PinTransaction::wait_for_edge(Edge::Falling));
        $clk.push(PinTransaction::wait_for_edge_forever(Edge::Rising));
        $dat.push(PinTransaction::wait_for_edge(Edge::Rising));
        $clk.push(PinTransaction::wait_for_edge(Edge::Rising));
    };
}

// Number of clockwise rotations to mock
const CW: usize = 6;
// Number of counter-clockwise rotations to mock
const CCW: usize = 9;

// type alias for the encoder - allows easy switching between different step modes
type Encoder<Clk, Dt, Steps = QuadStep, T = i32, PM = Blocking> =
    IncrementalEncoder<Rotary, Clk, Dt, Steps, T, PM>;

#[embassy_executor::task]
async fn encoder_task() {
    let mut clk_states: Vec<PinTransaction> = Vec::new();
    let mut dat_states: Vec<PinTransaction> = Vec::new();

    // Initial state of the pins is checked in RotaryEncoder::new()
    clk_states.push(PinTransaction::get(PinState::High));
    dat_states.push(PinTransaction::get(PinState::High));

    for _ in 0..CW {
        cw!(clk_states, dat_states);
    }

    for _ in 0..CCW {
        ccw!(clk_states, dat_states);
    }

    let pin_clk = PinMock::new(&clk_states);
    let pin_dt = PinMock::new(&dat_states);

    let mut encoder = Encoder::<_, _>::new(pin_clk, pin_dt).into_async();
    // fetch the number of position steps per rotation - this is dependent on the encoder resolution mode
    let steps: usize = encoder.pulses_per_cycle();
    let mut expected_pulse_count = steps * (CW + CCW); // count the absolute number of pulses we expect to receive

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
                expected_pulse_count -= 1;
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

        Timer::after_millis(50).await;

        // break loop after all pulses received,
        // if the mock was setup properly, then all of the
        // pin transactions should have been consumed.
        if expected_pulse_count == 0 {
            break;
        }
    }

    let encoder_position = encoder.position();
    let expected_position = ((CW as i32) - (CCW as i32)) * (steps as i32);
    std::assert!(
        encoder_position == expected_position,
        "Encoder position {encoder_position} didnt match expectation {expected_position}"
    );

    // release the pins from the encoder, then finish the embedded_hal_mock test
    let (mut pin_clk, mut pin_dt) = encoder.release();
    pin_clk.done();
    pin_dt.done();

    // quit!
    std::process::exit(0);
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
    });
}
