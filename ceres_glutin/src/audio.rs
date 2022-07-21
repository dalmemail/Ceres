use {
    cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        SampleRate,
    },
    dasp_ring_buffer::Bounded,
    parking_lot::Mutex,
    std::sync::Arc,
};

const BUFFER_SIZE: cpal::FrameCount = 1024;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 4;
const SAMPLE_RATE: u32 = 48000;

pub struct Renderer {
    ring_buffer: Arc<Mutex<Bounded<Box<[ceres_core::Sample]>>>>,
    stream: cpal::Stream,
}

impl Renderer {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();

        let desired_config = cpal::StreamConfig {
            channels: 2,
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        let ring_buffer = Arc::new(Mutex::new(Bounded::from(
            vec![0.0; RING_BUFFER_SIZE].into_boxed_slice(),
        )));
        let error_callback = |err| panic!("an AudioError occurred on stream: {}", err);
        let ring_buffer_arc = Arc::clone(&ring_buffer);
        let data_callback = move |output: &mut [f32], _: &_| {
            let mut buf = ring_buffer_arc.lock();
            output
                .iter_mut()
                .zip(buf.drain())
                .for_each(|(out_sample, gb_sample)| *out_sample = gb_sample);
        };

        let stream = device
            .build_output_stream(&desired_config, data_callback, error_callback)
            .unwrap();

        stream.play().expect("AudioError playing sound");

        Self {
            ring_buffer,
            stream,
        }
    }

    #[allow(dead_code)]
    pub fn play(&mut self) {
        self.stream.play().unwrap();
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        self.stream.pause().unwrap();
    }

    pub fn sample_rate() -> u32 {
        SAMPLE_RATE
    }

    pub fn push_frame(&mut self, l: ceres_core::Sample, r: ceres_core::Sample) {
        let mut buf = self.ring_buffer.lock();
        buf.push(l);
        buf.push(r);
    }
}
