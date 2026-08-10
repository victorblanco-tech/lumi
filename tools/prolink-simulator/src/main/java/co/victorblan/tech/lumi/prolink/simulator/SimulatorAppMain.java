package co.victorblan.tech.lumi.prolink.simulator;

import javax.swing.BorderFactory;
import javax.swing.Box;
import javax.swing.BoxLayout;
import javax.swing.JButton;
import javax.swing.JCheckBox;
import javax.swing.JComboBox;
import javax.swing.JFrame;
import javax.swing.JLabel;
import javax.swing.JPanel;
import javax.swing.JSpinner;
import javax.swing.JTextField;
import javax.swing.SpinnerNumberModel;
import javax.swing.SwingUtilities;
import javax.swing.UIManager;
import javax.swing.WindowConstants;
import java.awt.BorderLayout;
import java.awt.Color;
import java.awt.Desktop;
import java.awt.Dimension;
import java.awt.FlowLayout;
import java.awt.Font;
import java.awt.Toolkit;
import java.awt.datatransfer.StringSelection;
import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.Instant;
import java.util.Comparator;
import java.util.List;
import java.util.Properties;

public final class SimulatorAppMain {
    private static final String APP_NAME = "Lumi Pro DJ Link Simulator";
    private static final Path SUPPORT_ROOT = Path.of(
            System.getProperty("user.home"), "Library", "Application Support", APP_NAME
    );
    private static final Path SETTINGS_FILE = SUPPORT_ROOT.resolve("config.properties");
    private static final Path LOG_FILE = Path.of(
            System.getProperty("user.home"), "Library", "Logs", APP_NAME, "simulator.log"
    );

    private SimulatorAppMain() {
    }

    public static void main(String[] arguments) {
        configureLogging();
        System.setProperty("apple.awt.application.name", APP_NAME);
        System.setProperty("apple.laf.useScreenMenuBar", "true");
        SwingUtilities.invokeLater(() -> {
            try {
                UIManager.setLookAndFeel(UIManager.getSystemLookAndFeelClassName());
            } catch (Exception ignored) {
                // The cross-platform Swing theme remains a safe fallback.
            }
            new SimulatorWindow().show();
        });
    }

    static List<Path> findRekordboxVolumes(Path volumesRoot) throws IOException {
        if (!Files.isDirectory(volumesRoot)) {
            return List.of();
        }
        try (var entries = Files.list(volumesRoot)) {
            return entries
                    .filter(Files::isDirectory)
                    .filter(path -> Files.isRegularFile(path.resolve("PIONEER/rekordbox/export.pdb")))
                    .sorted(Comparator.comparing(path -> path.getFileName().toString(), String.CASE_INSENSITIVE_ORDER))
                    .toList();
        }
    }

    private static void configureLogging() {
        try {
            Files.createDirectories(LOG_FILE.getParent());
            System.setProperty("org.slf4j.simpleLogger.logFile", LOG_FILE.toString());
            Files.writeString(
                    LOG_FILE, System.lineSeparator() + "=== App launch " + Instant.now() + " ===" + System.lineSeparator(),
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND
            );
        } catch (IOException failure) {
            System.err.println("Could not configure log file: " + failure.getMessage());
        }
    }

    private static final class SimulatorWindow {
        private static final Color ACCENT = new Color(0x63C7FF);
        private static final Color READY = new Color(0x36D37E);
        private static final Color MUTED = new Color(0x8E9AA7);

        private final JFrame frame = new JFrame(APP_NAME);
        private final JComboBox<Path> usbVolumes = new JComboBox<>();
        private final JSpinner playerNumber = new JSpinner(new SpinnerNumberModel(1, 1, 4, 1));
        private final JCheckBox autoStart = new JCheckBox("Start automatically when a Rekordbox USB is found", true);
        private final JButton startStop = new JButton("Start simulator");
        private final JButton refresh = new JButton("Refresh USBs");
        private final JLabel status = new JLabel("Looking for a Rekordbox USB…");
        private final JLabel detail = new JLabel(" ");
        private final JTextField remoteUrl = new JTextField();
        private final JButton copyUrl = new JButton("Copy URL");
        private final JButton openUrl = new JButton("Open controls");
        private final Properties settings = loadSettings();
        private volatile SimulatorSession session;
        private volatile boolean busy;

        private SimulatorWindow() {
            frame.setDefaultCloseOperation(WindowConstants.DO_NOTHING_ON_CLOSE);
            frame.addWindowListener(new java.awt.event.WindowAdapter() {
                @Override
                public void windowClosing(java.awt.event.WindowEvent event) {
                    stopSession();
                    saveSettings();
                    frame.dispose();
                    System.exit(0);
                }
            });
            frame.setMinimumSize(new Dimension(580, 420));
            frame.setSize(640, 460);
            frame.setLocationByPlatform(true);
            frame.setContentPane(content());
            bindActions();
            applySettings();
        }

