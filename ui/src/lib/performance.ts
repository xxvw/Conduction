import { invoke } from "@tauri-apps/api/core";
import type { AudioOutputConfig, AudioOutputDescriptor, ControllerAction, LinkConfig, LinkInterface, MidiConfig, MidiDevices, MidiProfile, PerformanceBrowser, PerformanceStatus } from "@/types/performance";

export const performanceIpc = {
  status: () => invoke<PerformanceStatus>("get_performance_status"),
  perform: (action: ControllerAction) => invoke<void>("perform", { action }),
  configureAudio: (config: AudioOutputConfig) => invoke<void>("configure_audio", { config }),
  audioOutputs: () => invoke<AudioOutputDescriptor[]>("list_audio_outputs"),
  configureLink: (config: LinkConfig) => invoke<void>("configure_link", { config }),
  linkInterfaces: () => invoke<LinkInterface[]>("list_link_interfaces"),
  configureMidi: (config: MidiConfig) => invoke<void>("configure_midi", { config }),
  disconnectMidi: (inputPort: string | null) => invoke<void>("disconnect_midi", { inputPort }),
  midiDevices: () => invoke<MidiDevices>("list_midi_devices"),
  midiProfiles: () => invoke<MidiProfile[]>("list_midi_profiles"),
  requestLinkMaster: () => invoke<void>("request_link_master"),
  refreshLinkLibrary: () => invoke<void>("refresh_link_library"),
  setBrowser: (browser: PerformanceBrowser) => invoke<void>("set_performance_browser", { browser }),
};
