// Copyright (c) 2023, Bloop Box. All rights reserved.
// Authors: Ben Scholzen (DASPRiD) <mail@dasprids.de>
//
// Licensed under the BSD 2-Clause license. See LICENSE file in the project root for full license information.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use core::sync::atomic::{AtomicBool, Ordering};
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::spi::Spi;
use embassy_rp::usb::Driver;
use embassy_rp::{gpio, interrupt, spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{block_for, Duration, Timer};
use embassy_usb::class::hid::{HidReaderWriter, HidWriter, ReportId, RequestHandler, State};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder, Config, Handler};
use gpio::{Level, Output};
use mfrc522::Mfrc522;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};
use {defmt_rtt as _, panic_probe as _};

static SUSPENDED: AtomicBool = AtomicBool::new(false);

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let _power_led = Output::new(p.PIN_25, Level::High);
    let mut status_led = Output::new(p.PIN_16, Level::Low);
    let mode_switch = Input::new(p.PIN_22, Pull::Up);

    // HID Keyboard initialization
    let irq = interrupt::take!(USBCTRL_IRQ);
    let driver = Driver::new(p.USB, irq);

    // https://github.com/openmoko/openmoko-usb-oui/blob/master/usb_product_ids.psv
    let mut config = Config::new(0x1d50, 0x617f);
    config.manufacturer = Some("Bloop Box");
    config.product = Some("NFC UID Scanner");
    config.serial_number = Some("e621");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config.supports_remote_wakeup = true;

    let mut device_descriptor = [0; 256];
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let request_handler = MyRequestHandler {};
    let mut device_handler = MyDeviceHandler::new();

    let mut state = State::new();
    let mut builder = Builder::new(
        driver,
        config,
        &mut device_descriptor,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut control_buf,
    );
    builder.handler(&mut device_handler);

    let config = embassy_usb::class::hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: Some(&request_handler),
        poll_ms: 10,
        max_packet_size: 64,
    };
    let hid = HidReaderWriter::<_, 1, 8>::new(&mut builder, &mut state, config);

    let mut usb = builder.build();

    let remote_wakeup: Signal<CriticalSectionRawMutex, _> = Signal::new();

    let usb_fut = async {
        loop {
            usb.run_until_suspend().await;
            match select(usb.wait_resume(), remote_wakeup.wait()).await {
                Either::First(_) => (),
                Either::Second(_) => unwrap!(usb.remote_wakeup().await),
            }
        }
    };

    let (reader, mut writer) = hid.split();

    // NFC Reader initialization
    let miso = p.PIN_4;
    let mosi = p.PIN_7;
    let clk = p.PIN_6;
    let touch_cs = p.PIN_5;
    let touch_reset = p.PIN_2;

    let mut reset = Output::new(touch_reset, Level::Low);
    Timer::after(Duration::from_micros(1)).await;
    reset.set_high();
    Timer::after(Duration::from_micros(50)).await;

    let mut config = spi::Config::default();
    config.frequency = 500_000;
    let spi = Spi::new_blocking(p.SPI0, clk, mosi, miso, config);
    let cs = Output::new(touch_cs, Level::Low);

    let mut mfrc522 = Mfrc522::new(spi)
        .with_nss(cs)
        .with_delay(|| block_for(Duration::from_micros(1)))
        .init()
        .unwrap();

    let in_fut = async {
        loop {
            let uid = loop {
                Timer::after(Duration::from_millis(150)).await;
                let atqa = match mfrc522.reqa() {
                    Ok(atqa) => atqa,
                    Err(_) => continue,
                };

                match mfrc522.select(&atqa) {
                    Ok(uid) => break uid,
                    Err(_) => continue,
                };
            };

            info!("SCANNED");
            status_led.set_high();

            if SUSPENDED.load(Ordering::Acquire) {
                info!("Triggering remote wakeup");
                remote_wakeup.signal(());
            }

            let uid_bytes = uid.as_bytes();
            let mut buffer: [u8; 20] = [0; 20];
            let mut padded_uid = [0; 10];

            if uid_bytes.len() == padded_uid.len() {
                padded_uid.copy_from_slice(uid_bytes);
            } else {
                padded_uid[..uid_bytes.len()].copy_from_slice(uid_bytes);
            }

            hex::encode_to_slice(padded_uid, &mut buffer).unwrap();

            let send_hotkey = mode_switch.is_high();

            if send_hotkey {
                send_keypress(&mut writer, 24, 1 | 2 | 4).await;
            }

            for char in buffer.iter().take(uid_bytes.len() * 2) {
                let keycode = if *char == 48 {
                    39
                } else if *char <= 56 {
                    char - 19
                } else {
                    char - 93
                };

                send_keypress(&mut writer, keycode, 0).await;
            }

            if send_hotkey {
                send_keypress(&mut writer, 7, 1 | 2 | 4).await;
            }

            loop {
                Timer::after(Duration::from_millis(150)).await;

                if mfrc522.wupa().is_ok() {
                    continue;
                }

                match mfrc522.wupa() {
                    Ok(_) => continue,
                    Err(e) => {
                        if !matches!(e, mfrc522::error::Error::Collision) {
                            break;
                        }
                    }
                }
            }

            status_led.set_low();
            info!("RELEASED");
        }
    };

    let out_fut = async {
        reader.run(false, &request_handler).await;
    };

    join(usb_fut, join(in_fut, out_fut)).await;
}

async fn send_keypress<'a, I: embassy_rp::usb::Instance>(
    writer: &mut HidWriter<'a, Driver<'a, I>, 8>,
    keycode: u8,
    modifier: u8,
) {
    let report = KeyboardReport {
        keycodes: [keycode, 0, 0, 0, 0, 0],
        leds: 0,
        modifier,
        reserved: 0,
    };
    match writer.write_serialize(&report).await {
        Ok(()) => {}
        Err(e) => warn!("Failed to send report: {:?}", e),
    };

    let report = KeyboardReport {
        keycodes: [0, 0, 0, 0, 0, 0],
        leds: 0,
        modifier: 0,
        reserved: 0,
    };
    match writer.write_serialize(&report).await {
        Ok(()) => {}
        Err(e) => warn!("Failed to send report: {:?}", e),
    };
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {=[u8]}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}

struct MyDeviceHandler {
    configured: AtomicBool,
}

impl MyDeviceHandler {
    fn new() -> Self {
        MyDeviceHandler {
            configured: AtomicBool::new(false),
        }
    }
}

impl Handler for MyDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        self.configured.store(false, Ordering::Relaxed);
        SUSPENDED.store(false, Ordering::Release);
        if enabled {
            info!("Device enabled");
        } else {
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        self.configured.store(false, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        self.configured.store(false, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        if configured {
            info!(
                "Device configured, it may now draw up to the configured current limit from Vbus."
            )
        } else {
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }

    fn suspended(&mut self, suspended: bool) {
        if suspended {
            info!("Device suspended, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled).");
            SUSPENDED.store(true, Ordering::Release);
        } else {
            SUSPENDED.store(false, Ordering::Release);
            if self.configured.load(Ordering::Relaxed) {
                info!(
                    "Device resumed, it may now draw up to the configured current limit from Vbus"
                );
            } else {
                info!("Device resumed, the Vbus current limit is 100mA");
            }
        }
    }
}
