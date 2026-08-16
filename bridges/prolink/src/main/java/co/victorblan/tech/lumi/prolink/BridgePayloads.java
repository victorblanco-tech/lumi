package co.victorblan.tech.lumi.prolink;

final class BridgePayloads {
    record Hello(String bridgeVersion, String beatLinkVersion, boolean readOnly) {
    }

    record SourceStatus(String status, String detail) {
    }

    record Device(
            int deviceNumber,
            String deviceName,
            String address
    ) {
    }

    record DeckStatus(
            int deviceNumber,
            String deviceName,
            boolean playing,
            boolean paused,
            boolean cued,
            boolean tempoMaster,
            boolean onAir,
            int sourcePlayer,
            String sourceSlot,
            String trackType,
            int rekordboxId,
            double trackBpm,
            double effectiveBpm,
            long beatNumber,
            int beatWithinBar,
            int rawPitch
    ) {
    }

    record Beat(
            int deviceNumber,
            String deviceName,
            double effectiveBpm,
            int beatWithinBar,
            boolean tempoMaster
    ) {
    }

    record PrecisePosition(
            int deviceNumber,
            String deviceName,
            long playbackPositionMillis,
            double effectiveBpm,
            int beatWithinBar,
            boolean tempoMaster
    ) {
    }

    record TrackMetadata(
            int deckNumber,
            boolean available,
            Integer sourcePlayer,
            String sourceSlot,
            String trackType,
            Integer rekordboxId,
            String title,
            String artist,
            Integer durationSeconds,
            Double trackBpm,
            String musicalKey,
            String color
    ) {
    }

    record TrackSignature(int deckNumber, String signature) {
    }

    record Error(String operation, String message) {
    }

    private BridgePayloads() {
    }
}