        void show() {
            frame.setVisible(true);
            refreshVolumes(true);
        }

        private JPanel content() {
            JPanel root = new JPanel();
            root.setLayout(new BoxLayout(root, BoxLayout.Y_AXIS));
            root.setBorder(BorderFactory.createEmptyBorder(24, 26, 24, 26));

            JLabel title = new JLabel(APP_NAME);
            title.setFont(title.getFont().deriveFont(Font.BOLD, 24f));
            title.setAlignmentX(0f);
            root.add(title);
            JLabel subtitle = new JLabel("USB-backed development deck for Lumi");
            subtitle.setForeground(MUTED);
            subtitle.setAlignmentX(0f);
            root.add(subtitle);
            root.add(Box.createVerticalStrut(24));

            root.add(row("Rekordbox USB", usbVolumes, refresh));
            root.add(Box.createVerticalStrut(10));
            root.add(row("Player number", playerNumber));
            root.add(Box.createVerticalStrut(12));
            autoStart.setAlignmentX(0f);
            root.add(autoStart);
            root.add(Box.createVerticalStrut(22));

            JPanel statusPanel = new JPanel(new BorderLayout(12, 4));
            statusPanel.setBorder(BorderFactory.createCompoundBorder(
                    BorderFactory.createLineBorder(new Color(0x35424D)),
                    BorderFactory.createEmptyBorder(14, 14, 14, 14)
            ));
            status.setFont(status.getFont().deriveFont(Font.BOLD, 15f));
            statusPanel.add(status, BorderLayout.NORTH);
            detail.setForeground(MUTED);
            statusPanel.add(detail, BorderLayout.CENTER);
            statusPanel.setAlignmentX(0f);
            root.add(statusPanel);
            root.add(Box.createVerticalStrut(14));

            remoteUrl.setEditable(false);
            remoteUrl.setVisible(false);
            remoteUrl.setAlignmentX(0f);
            root.add(remoteUrl);
            root.add(Box.createVerticalStrut(10));

            JPanel actions = new JPanel(new FlowLayout(FlowLayout.LEFT, 8, 0));
            actions.setAlignmentX(0f);
            startStop.setForeground(ACCENT);
            actions.add(startStop);
            copyUrl.setVisible(false);
            openUrl.setVisible(false);
            actions.add(copyUrl);
            actions.add(openUrl);
            root.add(actions);
            root.add(Box.createVerticalGlue());

            JLabel footer = new JLabel("Development tool · trusted local network only");
            footer.setForeground(MUTED);
            footer.setAlignmentX(0f);
            root.add(footer);
            return root;
        }

        private JPanel row(String labelText, java.awt.Component... controls) {
            JPanel row = new JPanel(new BorderLayout(12, 0));
            JLabel label = new JLabel(labelText);
            label.setPreferredSize(new Dimension(120, 30));
            row.add(label, BorderLayout.WEST);
            JPanel fields = new JPanel(new FlowLayout(FlowLayout.LEFT, 8, 0));
            for (java.awt.Component control : controls) {
                if (control == usbVolumes) {
                    control.setPreferredSize(new Dimension(300, 30));
                }
                fields.add(control);
            }
            row.add(fields, BorderLayout.CENTER);
            row.setMaximumSize(new Dimension(Integer.MAX_VALUE, 34));
            row.setAlignmentX(0f);
            return row;
        }

        private void bindActions() {
            refresh.addActionListener(event -> refreshVolumes(false));
            startStop.addActionListener(event -> {
                if (session == null) {
                    startSession();
                } else {
                    stopSession();
                }
            });
            copyUrl.addActionListener(event -> Toolkit.getDefaultToolkit().getSystemClipboard()
                    .setContents(new StringSelection(remoteUrl.getText()), null));
            openUrl.addActionListener(event -> {
                try {
                    Desktop.getDesktop().browse(URI.create(remoteUrl.getText()));
                } catch (Exception failure) {
                    showFailure(failure);
                }
            });
        }

