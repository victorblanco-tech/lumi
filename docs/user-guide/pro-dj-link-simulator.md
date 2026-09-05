# Pro DJ Link Simulator

Try Lumi Live Decks without keeping your CDJs switched on. The separate **Lumi
Pro DJ Link Simulator** sends player discovery, track identity, position, tempo
and Master changes over your local network. Lumi receives them through its normal
Pro DJ Link input, not through a special demonstration mode.

It has two Players, independent track loops and playlist-driven Auto Mix. It
does **not** play audio or send DMX. SoundSwitch and your lighting configuration
remain responsible for the lights.

## Set up once

1. Download the **Simulator** DMG from [GitHub Releases](https://github.com/victorblanco-tech/lumi/releases).
   The simulator has its own version and installer; it is not part of the Lumi DMG.
2. Copy the simulator app to Applications and open it. Java is included.
   A non-admin account can install it in its own `~/Applications` folder.
3. If macOS blocks the downloaded app, use **System Settings → Privacy & Security
   → Open Anyway** after checking that it came from the official repository.
4. Synchronize the desired rekordbox OneLibrary USB playlists into Lumi first.
   Then safely eject that USB and connect it to the Mac running the simulator.
5. Select the USB in the simulator and start it. Allow Local Network access if
   macOS asks. Use the app's **Open** or **Copy** control for the remote-control URL.
6. On the Lumi Mac, enable **Integrations → Pro DJ Link**, then open **Live → Live
   Decks**. Keep Beat Link Trigger offline on that Mac: it uses the same network ports.

Use two Macs on the same local network: one for Lumi and one for the simulator.
Wi-Fi can be used; Ethernet helps rule out wireless loss during timing tests.
Do not reuse the Player numbers of physical CDJs that are active on that network.

## Load and play two tracks

Search the USB track list, then choose **Load P1** or **Load P2**. Each Player has
its own **Play**, **Pause**, **Master**, **On Air**, position and pitch controls.

- **Master** selects the Player whose tempo and lighting plan Lumi should follow.
- **Position** moves within the track, including backwards. Lumi should follow
  the new position and choose the appropriate phrase.
- **Pitch** changes the effective BPM. With Lumi's Ableton Link relay enabled,
  SoundSwitch should follow the Master tempo.
- **On Air** represents that Player's mixer-channel state; it does not play audio.

In Lumi, use **Arm**, then **Start** when you want real lighting output. This can
trigger your mapped lights, just as with physical players. Use **Pause** or **Off**
in Lumi when checking the simulator without wanting new lighting actions.

## Repeat a section

1. Move to the start position and press **Loop In**.
2. Move to the end position and press **Loop Out**.
3. Start playback. That Player wraps from the end back to the start.
4. Press **Loop Off** to continue normally.

Loops belong to each Player separately. A long loop crossing several Lumi
phrases is useful for checking repeated AutoLoop transitions. Moving backwards
at the loop boundary is expected; jumping during uninterrupted playback is not.

## Run an unattended test with Auto Mix

Choose a playlist, select the switching interval and optionally enable
**Shuffle**, then press **Start Auto Mix**. Before each handoff, the simulator
loads another track onto the idle Player and transfers Master. Without a playlist,
Auto Mix alternates the two manually loaded tracks.

**Stop Auto Mix** stops automatic handoffs, not necessarily the Player already
playing. Pause the Players separately, or turn Lumi **Off**, to stop the test.

For a useful first test, use a small playlist whose tracks are already synced
and prepared in Lumi. Check:

- both tracks resolve to the correct local library entries;
- changing Master selects the other Player and its prepared Light Plan;
- play, pause, backward seeks and loop wraps remain responsive;
- SoundSwitch tempo follows pitch changes without timeline corrections;
- AutoLoops change at the intended phrase boundaries;
- switching between Lumi pages or reconnecting Lumi Remote does not disrupt
  the show.

Simulator tests help find regressions. They do not replace final testing with
your own CDJs, network and lighting hardware.

## When something is missing

| Symptom | Check |
| --- | --- |
| No Players in Lumi | Same LAN, Local Network permission, selected network interface and no competing Pro DJ Link app on the Lumi Mac. |
| Player appears, track is unknown | Sync that USB's playlist into Lumi. A title alone is not proof of the correct USB track identity. |
| Track appears, no useful Light Plan | Check its Lumi phrases and your SoundSwitch Bank/AutoLoop mappings. Loading a track in the simulator does not prepare it in Lumi. |
| Remote-control URL no longer works | Copy the current URL from the simulator app. A restart may change its access token. |
| No sound | Expected: the simulator generates network playback information, not audio. |

The control URL grants access to the simulator. Keep it private, use a trusted
local network and never forward its port to the internet. Do not include its
token in screenshots or issue reports.

For logs, scripting and developer-only fault tests, see the
[technical simulator guide](https://github.com/victorblanco-tech/lumi/blob/dev/tools/prolink-simulator/README.md).

[Back to the Lumi user guide](README.md)
