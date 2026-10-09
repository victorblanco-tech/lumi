package co.victorblan.tech.lumi.prolink.simulator;

import java.net.Inet4Address;
import java.net.InetAddress;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Manual UI acceptance fixture. Not packaged; never opens the user's library or USB. */
public final class MediaAcceptanceHarness {
    public static void main(String[] args) throws Exception {
        Path root=Files.createTempDirectory("lumi-simulator-ui-");
        PlayerState first=new PlayerState(1), second=new PlayerState(2);
        List<PlayerState> players=List.of(first,second);
        for (int i=0;i<2;i++) {
            Path usb=Files.createDirectory(root.resolve("Fixture-USB-"+(i+1)));
            Files.writeString(usb.resolve(".lumi-media.json"),"{\"schemaVersion\":1,\"mediaId\":\"00000000-0000-4000-8000-00000000000"+i+"\",\"sourceId\":\"usb-local:fixture"+i+"\"}");
            ArrayList<UsbLibrary.BeatPoint> beats=new ArrayList<>();
            for(int b=0;b<480;b++) beats.add(new UsbLibrary.BeatPoint(b+1,b%4+1,12000,b*500));
            var track=new UsbLibrary.Track(101,"Test fixture "+(i+1),"Synthetic acceptance",12000,240000,usb.resolve("unused.DAT"),true,beats);
            var alternate=new UsbLibrary.Track(102,"Alternate fixture "+(i+1),"Synthetic acceptance",12000,240000,usb.resolve("unused.DAT"),true,beats);
            UsbLibrary library=UsbLibrary.forTesting(usb,List.of(track,alternate),List.of(new UsbLibrary.Playlist(1,"Acceptance / Tracks",List.of(track,alternate))));
            players.get(i).configureUsb(library);
            players.get(i).loadFrom(players.get(i).usb(),101);
        }
        var primary=ProLinkBroadcaster.selectEndpoint(null);
        var loopback=new ProLinkBroadcaster.Endpoint("loopback-fixture",(Inet4Address)InetAddress.getByName("127.0.0.1"),
                (Inet4Address)InetAddress.getByName("127.255.255.255"),new byte[6]);
        MediaRpcService[] media={null};
        SimulatorTransport network=new SimulatorTransport() {
            public void triggerPreciseBurst(int number) {}
            public ProLinkBroadcaster.Endpoint endpoint() {return primary;}
            public ProLinkBroadcaster.Endpoint endpointForPlayer(int player) {return player==1?loopback:primary;}
            public int peerCount() {return 0;}
            public ProLinkBroadcaster.TrafficDiagnostics trafficDiagnostics() {return new ProLinkBroadcaster.TrafficDiagnostics("fixture-no-broadcast",200,20,0,0,0,0,0,null);}
            public String mediaStatus() {return media[0].status();}
            public void mediaFault(int player,String kind,int duration) {media[0].fault(player,kind,duration);}
            public void clearMediaFaults() {media[0].clearFaults();}
        };
        media[0]=MediaRpcService.start(players,network);
        var library=first.usb().configuredLibrary();
        var autoMix=new AutoMixController(players,library);
        var faults=new TrafficFaultController(players,autoMix);
        var server=new RemoteControlServer(library,players,autoMix,network,faults,"127.0.0.1",0,"local-ui-acceptance-only");
        first.setMaster(true); first.play(); first.setLoop(0,8000);
        server.start();
        System.out.println("UI http://127.0.0.1:"+server.port()+"/?token=local-ui-acceptance-only");
        try { System.in.read(); }
        finally {
            server.close(); faults.close();autoMix.close();media[0].close();
            try(var paths=Files.walk(root)) {for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);}
        }
    }
}
