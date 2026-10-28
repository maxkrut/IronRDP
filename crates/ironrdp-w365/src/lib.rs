#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(html_logo_url = "https://cdnweb.devolutions.net/images/projects/devolutions/logos/devolutions-icon-shadow.svg")]
#![no_std]

extern crate alloc;

pub mod battery;
pub mod power_request;
pub mod screen_capture;

/// Protocol version every channel in this crate offers and accepts.
pub const PROTOCOL_VERSION_1: u32 = 0x0001_0000;
