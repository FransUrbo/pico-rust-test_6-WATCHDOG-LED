//! Combination between
//! * https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/pio_ws2812.rs
//! * https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/watchdog.rs

#![no_std]
#![no_main]


use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::watchdog::*;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::bind_interrupts;
use embassy_time::{Duration, Timer};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel as SyncChannel, Receiver};

use {defmt_rtt as _, panic_probe as _};

enum StopState { Yes, }
static CHANNEL: SyncChannel<ThreadModeRawMutex, StopState, 64> = SyncChannel::new();

// ================================================================================

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

// ================================================================================

#[embassy_executor::task]
async fn feed_watchdog(
    control: Receiver<'static, ThreadModeRawMutex, StopState, 64>,
    mut wd: embassy_rp::watchdog::Watchdog)
{
    // Feed the watchdog every 3/4 second to avoid reset.
    loop {
	info!("Feeding watchdog (task)");
	wd.feed();

        Timer::after_millis(750).await;

	info!("Trying to receive");
	match control.try_receive() {
	    core::prelude::v1::Ok(StopState::Yes) => {
		info!("StopState = Yes received");
		break
	    },
	    Err(_) => {
		info!("Uncaught error received");
		continue
	    }
	}
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Start");

    let p = embassy_rp::init(Default::default());

    let Pio { mut common, sm0, .. } = Pio::new(p.PIO0, Irqs);
    let mut ws2812 = ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH0, p.PIN_15);

    let mut watchdog = Watchdog::new(p.WATCHDOG);
    watchdog.start(Duration::from_millis(1_050));
    info!("Started the watchdog timer");

    spawner.spawn(feed_watchdog(CHANNEL.receiver(), watchdog)).unwrap();

    // BLUE
    ws2812.write(&[(0,0,255).into()]).await;
    Timer::after_secs(2).await;
    ws2812.write(&[(0,0,0).into()]).await;
    Timer::after_millis(500).await;

    // Blink once a second for 5 seconds.
    for _ in 1..=5 {
	// GREEN
	info!("GREEN/ON");
	ws2812.write(&[(0,255,0).into()]).await;
        Timer::after_millis(500).await;

	info!("GREEN/OFF");
	ws2812.write(&[(0,0,0).into()]).await;
        Timer::after_millis(500).await;

	// WHITE
	info!("WHITE/ON");
	ws2812.write(&[(255,255,255).into()]).await;
        Timer::after_millis(500).await;

	info!("WHITE/OFF");
	ws2812.write(&[(0,0,0).into()]).await;
        Timer::after_millis(500).await;
    }

    // Tell the task to terminate.
    CHANNEL.send(StopState::Yes).await;
    info!("Stopped feeding, device will reset in 1.05 seconds");

    // Blink 10 times per second, not feeding the watchdog.
    // The processor should reset in 1.05 seconds.
    loop {
	// RED
	ws2812.write(&[(255,0,0).into()]).await;
        Timer::after_millis(100).await;

	ws2812.write(&[(0,0,0).into()]).await;
        Timer::after_millis(100).await;
    }
}