        private void refreshVolumes(boolean startWhenReady) {
            if (busy || session != null) {
                return;
            }
            busy = true;
            status.setText("Looking for a Rekordbox USB…");
            detail.setText("Scanning /Volumes");
            Thread.startVirtualThread(() -> {
                try {
                    List<Path> volumes = findRekordboxVolumes(Path.of("/Volumes"));
                    SwingUtilities.invokeLater(() -> {
                        usbVolumes.removeAllItems();
                        volumes.forEach(usbVolumes::addItem);
                        selectPreferredVolume();
                        busy = false;
                        if (volumes.isEmpty()) {
                            status.setText("No Rekordbox USB found");
                            detail.setText("Connect a USB exported by Rekordbox, then choose Refresh USBs.");
                            startStop.setEnabled(false);
                        } else {
                            status.setText(volumes.size() == 1 ? "Rekordbox USB ready" : volumes.size() + " Rekordbox USBs found");
                            detail.setText("Select the device that this simulated deck should load tracks from.");
                            startStop.setEnabled(true);
                            if (startWhenReady && autoStart.isSelected()) {
                                startSession();
                            }
                        }
                    });
                } catch (Exception failure) {
                    SwingUtilities.invokeLater(() -> {
                        busy = false;
                        showFailure(failure);
                    });
                }
            });
        }

        private void startSession() {
            Path usb = (Path) usbVolumes.getSelectedItem();
            if (usb == null || busy) {
                return;
            }
            busy = true;
            setControlsEnabled(false);
            status.setText("Starting simulator…");
            detail.setText("Reading the Rekordbox USB and joining the local Pro DJ Link network.");
            int player = (Integer) playerNumber.getValue();
            Thread.startVirtualThread(() -> {
                try {
                    SimulatorConfig config = SimulatorConfig.parse(new String[]{
                            "--usb", usb.toString(), "--player", Integer.toString(player)
                    });
                    SimulatorSession started = SimulatorSession.start(config);
                    session = started;
                    SwingUtilities.invokeLater(() -> {
                        busy = false;
                        status.setForeground(READY);
                        status.setText("Simulator running · Deck " + player);
                        detail.setText(started.library().size() + " tracks · " + started.networkSummary());
                        remoteUrl.setText(started.remoteUrl());
                        remoteUrl.setVisible(true);
                        copyUrl.setVisible(true);
                        openUrl.setVisible(true);
                        startStop.setText("Stop simulator");
                        startStop.setEnabled(true);
                        frame.revalidate();
                        saveSettings();
                    });
                } catch (Exception failure) {
                    SwingUtilities.invokeLater(() -> {
                        busy = false;
                        setControlsEnabled(true);
                        showFailure(failure);
                    });
                }
            });
        }

        private void stopSession() {
            SimulatorSession running = session;
            session = null;
            if (running != null) {
                running.close();
            }
            busy = false;
            status.setForeground(UIManager.getColor("Label.foreground"));
            status.setText("Simulator stopped");
            detail.setText("The USB remains available for another test run.");
            remoteUrl.setVisible(false);
            copyUrl.setVisible(false);
            openUrl.setVisible(false);
            startStop.setText("Start simulator");
            setControlsEnabled(true);
            frame.revalidate();
        }

        private void setControlsEnabled(boolean enabled) {
            usbVolumes.setEnabled(enabled);
            playerNumber.setEnabled(enabled);
            refresh.setEnabled(enabled);
            autoStart.setEnabled(enabled);
            startStop.setEnabled(enabled);
        }

        private void showFailure(Throwable failure) {
            status.setForeground(new Color(0xFF6677));
            status.setText("Simulator could not start");
            detail.setText(failure.getMessage() == null ? failure.getClass().getSimpleName() : failure.getMessage());
        }

        private void applySettings() {
            playerNumber.setValue(Integer.parseInt(settings.getProperty("playerNumber", "1")));
            autoStart.setSelected(Boolean.parseBoolean(settings.getProperty("autoStart", "true")));
        }

        private void selectPreferredVolume() {
            String preferred = settings.getProperty("usbRoot", "");
            for (int index = 0; index < usbVolumes.getItemCount(); index++) {
                if (usbVolumes.getItemAt(index).toString().equals(preferred)) {
                    usbVolumes.setSelectedIndex(index);
                    return;
                }
            }
        }

        private void saveSettings() {
            try {
                Files.createDirectories(SUPPORT_ROOT);
                Path usb = (Path) usbVolumes.getSelectedItem();
                if (usb != null) {
                    settings.setProperty("usbRoot", usb.toString());
                }
                settings.setProperty("playerNumber", playerNumber.getValue().toString());
                settings.setProperty("autoStart", Boolean.toString(autoStart.isSelected()));
                try (var output = Files.newOutputStream(SETTINGS_FILE)) {
                    settings.store(output, APP_NAME);
                }
            } catch (IOException failure) {
                System.err.println("Could not save settings: " + failure.getMessage());
            }
        }

        private Properties loadSettings() {
            Properties loaded = new Properties();
            if (!Files.isRegularFile(SETTINGS_FILE)) {
                return loaded;
            }
            try (var input = Files.newInputStream(SETTINGS_FILE)) {
                loaded.load(input);
            } catch (IOException failure) {
                System.err.println("Could not read settings: " + failure.getMessage());
            }
            return loaded;
        }
    }
}
