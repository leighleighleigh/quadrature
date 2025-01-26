use embassy_futures::block_on;
use embedded_hal_mock::eh1::digital::State;
use embedded_hal_mock::eh1::digital::{
    Mock as PinMock, State as PinState, Transaction as PinTransaction,
};

use quadrature_encoder::{LinearEncoder, LinearMovement};

fn main() {
    let pin_clk = PinMock::new(&[
        PinTransaction::get(PinState::High),
        PinTransaction::wait_for_state(State::Low),
        PinTransaction::get(PinState::High),
    ]);
    let pin_dt = PinMock::new(&[PinTransaction::get(PinState::High)]);

    let mut encoder = LinearEncoder::<_, _>::new(pin_clk, pin_dt).into_async();

    match block_on(encoder.poll()) {
        Ok(Some(movement)) => {
            let direction = match movement {
                LinearMovement::Forward => "forward",
                LinearMovement::Backward => "backward",
            };
            println!("Movement detected in {:?} direction.", direction)
        }
        Ok(_) => println!("No movement detected."),
        Err(error) => println!("Error detected: {:?}.", error),
    }
    println!("Encoder is at position: {:?}.", encoder.position());

    let (mut pin_clk, mut pin_dt) = encoder.release();
    pin_clk.done();
    pin_dt.done();
}
