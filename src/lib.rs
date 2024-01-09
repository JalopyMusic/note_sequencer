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
    last_playing: Option<bool>,
    last_pos_beats: Option<f64>,
    searching_for_step: bool,
}

impl Default for MyPlugin {
    fn default() -> Self {
        nih_log!("default constructor");
        Self {
            params: Arc::new(MyPluginParams::default()),
            buffer_sample_rate: None,
            last_playing: None,
            last_pos_beats: None,
            searching_for_step: true,
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
        nih_log!("initialize");
        self.buffer_sample_rate = Some(buffer_config.sample_rate);
        self.last_playing = None;
        self.last_pos_beats = None;
        self.searching_for_step = true;
        true
    }

    fn reset(&mut self) {
        nih_log!("reset");
        self.last_playing = None;
        self.last_pos_beats = None;
        self.searching_for_step = true;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let transport = context.transport();

        if !transport.playing {
            if self.last_playing.unwrap_or(true) {
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
            self.last_playing = Some(false);
            self.searching_for_step = true;
            return ProcessStatus::Normal;
        }

        if transport.preroll_active.unwrap_or(false) {
            nih_log!("preroll active: do nothing");
            return ProcessStatus::Normal;
        }

        if transport.playing && !self.last_playing.unwrap_or(false) {
            // catch initial step
            self.searching_for_step = true;
        }

        self.last_playing = Some(true);

        let pos_beats = match transport.pos_beats() {
            Some(value) => value,
            None => {
                nih_log!("missing pos_beats");
                return ProcessStatus::Normal;
            }
        };

        // if a note on/off should be sent within this buffer,
        // then timing is set to the sample index corresponding to the start of the step
        let mut timing: Option<u32> = None;

        // sometimes steps begin between buffers
        if self.searching_for_step
            && pos_beats.floor() > self.last_pos_beats.unwrap_or(-1.0).floor()
        {
            nih_log!("missed buffer containing step start, setting timing to 0");
            timing = Some(0);
        }

        self.last_pos_beats = Some(pos_beats);
        self.searching_for_step = true;

        if timing == None {
            // fraction of a beat remaining in this beat
            let remain_beats: f64 = 1.0 - pos_beats % 1.0;

            let tempo: f64 = match transport.tempo {
                Some(value) => value,
                None => {
                    nih_log!("missing tempo");
                    return ProcessStatus::Normal;
                }
            };

            // fraction of a second remaining in this beat
            let remain_seconds: f64 = remain_beats * 60.0 / tempo;

            let buffer_samples = buffer.samples();

            let buffer_sample_rate = match self.buffer_sample_rate {
                Some(value) => value,
                None => {
                    nih_log!("missing buffer_sample_rate");
                    return ProcessStatus::Normal;
                }
            };

            // fraction of a second this buffer represents
            let buffer_seconds: f64 = buffer_samples as f64 / buffer_sample_rate as f64;

            if remain_seconds > buffer_seconds {
                // buffer does not contain a beat
                return ProcessStatus::Normal;
            }

            nih_log!("buffer contains start of step");
            self.searching_for_step = false;

            // sample index of next beat
            let remain_samples = (buffer_sample_rate as f64 * remain_seconds).round() as i32;

            if remain_samples < 0 {
                nih_log!("timing is < 0");
                return ProcessStatus::Normal;
            }

            if remain_samples >= buffer_samples as i32 {
                nih_log!("timing is >= buffer size");
                return ProcessStatus::Normal;
            }

            timing = Some(remain_samples as u32);
        }

        match timing {
            Some(timing) => {
                context.send_event(NoteEvent::NoteOn {
                    timing,
                    voice_id: None,
                    channel: 0,
                    note: 60,
                    velocity: 0.8,
                });
            }
            None => {
                nih_log!("missing timing");
            }
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
