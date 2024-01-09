use nih_plug::prelude::*;
use std::sync::Arc;

const MILLISECONDS: &[time::format_description::FormatItem] =
    time::macros::format_description!("[subsecond digits:3]");

macro_rules! nih_log {
    ($($args:tt)*) => (
        let ms = time::OffsetDateTime::now_utc().format(MILLISECONDS).unwrap_or("xxx".to_string());
        let ms_msg = format!("{} {}", ms, format_args!($($args)*));
        nih_plug::prelude::nih_log!("{ms_msg}");
    );
}

#[derive(Params)]
struct MyPluginParams {}

impl Default for MyPluginParams {
    fn default() -> Self {
        Self {}
    }
}

struct MyPlugin {
    params: Arc<MyPluginParams>,
    buffer_sample_rate: Option<f32>,
    last_playing: bool,
}

impl Default for MyPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(MyPluginParams::default()),
            buffer_sample_rate: None,
            last_playing: true,
        }
    }
}

impl Plugin for MyPlugin {
    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.buffer_sample_rate = Some(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let transport = context.transport();

        if !transport.playing {
            if self.last_playing {
                nih_log!("all notes off: transport pause");
                for n in 0..=127 {
                    context.send_event(NoteEvent::NoteOff {
                        timing: 0,
                        voice_id: None,
                        channel: 0,
                        note: n,
                        velocity: 0.0,
                    });
                }
            }
            self.last_playing = false;
            return ProcessStatus::Normal;
        }

        if transport.preroll_active.unwrap_or(false) {
            nih_log!("preroll active: do nothing");
            return ProcessStatus::Normal;
        }

        self.last_playing = true;

        let pos_samples: i64 = match transport.pos_samples() {
            Some(value) => value,
            None => {
                nih_log!("missing pos_samples");
                return ProcessStatus::Normal;
            }
        };

        if pos_samples == 0 {
            nih_log!("note on: initial beat");
            context.send_event(NoteEvent::NoteOn {
                timing: 0,
                voice_id: None,
                channel: 0,
                note: 60,
                velocity: 0.8,
            });
            return ProcessStatus::Normal;
        }

        let pos_beats = match transport.pos_beats() {
            Some(value) => value,
            None => {
                nih_log!("missing pos_beats");
                return ProcessStatus::Normal;
            }
        };

        let tempo: f64 = match transport.tempo {
            Some(value) => value,
            None => {
                nih_log!("missing tempo");
                return ProcessStatus::Normal;
            }
        };

        let buffer_sample_rate = match self.buffer_sample_rate {
            Some(value) => value,
            None => {
                nih_log!("missing buffer_sample_rate");
                return ProcessStatus::Normal;
            }
        };

        // fraction of a beat remaining in this beat
        let remain_beats: f64 = 1.0 - pos_beats % 1.0;

        // fraction of a second remaining in this beat
        let remain_seconds: f64 = remain_beats * 60.0 / tempo;

        let buffer_samples = buffer.samples();

        // fraction of a second this buffer represents
        let buffer_seconds: f64 = buffer_samples as f64 / buffer_sample_rate as f64;

        if remain_seconds > buffer_seconds {
            nih_log!(
                "\
                remain_seconds={remain_seconds} \
                buffer_seconds={buffer_seconds} \
                remain_beats={remain_beats} \
                pos_beats={pos_beats} \
                buffer_samples={buffer_samples} \
                tempo={tempo} \
                buffer_sample_rate={buffer_sample_rate} \
                "
            );

            // buffer does not contain a beat
            return ProcessStatus::Normal;
        }

        // sample index of next beat
        let remain_samples: i64 = (buffer_sample_rate as f64 * remain_seconds).round() as i64;

        if remain_samples < 0 {
            nih_log!("sample index of next beat is unexpectedly a negative number");
            return ProcessStatus::Normal;
        }

        if remain_samples >= buffer_samples as i64 {
            nih_log!("computed sample index of next beat is unexpectedly greater than max sample index for buffer");
            return ProcessStatus::Normal;
        }

        // send quarter note on every odd beat
        if (pos_beats / 1.0) as i64 % 2 == 0 {
            context.send_event(NoteEvent::NoteOff {
                timing: remain_samples as u32,
                voice_id: None,
                channel: 0,
                note: 60,
                velocity: 0.0,
            });
        } else {
            context.send_event(NoteEvent::NoteOn {
                timing: remain_samples as u32,
                voice_id: None,
                channel: 0,
                note: 60,
                velocity: 0.8,
            });
        }

        ProcessStatus::Normal
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    const NAME: &'static str = "Note Sequencer";
    const VENDOR: &'static str = "Brian Edwards";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "brian.edwards@jalopymusic.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();
}

impl ClapPlugin for MyPlugin {
    const CLAP_ID: &'static str = "com.jalopymusic.note-sequencer";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Hello world note sequencer plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteEffect];
}

impl Vst3Plugin for MyPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"NoteSequencerJal";

    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument];
}

nih_export_clap!(MyPlugin);
nih_export_vst3!(MyPlugin);
