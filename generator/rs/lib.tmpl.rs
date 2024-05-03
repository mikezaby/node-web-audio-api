#![deny(clippy::all)]

use napi::{Env, JsObject, JsUndefined, JsFunction, Result};
use napi_derive::{module_exports, napi};

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

type SendItem = String;
pub(crate) fn send_recv_pair() -> &'static Mutex<(Option<Sender<SendItem>>, Option<Receiver<SendItem>>)> {
    static PAIR: OnceLock<Mutex<(Option<Sender<SendItem>>, Option<Receiver<SendItem>>)>> = OnceLock::new();
    PAIR.get_or_init(|| {
        let (send, recv) = mpsc::channel();
        Mutex::new((Some(send), Some(recv)))
    })
}

#[napi]
pub fn run_audio_worklet(env: Env) -> Result<JsUndefined> {
    println!("inside rust worklet");
    let recv = dbg!(send_recv_pair().lock().unwrap()).1.take().unwrap();
    for item in recv {
        println!("got one {}", &item);
        let proc = env.get_global()?.get_property::<_, JsObject>(env.create_string("proc123")?)?;
        let process = proc.get_property::<_, JsFunction>(env.create_string("process")?)?;

        let mut output_samples = vec![0.; 128];
        let data: &mut[u8] = unsafe {
            std::slice::from_raw_parts_mut(output_samples.as_mut_ptr() as *mut _, output_samples.len() * 4)
        };
        let data_ptr = data.as_mut_ptr();
        let ptr_length = data.len();
        let manually_drop = std::mem::ManuallyDrop::new(output_samples);
        let output_samples = unsafe {
            env
                .create_arraybuffer_with_borrowed_data(
                    data_ptr,
                    ptr_length,
                    manually_drop,
                    napi::noop_finalize,
                )
                .map(|array_buffer| {
                    array_buffer
                        .into_raw()
                        .into_typedarray(napi::TypedArrayType::Float32, 128, 0)
                })
            .unwrap()
        };

        let mut output_channels = env.create_array(0)?;
        output_channels.insert(output_samples)?;
        let mut outputs = env.create_array(0)?;
        outputs.insert(output_channels)?;

        let ret: bool = process.call3(
            env.create_array(128)?,
            outputs,
            env.create_array(128)?,
        )?;
        dbg!(ret);

        let output_samples: Vec<f32> = unsafe { Vec::from_raw_parts(data_ptr as *mut _, 128, 128) };
        dbg!(output_samples);
    }
    env.get_undefined()
}

#[macro_use]
mod base_audio_context;
#[macro_use]
mod audio_node;

// halpers
mod utils;
// Web Audio API
mod audio_context;
use crate::audio_context::NapiAudioContext;
mod audio_destination_node;
use crate::audio_destination_node::NapiAudioDestinationNode;
mod audio_param;
use crate::audio_param::NapiAudioParam;
mod audio_listener;
use crate::audio_listener::NapiAudioListener;
mod audio_buffer;
use crate::audio_buffer::NapiAudioBuffer;
mod periodic_wave;
use crate::periodic_wave::NapiPeriodicWave;
mod offline_audio_context;
use crate::offline_audio_context::NapiOfflineAudioContext;
// Generated audio nodes
${d.nodes.map(n => { return `
mod ${d.slug(n)};
use crate::${d.slug(n)}::${d.napiName(n)};`}).join('')}

// MediaDevices & MediaStream API
mod media_streams;
use crate::media_streams::NapiMediaStream;
mod media_devices;
use crate::media_devices::napi_enumerate_devices;
use crate::media_devices::napi_get_user_media;

#[cfg(all(
    any(windows, unix),
    target_arch = "x86_64",
    not(target_env = "musl"),
    not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[module_exports]
fn init(mut exports: JsObject, env: Env) -> Result<()> {
    // Do not print panic messages, handle through JS errors
    std::panic::set_hook(Box::new(|_panic_info| {}));

    let napi_class = NapiAudioContext::create_js_class(&env)?;
    exports.set_named_property("AudioContext", napi_class)?;

    let napi_class = NapiOfflineAudioContext::create_js_class(&env)?;
    exports.set_named_property("OfflineAudioContext", napi_class)?;

    let napi_class = NapiAudioBuffer::create_js_class(&env)?;
    exports.set_named_property("AudioBuffer", napi_class)?;

    let napi_class = NapiPeriodicWave::create_js_class(&env)?;
    exports.set_named_property("PeriodicWave", napi_class)?;

    let napi_class = NapiMediaStreamAudioSourceNode::create_js_class(&env)?;
    exports.set_named_property("MediaStreamAudioSourceNode", napi_class)?;

    // ----------------------------------------------------------------
    // Generated audio nodes
    // ----------------------------------------------------------------
    ${d.nodes.map(n => { return `
    let napi_class = ${d.napiName(n)}::create_js_class(&env)?;
    exports.set_named_property("${d.name(n)}", napi_class)?;
    `}).join('')}

    // ----------------------------------------------------------------
    // MediaStream API & Media Devices API
    // ----------------------------------------------------------------
    let mut media_devices = env.create_object()?;

    let napi_class = NapiMediaStream::create_js_class(&env)?;
    media_devices.set_named_property("MediaStream", napi_class)?;

    media_devices.create_named_method("enumerateDevices", napi_enumerate_devices)?;
    media_devices.create_named_method("getUserMedia", napi_get_user_media)?;
    // expose media devices
    exports.set_named_property("mediaDevices", media_devices)?;

    // ----------------------------------------------------------------
    // Store constructors for classes that need to be created from within Rust code
    // ----------------------------------------------------------------
    let mut store = env.create_object()?;

    let napi_class = NapiAudioDestinationNode::create_js_class(&env)?;
    store.set_named_property("AudioDestinationNode", napi_class)?;

    let napi_class = NapiAudioListener::create_js_class(&env)?;
    store.set_named_property("AudioListener", napi_class)?;

    let napi_class = NapiAudioBuffer::create_js_class(&env)?;
    store.set_named_property("AudioBuffer", napi_class)?;

    let napi_class = NapiMediaStream::create_js_class(&env)?;
    store.set_named_property("MediaStream", napi_class)?;

    // store the store into instance so that it can be globally accessed
    let store_ref = env.create_reference(store)?;
    env.set_instance_data(store_ref, 0, |mut c| {
        // don't have any idea of what this does
        c.value.unref(c.env).unwrap();
    })?;

    Ok(())
}
