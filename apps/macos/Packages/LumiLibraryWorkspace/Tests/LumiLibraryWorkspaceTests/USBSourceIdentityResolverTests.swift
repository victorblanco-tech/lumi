import Foundation
import Testing
@testable import LumiLibraryWorkspace

@Suite("USB source identity")
struct USBSourceIdentityResolverTests {
    @Test("Stable USB sources match only their filesystem identity")
    func stableSourcesDoNotAliasByDisplayName() {
        let gray = device(
            sourceID: "usb-fs:gray-volume",
            displayName: "DJ VIC GRAY"
        )
        let clonedName = MountedUSBIdentity(
            sourceID: "usb-fs:chrm-volume",
            displayName: "DJ VIC GRAY"
        )

        #expect(!USBSourceIdentityResolver.volume(clonedName, matches: gray))
        #expect(
            USBSourceIdentityResolver.selectedSourceID(for: clonedName, devices: [gray])
                == "usb-fs:chrm-volume"
        )
    }

    @Test("A mounted volume selects its exact stable trusted source")
    func exactStableIdentityWins() {
        let originalGray = device(
            sourceID: "usb-fs:gray-volume",
            displayName: "DJ VIC GRAY"
        )
        let staleCHRMLabel = device(
            sourceID: "usb-fs:chrm-volume",
            displayName: "DJ VIC GRAY"
        )
        let chrm = MountedUSBIdentity(
            sourceID: "usb-fs:chrm-volume",
            displayName: "DJ VIC CHRM"
        )

        #expect(
            USBSourceIdentityResolver.selectedSourceID(
                for: chrm,
                devices: [originalGray, staleCHRMLabel]
            ) == staleCHRMLabel.sourceID
        )
        #expect(!USBSourceIdentityResolver.volume(chrm, matches: originalGray))
        #expect(USBSourceIdentityResolver.volume(chrm, matches: staleCHRMLabel))
    }

    @Test("Current inspection label replaces a stale label for the same physical USB")
    func inspectionCorrectsPresentationName() {
        let stale = device(
            sourceID: "usb-fs:chrm-volume",
            displayName: "DJ VIC GRAY"
        )
        let inspection = RekordboxDeviceInspectionState(
            sourceID: stale.sourceID,
            displayName: "DJ VIC CHRM",
            databaseRevision: "chrm-revision",
            libraryFormat: "OneLibrary",
            databaseVersion: "1",
            exportedAt: "2026-08-16",
            trackCount: 956,
            playlistCount: 71,
            selectedPlaylistIDs: [],
            playlists: []
        )

        #expect(
            USBSourceIdentityResolver.displayName(for: stale, inspection: inspection)
                == "DJ VIC CHRM"
        )
    }

    @Test("Legacy sources may still migrate by their volume name")
    func legacyNameFallbackRemainsAvailable() {
        let legacy = device(
            sourceID: "rekordbox-device:dj-vic-gray",
            displayName: "DJ VIC GRAY"
        )
        let volume = MountedUSBIdentity(
            sourceID: "usb-fs:new-stable-identity",
            displayName: "dj vic gray"
        )

        #expect(
            USBSourceIdentityResolver.selectedSourceID(for: volume, devices: [legacy])
                == legacy.sourceID
        )
        #expect(USBSourceIdentityResolver.volume(volume, matches: legacy))
    }

    private func device(sourceID: String, displayName: String) -> RekordboxDeviceState {
        RekordboxDeviceState(
            sourceID: sourceID,
            displayName: displayName,
            databaseRevision: "same-cloned-revision",
            activeTracks: 63,
            matchedTracks: 63,
            unmatchedTracks: 0,
            syncedAt: "2026-08-16 09:00:00",
            trustState: "trusted",
            currentTracks: 63,
            promotedTracks: 0,
            protectedTracks: 0,
            conflictTracks: 0,
            beatGridRefresh: true,
            cueRevisionTracked: true,
            playlists: []
        )
    }
}
