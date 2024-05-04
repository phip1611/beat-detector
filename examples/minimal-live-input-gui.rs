use beat_detector::recording;
use cpal::traits::StreamTrait;
use minifb::{Key, Window, WindowOptions};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[path = "_modules/example_utils.rs"]
mod example_utils;

const WIDTH: usize = 600;
const HEIGHT: usize = 400;

fn main() {
    example_utils::init_logger();
    let input_device = example_utils::select_audio_device();

    let ctrlc_pressed = Arc::new(AtomicBool::new(false));
    {
        let stop_recording = ctrlc_pressed.clone();
        ctrlc::set_handler(move || {
            stop_recording.store(true, Ordering::SeqCst);
        })
        .unwrap();
    }

    // Each Pixel is encoded as "<:8><red:8><green:8><blue:8>".
    let rgb_buffer: Vec<u32> = vec![0 /* black */; WIDTH * HEIGHT];
    let mut rgb_copy_buffer = rgb_buffer.clone();
    let rgb_buffer = Arc::new(Mutex::new(rgb_buffer));

    let mut window = Window::new(
        "Live Beat Visualizer - ESC to exit",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });

    // Limit to max ~60 fps update rate
    window.limit_update_rate(Some(std::time::Duration::from_secs_f32(1.0 / 60.0)));

    let handle = {
        let rgb_buffer = rgb_buffer.clone();
        recording::start_detector_thread(
            move |info| {
                println!("found beat!");
                let mut rgb_buffer_locked = rgb_buffer.lock().unwrap();
                for xrgb_pxl in rgb_buffer_locked.iter_mut() {
                    *xrgb_pxl = 0x00ffffffff;
                }
            },
            Some(input_device),
        )
        .unwrap()
    };

    log::info!("Start recording");

    while window.is_open()
        && !window.is_key_down(Key::Escape)
        && !ctrlc_pressed.load(Ordering::SeqCst)
    {
        let mut rgb_buffer_locked = rgb_buffer.lock().unwrap();
        for (i, xrgb_pxl) in rgb_buffer_locked.iter_mut().enumerate() {
            *xrgb_pxl = u32::from_ne_bytes([
                (xrgb_pxl.to_ne_bytes()[0] as f32 * 0.95) as u8,
                (xrgb_pxl.to_ne_bytes()[1] as f32 * 0.95) as u8,
                (xrgb_pxl.to_ne_bytes()[2] as f32 * 0.95) as u8,
                0,
            ]);
            // Update copy buffer.
            rgb_copy_buffer[i] = *xrgb_pxl;
        }

        // drop lock as early as possible to unblock beat detection thread.
        drop(rgb_buffer_locked);

        // We unwrap here as we want this code to exit if it fails.
        window
            .update_with_buffer(&rgb_copy_buffer, WIDTH, HEIGHT)
            .unwrap();
    }
    handle.pause().unwrap();
    log::info!("Stopped recording");
}
