# SoundSwitch / Control One responsiveness incident

During E11 desktop acceptance on 2026-10-09, SoundSwitch 2.10.3 stopped
responding to accessibility requests. A three-second process sample at 12:40
confirmed that its main thread remained in the same blocking call throughout
the sample:

`MIDIManager::pollWork → JLC1Manager::resetJLC1DeviceList →
JLC1Storage::~JLC1Storage → std::thread::join`.

The storage worker was waiting in
`MIDIManager::sendMIDIMessage → JLC1Manager::getJLC1LastExclusiveMsgID →
std::recursive_mutex::lock`. Another Control One firmware-availability worker
was waiting for a manager lock as well. This is evidence of a blocked Control
One device-lifecycle path, not proof of the initiating cause or of a specific
Lumi defect. MIDI endpoint creation/removal during tests is a candidate trigger
to investigate, not a demonstrated cause.

The local diagnostic is `build/soundswitch-responsiveness-sample.txt`. Do not
publish the raw system sample: it contains local machine/process details.

Lumi Dev-24 was quit normally afterward. A process check found no remaining
Lumi engine, Pro DJ Link bridge or Carabiner process. Permission was requested
before any forced SoundSwitch restart because unsaved project changes could
be lost. No mappings, OS permissions or network settings were changed.

## Acceptance consequence

- MIDI scheduler/dispatch tests remain useful but cannot establish downstream
  playback acceptance while SoundSwitch is unresponsive.
- Restore SoundSwitch only with owner approval; confirm Control One and its UI.
- Repeat a bounded Lumi cold start / normal quit / restart sequence while
  observing SoundSwitch, separately from high-volume endpoint integration tests.
- If reproduced, preserve samples and endpoint lifecycle evidence. Do not hide
  this by restarting SoundSwitch automatically or changing its mappings.
- Keep E11 integrated acceptance open until actual SoundSwitch playback and
  responsiveness pass.

## Authorized recovery

The owner approved a forced restart. The exact hung process was stopped and
SoundSwitch reopened the existing project without changing mappings. Its UI
responded again and showed Control One Connected. Enabling Link showed one
peer at 155.0 BPM; simulator pitch -2% produced 151.9 BPM in SoundSwitch, then
restoring pitch produced 155.0 BPM. These UI observations establish correct
tempo values, not a measured end-to-end latency bound.

A normal Lumi Dev-25 quit left no engine, bridge, gateway or Carabiner process;
SoundSwitch remained responsive and its Link peer disappeared. The application
was then reopened: SoundSwitch stayed responsive and showed one Link peer at
155.0 BPM again. The venue-selection screen requires
the owner's choice between Woonkamer v001 and v002 before playback acceptance;
no venue or fixture configuration was guessed.
