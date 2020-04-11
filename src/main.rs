extern crate libusb_sys as ffi;

use ffi::{libusb_context, libusb_device_handle, LIBUSB_SUCCESS, libusb_transfer};
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use core::mem;

const SPKR_EP: i32 = 1;
const ID_VENDOR: u16 = 0x046d;
const ID_PRODUCT: u16 = 0x0867;
const SAMPLE_RATE: u32 = 32000;
const CTRL_IFACE: i32 = 0;
const SPKR_IFACE: i32 = 2;
const ISO_PKT_PER_FRAME: i32 = 6;
const BYTES_PER_ISO_PKT: i32 = 128;
const TIMEOUT: i32 = 1000;
const A4: f32 = 440.0f32;
const CHANNEL_CNT: i32 = 2;
const TAU: f32 = std::f32::consts::PI * 2.0f32;
const BUFFER_SZ: usize = (BYTES_PER_ISO_PKT * BYTES_PER_ISO_PKT) as usize;

type SampleSize = i16;

// pros: no forward declarations, no h files, cargo easier thn cmake, sane int types, made me think about allocation
// cons: some pointer ceremony, more docs and less S/O, made me think about allocation

struct PlayState {
    samples_played: Mutex<i32>,
    buffers: [[SampleSize; BUFFER_SZ]; 3],
    xfers: [*mut libusb_transfer; 3],
}

impl PlayState {
    fn new() -> PlayState {
        PlayState {
            samples_played: Mutex::new(0),
            buffers: [[0i16; BUFFER_SZ]; 3],
            xfers: [null_mut(); 3],
        }
    }
}

fn main() {
    unsafe {
        // init
        let mut ctx: *mut libusb_context = null_mut();
        ffi::libusb_init(&mut ctx);
        if ctx == null_mut() {
            panic!("Couldn't init usb");
        }

        let dev_handle = ffi::libusb_open_device_with_vid_pid(ctx, ID_VENDOR, ID_PRODUCT);
        if dev_handle == null_mut() {
            panic!("Couldn't open device");
        }

        detach_kernel(dev_handle, CTRL_IFACE);
        detach_kernel(dev_handle, SPKR_IFACE);

        claim_iface(dev_handle, CTRL_IFACE);
        claim_iface(dev_handle, SPKR_IFACE);

        if ffi::libusb_set_interface_alt_setting(dev_handle, SPKR_IFACE, 1) != LIBUSB_SUCCESS {
            panic!("Can't enable speaker!");
        }

        // play
        let mut state = PlayState::new();
        for i in 0..3 {
            state.xfers[i] = ffi::libusb_alloc_transfer(ISO_PKT_PER_FRAME);
//            ffi::libusb_set_iso_packet_lengths( state.xfers[i], BYTES_PER_ISO_PKT );
            fill_xfer(&state, i);
        }
    }
}

unsafe fn fill_xfer(play_state: &PlayState, buff_idx: usize) {
    let mut samples_played = play_state.samples_played.lock().unwrap();

    let byte_cnt = BYTES_PER_ISO_PKT * ISO_PKT_PER_FRAME;
    let sample_cnt = byte_cnt / CHANNEL_CNT / mem::size_of::<SampleSize>() as i32;
    let max_samp_vol = SampleSize::max_value();
    for i in 0..sample_cnt {
        let seconds = (*samples_played + i) as f32 / SAMPLE_RATE as f32;
        let cycles = A4 * seconds;
        let samp = (cycles * TAU).sin() * max_samp_vol;
        play_state.buffers[buff_idx][i] = samp;
    }
    *samples_played += sample_cnt;

    ffi::libusb_fill_iso_transfer(
        play_state.xfers[buff_idx],
        dev_handle,
        SPKR_EP,
        play_state.buffers[buff_idx],
        byte_cnt,
        ISO_PKT_PER_FRAME,
        xferComplete,
        this,
        TIMEOUT,
    );
}

unsafe fn detach_kernel(dev_handle: *mut libusb_device_handle, iface_id: i32) {
    if ffi::libusb_kernel_driver_active(dev_handle, iface_id) != 1 {
        return;
    }
    if ffi::libusb_detach_kernel_driver(dev_handle, iface_id) != 0 {
        panic!("failed to detach");
    }
}

unsafe fn claim_iface(dev_handle: *mut libusb_device_handle, iface_id: i32) {
    if ffi::libusb_claim_interface(dev_handle, iface_id) != LIBUSB_SUCCESS {
        panic!("Couldn't claim interface");
    }
}
