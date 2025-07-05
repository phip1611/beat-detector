//! The audio analysis layer focuses on the actual beat detection.
//!
//! All code here requires that all data was properly processed and validated
//! by the [input processing layer]. Here lives the actual beat detection
//! algorithm.
//!
//! [input layer]: crate::layer_input_processing


pub mod root_iterator;
pub mod max_min_iterator;
pub mod beat_detector;