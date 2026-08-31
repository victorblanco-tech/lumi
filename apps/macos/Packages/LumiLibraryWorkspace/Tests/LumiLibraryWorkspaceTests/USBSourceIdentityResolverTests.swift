import Foundation
import Testing
@testable import LumiLibraryWorkspace

@Suite("USB source identity")
struct USBSourceIdentityResolverTests {
    @Test("Independent FAT volumes with the same UUID remain separate")
    func independentMediaRemainSeparate() {
        let gray = USBStableSourceIdentity.sourceID(
            fileSystemUUID: "same-cloned-uuid",
            displayName: "DJ VIC GRAY"
        )
        let chrm = USBStableSourceIdentity.sourceID(
            fileSystemUUID: "same-cloned-uuid",
            displayName: "DJ VIC CHRM"
        )

        #expect(gray != nil)
        #expect(chrm != nil)
        #expect(gray != chrm)
        #expect(
            gray == USBStableSourceIdentity.sourceID(
                fileSystemUUID: "SAME-CLONED-UUID",
                displayName: "dj vic gray"
            )
        )
    }

    @Test("Hardware serial separates equal models with the same FAT identity")
    func hardwareSerialSeparatesEqualModels() {
        let chrm = USBStableSourceIdentity.sourceID(
            fileSystemUUID: "same-fat-uuid",
            displayName: "DJ VIC CHRM",
            hardwareSerial: "DD56419884401"
        )
        let renamedCHRM = USBStableSourceIdentity.sourceID(
            fileSystemUUID: "changed-fat-uuid",
            displayName: "CHRM RENAMED",
            hardwareSerial: "DD56419884401"
        )
        let gray = USBStableSourceIdentity.sourceID(
            fileSystemUUID: "same-fat-uuid",
            displayName: "DJ VIC GRAY",
            hardwareSerial: "DD56419884410"
        )

        #expect(chrm != renamedCHRM)
        #expect(chrm != gray)
        #expect(chrm?.hasPrefix("usb-fs:v2-") == true)
        #expect(
            chrm == USBStableSourceIdentity.sourceID(
                fileSystemUUID: "same-fat-uuid",
                displayName: "DJ VIC CHRM",
                hardwareSerial: "DD56419884401"
            )
        )
    }

    @Test("A current stable identity never rebinds by matching label")
    func currentStableIdentityNeverRebindsByLabel() {
        let trusted = device(
            sourceID: "usb-fs:v2-trusted",
            displayName: "DJ USB"
        )
        let replacement = MountedUSBIdentity(
            sourceID: "usb-fs:v2-replacement",
            displayName: "DJ USB"
        )

        #expect(
            USBSourceIdentityResolver.selectedSourceID(
                for: replacement,
                devices: [trusted]
            ) == replacement.sourceID
        )
        #expect(!USBSourceIdentityResolver.volume(replacement, matches: trusted))
    }

    @Test("Stable USB sources match only their filesystem identity")
    func stableSourcesDoNotAliasByDisplayName() {
        let gray = device(
            sourceID: "usb-fs:v2-gray",
            displayName: "DJ VIC GRAY"
        )
        let clonedName = MountedUSBIdentity(
            sourceID: "usb-fs:v2-chrm",
            displayName: "DJ VIC GRAY"
        )

        #expect(!USBSourceIdentityResolver.volume(clonedName, matches: gray))
        #expect(
            USBSourceIdentityResolver.selectedSourceID(for: clonedName, devices: [gray])
                == "usb-fs:v2-chrm"
        )
    }

    @Test("A mounted volume selects its exact stable trusted source")
    func exactStableIdentityWins() {
        let originalGray = device(
            sourceID: "usb-fs:hardware-gray",
            displayName: "DJ VIC GRAY"
        )
        let staleCHRMLabel = device(
            sourceID: "usb-fs:hardware-chrm",
            displayName: "DJ VIC GRAY"
        )
        let chrm = MountedUSBIdentity(
            sourceID: "usb-fs:hardware-chrm",
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
            sourceID: "usb-fs:hardware-chrm",
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
            sourceID: "usb-fs:hardware-new-stable-identity",
            displayName: "dj vic gray"
        )

        #expect(
            USBSourceIdentityResolver.selectedSourceID(for: volume, devices: [legacy])
                == legacy.sourceID
        )
        #expect(USBSourceIdentityResolver.volume(volume, matches: legacy))
    }

    @Test("A UUID-only trusted source attaches to its same-name hardware media for migration")
    func uuidOnlySourceAttachesForMigration() {
        let previousGray = device(
            sourceID: "usb-fs:5abc7360-045c-3a24-98a2-0723c3cb10fb",
            displayName: "DJ VIC GRAY"
        )
        let mountedGray = MountedUSBIdentity(
            sourceID: "usb-fs:hardware-778c17789217e275",
            displayName: "DJ VIC GRAY"
        )
        let currentInspection = RekordboxDeviceInspectionState(
            sourceID: mountedGray.sourceID!,
            displayName: mountedGray.displayName,
            databaseRevision: "gray-current",
            libraryFormat: "OneLibrary",
            databaseVersion: "1",
            exportedAt: "2026-08-23",
            trackCount: 1_156,
            playlistCount: 77,
            selectedPlaylistIDs: [],
            playlists: []
        )

        #expect(
            USBSourceIdentityResolver.selectedSourceID(
                for: mountedGray,
                devices: [previousGray]
            ) == previousGray.sourceID
        )
        #expect(USBSourceIdentityResolver.volume(mountedGray, matches: previousGray))
        #expect(USBSourceIdentityResolver.inspection(currentInspection, matches: previousGray))
    }

    @Test("A duplicated hardware serial cannot attach GRAY to CHRM")
    func duplicatedHardwareSerialUsesUniqueLegacyName() {
        let chrm = device(
            sourceID: "usb-fs:hardware-shared",
            displayName: "DJ VIC CHRM"
        )
        let previousGray = device(
            sourceID: "usb-fs:5abc7360-045c-3a24-98a2-0723c3cb10fb",
            displayName: "DJ VIC GRAY"
        )
        let mountedGray = MountedUSBIdentity(
            sourceID: chrm.sourceID,
            displayName: "DJ VIC GRAY"
        )

        #expect(
            USBSourceIdentityResolver.selectedSourceID(
                for: mountedGray,
                devices: [chrm, previousGray]
            ) == previousGray.sourceID
        )
        #expect(USBSourceIdentityResolver.mountedVolume(
            mountedGray,
            represents: previousGray,
            among: [chrm, previousGray]
        ))
        #expect(!USBSourceIdentityResolver.mountedVolume(
            mountedGray,
            represents: chrm,
            among: [chrm, previousGray]
        ))
    }

    @Test("Generated local identities are independent and syntactically stable")
    func generatedLocalIdentitiesDoNotAlias() {
        let first = USBLocalSourceIdentity.generated()
        let second = USBLocalSourceIdentity.generated()

        #expect(first.hasPrefix("usb-local:"))
        #expect(second.hasPrefix("usb-local:"))
        #expect(first != second)
        #expect(!first.contains("/"))
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
            reviewTracks: [],
            playlists: []
        )
    }
}
