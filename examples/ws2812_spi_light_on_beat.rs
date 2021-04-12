/*
MIT License

Copyright (c) 2021 Philipp Schuster

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
use cpal::Device;
use std::collections::{BTreeMap};
use std::io::stdin;
use beat_detector::StrategyKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};
use std::ops::Add;
use ws2818_rgb_led_spi_driver::adapter_spi::WS28xxSpiAdapter;
use ws2818_rgb_led_spi_driver::adapter_gen::WS28xxAdapter;


// LED steps per second
pub const ANIMATION_FREQUENCY: u64 = 90; // in Hz
pub const ANIMATION_FREQUENCY_MS: u64 = 1000 / ANIMATION_FREQUENCY;

/// Binary created for the Raspberry Pi which consumes audio
/// and outputs light on WS2812 LEDs via the SPI device.
fn main() {
    println!("make sure you have \"SPI\" on your Pi enabled and that MOSI-Pin is connected with DIN-Pin!");
    let mut adapter = WS28xxSpiAdapter::new("/dev/spidev0.0").unwrap();

    let num_leds = select_num_leds();
    let anim = MovingLightStripsAnimation::new(num_leds as usize);
    let anim = Arc::new(Mutex::new(anim));

    let recording = Arc::new(AtomicBool::new(true));
    let recording_cpy = recording.clone();
    ctrlc::set_handler(move || {
        eprintln!("Stopping recording");
        recording_cpy.store(false, Ordering::SeqCst);
    }).expect("Ctrl-C handler doesn't work");

    let devs = beat_detector::record::audio_input_device_list();
    if devs.is_empty() { panic!("No audio input devices found!") }
    let dev = if devs.len() > 1 { select_input_device(devs) } else { devs.into_iter().next().unwrap().1 };
    let strategy = select_strategy();
    let anim_t = anim.clone();
    let on_beat = move |info| {
        println!("Found beat at {:?}ms", info);
        anim_t.lock().unwrap().add_next_light_impulse();
    };
    let handle = beat_detector::record::start_listening(
        on_beat,
        Some(dev),
        strategy,
        recording.clone(),
    ).unwrap();

    while recording.load(Ordering::SeqCst) {
        let next_timestamp = Instant::now().add(Duration::from_millis(ANIMATION_FREQUENCY_MS));
        {
            // drop lock early
            let mut anim = anim.lock().unwrap();
            anim.shift_all_pixels();
            adapter.write_rgb(&anim.rgb_strip_vec_data).unwrap();
        }

        sleep_busy_waiting_until(next_timestamp);
    }

    handle.join().unwrap();
}

fn select_input_device(devs: BTreeMap<String, Device>) -> Device {
    println!("Available audio devices:");
    for (i, (name, _)) in devs.iter().enumerate() {
        println!("  [{}] {}", i, name);
    }
    println!("Select audio device: input device number and enter:");
    let mut input = String::new();
    while stdin().read_line(&mut input).unwrap() == 0 {}
    let input = input.trim().parse::<u8>().expect("Input must be a valid number!");
    devs.into_iter().enumerate()
        .filter(|(i, _)| *i == input as usize)
        .map(|(_i, (_name, dev))| dev)
        .take(1)
        .next()
        .unwrap()
}

fn select_strategy() -> StrategyKind {
    println!("Available beat detection strategies:");
    StrategyKind::values().into_iter().enumerate().for_each(|(i, s)| {
        println!("  [{}] {} - {}", i, s.name(), s.description());
    });
    println!("Select strategy: input id and enter:");
    let mut input = String::new();
    while stdin().read_line(&mut input).unwrap() == 0 {}
    let input = input.trim().parse::<u8>().expect("Input must be a valid number!");
    match input {
        0 => StrategyKind::LPF,
        1 => StrategyKind::Spectrum,
        _ => panic!("Invalid strategy!"),
    }
}

/// Returns n from args or default.
pub fn select_num_leds() -> u16 {
    println!("Input and enter how many LEDs are connected to your device (64, 150, ..):");
    let mut input = String::new();
    while stdin().read_line(&mut input).unwrap() == 0 {}
    let input = input.trim().parse::<u16>().expect("Input must be a valid number!");
    input
}

#[inline(always)]
pub fn sleep_busy_waiting_ms(ms: u64) {
    let target_time = Instant::now().add(Duration::from_millis(ms));
    loop {
        if Instant::now() >= target_time {
            break;
        }
    }
}

#[inline(always)]
pub fn sleep_busy_waiting_until(then: Instant) {
    loop {
        if Instant::now() >= then {
            break;
        }
    }
}

/// Returns a pixel with a random color and a minimal
/// brightness. Tries to get real colors instead of white.
#[inline(always)]
pub fn get_random_pixel_val() -> (u8, u8, u8) {
    const COLORS: [(u8, u8, u8); 28] = [
        // some colors are multiple times listed to increase
        // their probability.
        (255, 255, 255), // white
        (255, 0, 0),     // red
        (255, 0, 0),     // red
        (255, 0, 0),     // red
        (0, 255, 0),     // green
        (0, 0, 255),     // blue
        (13, 255, 248),  // turquoise
        (13, 255, 248),  // turquoise
        (13, 255, 248),  // turquoise
        (255, 168, 0),   // dark orange
        (255, 168, 0),   // dark orange
        (255, 189, 0),   // bright orange
        (255, 189, 0),   // bright orange
        (255, 189, 0),   // bright orange
        (255, 255, 0),   // yellow
        (255, 255, 0),   // yellow
        (255, 255, 0),   // yellow
        (234, 10, 142),  // Telekom Magenta
        (234, 10, 142),  // Telekom Magenta
        (234, 10, 142),  // Telekom Magenta
        (175, 0, 255),   // purple
        (0, 150, 255),   // semi light blue
        (0, 198, 255),   // very light blue
        (0, 198, 255),   // very light blue
        (0, 198, 255),   // very light blue
        (255, 114, 114), // light red
        (255, 114, 114), // light red
        (255, 114, 114), // light red
    ];

    let i = rand::random::<u8>();
    let i = i % COLORS.len() as u8;

    COLORS[i as usize]
}

#[inline(always)]
pub fn darken_rgb(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    (
        ((r as f32) * factor) as u8,
        ((g as f32) * factor) as u8,
        ((b as f32) * factor) as u8,
    )
}

const MOVING_LIGHT_IMPULSE_LEN: usize = 15;

pub struct MovingLightStripsAnimation {
    led_count: usize,
    rgb_strip_vec_data: Vec<(u8, u8, u8)>,
    new_rgb_data_vec: Vec<(u8, u8, u8)>,
}

impl MovingLightStripsAnimation {
    pub fn new(mut led_count: usize) -> Self {
        if led_count % 2 != 0 {
            led_count = led_count + 1;
        }

        MovingLightStripsAnimation {
            led_count,
            rgb_strip_vec_data: vec![(0, 0, 0); led_count],
            new_rgb_data_vec: vec![(0, 0, 0); MOVING_LIGHT_IMPULSE_LEN],
        }
    }

    #[inline(always)]
    fn add_next_light_impulse(&mut self) {
        let (r, g, b) = get_random_pixel_val();
        self.new_rgb_data_vec[00] = darken_rgb(r, g, b, 0.1);
        self.new_rgb_data_vec[01] = darken_rgb(r, g, b, 0.2);
        self.new_rgb_data_vec[02] = darken_rgb(r, g, b, 0.4);
        self.new_rgb_data_vec[03] = darken_rgb(r, g, b, 0.6);
        self.new_rgb_data_vec[04] = darken_rgb(r, g, b, 0.7);
        self.new_rgb_data_vec[05] = darken_rgb(r, g, b, 0.8);
        self.new_rgb_data_vec[06] = darken_rgb(r, g, b, 0.9);
        self.new_rgb_data_vec[07] = (r, g, b);
        self.new_rgb_data_vec[08] = darken_rgb(r, g, b, 0.9);
        self.new_rgb_data_vec[09] = darken_rgb(r, g, b, 0.8);
        self.new_rgb_data_vec[10] = darken_rgb(r, g, b, 0.7);
        self.new_rgb_data_vec[11] = darken_rgb(r, g, b, 0.6);
        self.new_rgb_data_vec[12] = darken_rgb(r, g, b, 0.4);
        self.new_rgb_data_vec[13] = darken_rgb(r, g, b, 0.2);
        self.new_rgb_data_vec[14] = darken_rgb(r, g, b, 0.1);
    }

    /// Shifts all pixel to the next position.
    /// Iterates backwards through  `self.rgb_strip_vec_data` from both sides!
    /// Because our strip looks like this:
    ///
    /// ```
    /// [LED 0]   [LED 1]       ... [LED 5]  [LED 6]  ... [LED N]
    /// [RGB N/2] [RGB N/2 - 1] ... [RGB 0]  [RGB 1]  ... [RGB N/2]  // RGB value; animated motion to the edges
    /// [Vec[0]]  [Vec[1]]      ... [Vec[x]] [Vec[y]] ... [Vec[N]]
    /// ```
    #[inline(always)]
    fn shift_all_pixels(&mut self) {
        for i in 0..self.led_count / 2 {
            let i_left = i;
            let i_right = self.led_count - 1 - i;
            let is_in_center = i_left + 1 == i_right;

            if is_in_center {
                let new = self.new_rgb_data_vec.last().unwrap().clone();
                self.rgb_strip_vec_data[i_left] = new;
                self.rgb_strip_vec_data[i_right] = new;
            } else {
                let prev_left = self.rgb_strip_vec_data[i_left + 1].clone();
                self.rgb_strip_vec_data[i_left] = prev_left;
                let prev_right = self.rgb_strip_vec_data[i_right - 1].clone();
                self.rgb_strip_vec_data[i_right] = prev_right;
            }
        }

        for i in 0..MOVING_LIGHT_IMPULSE_LEN {
            let i = MOVING_LIGHT_IMPULSE_LEN - 1 - i;

            if i == 0 {
                self.new_rgb_data_vec[i] = (0, 0, 0);
            } else {
                let prev = self.new_rgb_data_vec[i - 1].clone();

                self.new_rgb_data_vec[i] = prev;
            }
        }
    }
}
